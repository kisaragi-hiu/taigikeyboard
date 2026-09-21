// Verifies the v3.6.1 R3 custom-dictionary cross-mode SQL JOIN structure.
// Android's SQLiteDatabase is unavailable in JVM unit tests, so this exercises
// the EXACT production DDL + query strings (`CustomDictionaryService` companion
// constants) against an in-memory JDBC SQLite. The native derivation parity
// (DeriveCustomSearchKeys / DeriveCustomQueryKey producing the family/form/key
// rows) is covered by the Rust `engine/phonetics/src/custom_search.rs` tests +
// device dogfood — JVM can't load the `.so` (same constraint as the NextWord
// S11 round).

package com.siansiansu.taigikeyboard.ime.dictionary

import org.junit.Assert.assertEquals
import org.junit.Test
import java.sql.Connection

class CustomDictionaryServiceCrossModeTest {
    /** Run the production `SEARCH_SQL` and return the matched entry ids in order. */
    private fun search(
        conn: Connection,
        family: String,
        form: String,
        key: String,
        limit: Int = 50,
    ): List<String> {
        val ids = mutableListOf<String>()
        conn.prepareStatement(CustomDictionaryService.SEARCH_SQL).use { ps ->
            ps.setString(1, family)
            ps.setString(2, form)
            ps.setString(3, key)
            ps.setInt(4, limit)
            ps.executeQuery().use { rs ->
                while (rs.next()) {
                    ids.add(rs.getString(1))
                }
            }
        }
        return ids
    }

    /**
     * The 食 entry is stored once but indexed under all three families; a query
     * in ANY family (tl / poj / tps) joins back to the same entry. Pre-R3 a
     * POJ-stored entry hard-missed a TL/TPS query — this is the fix.
     */
    @Test
    fun INVARIANT_CUSTOM_DICT_CROSS_MODE_anyFamilyKeyJoinsToEntry() {
        openCustomDictionarySchema().use { conn ->
            conn.insertEntry("e1", "chiah", "食")
            conn.insertSearchKey("e1", "tl", "notone", "tsiah")
            conn.insertSearchKey("e1", "poj", "notone", "chiah")
            conn.insertSearchKey("e1", "tps", "notone", "ㄐㄧㄚㆷ")

            // TL-family query finds the entry via the tl/notone key.
            assertEquals(listOf("e1"), search(conn, family = "tl", form = "notone", key = "tsiah"))
            // POJ-family query finds the same entry via the poj/notone key.
            assertEquals(listOf("e1"), search(conn, family = "poj", form = "notone", key = "chiah"))
            // TPS-family query finds the same entry via the tps/notone key.
            assertEquals(listOf("e1"), search(conn, family = "tps", form = "notone", key = "ㄐㄧㄚㆷ"))
            // Wrong family → no match (the key only exists under its own family).
            assertEquals(emptyList<String>(), search(conn, family = "tl", form = "notone", key = "chiah"))
        }
    }

    /**
     * `form IN (?, 'abbrev')` — an abbrev-form row is matched by a query whose
     * primary form is `notone`. A two-syllable entry's `gs` abbrev surfaces it.
     */
    @Test
    fun INVARIANT_CUSTOM_DICT_CROSS_MODE_abbrevFormAlwaysMatched() {
        openCustomDictionarySchema().use { conn ->
            conn.insertEntry("e2", "gâu-tsá", "𠢕早")
            conn.insertSearchKey("e2", "tl", "notone", "gautsa")
            conn.insertSearchKey("e2", "tl", "abbrev", "gs")

            // Primary-form (notone) query matches the abbrev row.
            assertEquals(listOf("e2"), search(conn, family = "tl", form = "notone", key = "gs"))
            // Primary-form query also still matches the notone row.
            assertEquals(listOf("e2"), search(conn, family = "tl", form = "notone", key = "gau"))
        }
    }

    /**
     * §50 (USER 2026-09-21) — the v9 → v10 arm on the dev-only v9 shape: the
     * learned row and its side keys go, the manual row and its keys stay, the
     * partial index is dropped. (A released v8 DB never runs the arm — the
     * `oldVersion == 9` gate in `onUpgrade`; its table has no `origin`.)
     */
    @Test
    fun migrationV9ToV10_dropsLearnedRowsAndKeepsManualRows() {
        openDevV9CustomDictionarySchema().use { conn ->
            conn.insertEntry("m1", "tâi-gí", "台語")
            conn.insertSearchKey("m1", "tl", "notone", "taigi")
            conn.createStatement().use { st ->
                st.executeUpdate("INSERT INTO custom_dictionary (id, roman, hanzi, origin, learn_count) VALUES ('l1', 'kì--khí-lâi', '記起來', 1, 3)")
            }
            conn.insertSearchKey("l1", "tl", "notone", "kikhilai")

            conn.createStatement().use { st -> CustomDictionaryService.MIGRATE_V9_TO_V10_SQL.forEach(st::executeUpdate) }

            assertEquals(listOf("m1"), conn.queryIds("SELECT id FROM custom_dictionary ORDER BY id"))
            assertEquals(listOf("m1"), conn.queryIds("SELECT entry_id FROM custom_search_key ORDER BY entry_id"))
            assertEquals(
                emptyList<String>(),
                conn.queryIds("SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_custom_learned_pair'"),
            )
            assertEquals(listOf("m1"), search(conn, family = "tl", form = "notone", key = "taigi"))
        }
    }

    /** `LIKE ? || '%'` is a prefix match; `DISTINCT` collapses multi-row joins. */
    @Test
    fun INVARIANT_CUSTOM_DICT_CROSS_MODE_prefixMatchDistinctEntry() {
        openCustomDictionarySchema().use { conn ->
            conn.insertEntry("e3", "tâi-gí", "台語")
            conn.insertSearchKey("e3", "tl", "notone", "taigi")
            conn.insertSearchKey("e3", "tl", "abbrev", "tg")

            // Prefix "tai" matches the notone key; DISTINCT keeps it a single row
            // even though both side rows belong to the same entry.
            assertEquals(listOf("e3"), search(conn, family = "tl", form = "notone", key = "tai"))
            // Non-matching prefix → empty.
            assertEquals(emptyList<String>(), search(conn, family = "tl", form = "notone", key = "xyz"))
        }
    }
}
