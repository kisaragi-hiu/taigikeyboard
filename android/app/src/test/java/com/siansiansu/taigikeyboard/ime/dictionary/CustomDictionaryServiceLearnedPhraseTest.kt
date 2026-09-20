// Learned phrases (§50) on `custom_dictionary.db` — the SQL the service and
// the capacity policy run, exercised as the EXACT production strings against
// an in-memory JDBC SQLite (Android's SQLiteDatabase is unavailable in JVM
// unit tests; the same constraint as `CustomDictionaryServiceCrossModeTest`).
// Pins: the learn UPDATE / INSERT pair folds a repeat into one row; the
// learned-pair unique index rejects a duplicate; the prefix search is
// manual-only and the exact learned query learned-only; eviction past the
// cap spares the row just written. Behaviour on device: S62 dogfood.
package com.siansiansu.taigikeyboard.ime.dictionary

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.sql.Connection
import java.sql.SQLException

class CustomDictionaryServiceLearnedPhraseTest {
    /** The service's learn pair: UPDATE, then INSERT only when nothing was bumped. */
    private fun learn(
        conn: Connection,
        id: String,
        hanzi: String,
        roman: String,
        count: Int = 1,
    ) {
        val bumped =
            conn.prepareStatement(CustomDictionaryService.LEARN_UPDATE_SQL).use { ps ->
                ps.setInt(1, count)
                ps.setString(2, hanzi)
                ps.setString(3, roman)
                ps.executeUpdate()
            }
        if (bumped == 0) {
            conn.prepareStatement(CustomDictionaryService.LEARN_INSERT_SQL).use { ps ->
                ps.setInt(1, count)
                ps.setString(2, hanzi)
                ps.setString(3, roman)
                ps.setString(4, id)
                ps.setString(5, "")
                ps.setString(6, "")
                ps.setString(7, "")
                ps.executeUpdate()
            }
        }
    }

    private fun learnedRows(conn: Connection): List<Pair<String, Int>> {
        val out = mutableListOf<Pair<String, Int>>()
        conn.createStatement().use { st ->
            st.executeQuery("SELECT hanzi, learn_count FROM custom_dictionary WHERE origin = 1 ORDER BY hanzi").use { rs ->
                while (rs.next()) out.add(rs.getString(1) to rs.getInt(2))
            }
        }
        return out
    }

    @Test
    fun learnPair_foldsARepeatIntoOneRowAndClampsTheCount() {
        openCustomDictionarySchema().use { conn ->
            learn(conn, "l1", "記起來", "kì--khí-lâi")
            learn(conn, "l2", "記起來", "kì--khí-lâi")
            assertEquals(listOf("記起來" to 2), learnedRows(conn))
            learn(conn, "l3", "記起來", "kì--khí-lâi", count = Int.MAX_VALUE)
            assertEquals(listOf("記起來" to CustomDictionaryService.MAX_LEARN_COUNT), learnedRows(conn))
        }
    }

    @Test
    fun learnedPairIndex_rejectsASecondLearnedRowForThePair_butNotAManualOne() {
        openCustomDictionarySchema().use { conn ->
            learn(conn, "l1", "記起來", "kì--khí-lâi")
            assertThrows(SQLException::class.java) {
                conn.prepareStatement(CustomDictionaryService.LEARN_INSERT_SQL).use { ps ->
                    ps.setInt(1, 1)
                    ps.setString(2, "記起來")
                    ps.setString(3, "kì--khí-lâi")
                    ps.setString(4, "l2")
                    ps.setString(5, "")
                    ps.setString(6, "")
                    ps.setString(7, "")
                    ps.executeUpdate()
                }
            }
            // Manual rows keep their duplicate tolerance (partial index).
            conn.insertEntry("m1", "kì--khí-lâi", "記起來")
            conn.insertEntry("m2", "kì--khí-lâi", "記起來")
        }
    }

    @Test
    fun prefixSearchIsManualOnly_exactLearnedSearchIsLearnedOnly() {
        openCustomDictionarySchema().use { conn ->
            learn(conn, "l1", "記起來", "kì--khí-lâi")
            conn.insertSearchKey("l1", "tl", "notone", "kikhilai")
            conn.insertEntry("m1", "kì-khí", "記起")
            conn.insertSearchKey("m1", "tl", "notone", "kikhi")

            assertEquals(listOf("m1"), conn.queryIds(CustomDictionaryService.SEARCH_SQL, "tl", "notone", "ki", 20))
            assertEquals(listOf("l1"), conn.queryIds(CustomDictionaryService.LEARNED_EXACT_SQL, "tl", "notone", "kikhilai", 5))
            assertEquals(emptyList<String>(), conn.queryIds(CustomDictionaryService.LEARNED_EXACT_SQL, "tl", "notone", "kikhi", 5))
            assertEquals(emptyList<String>(), conn.queryIds(CustomDictionaryService.LEARNED_EXACT_SQL, "tl", "notone", "ki", 5))
        }
    }

    @Test
    fun evictionPastTheCap_dropsFewestComposedFirst_andSparesTheKeptRow() {
        openCustomDictionarySchema().use { conn ->
            learn(conn, "l1", "詞一", "su-it")
            learn(conn, "l2", "詞二", "su-jī")
            learn(conn, "l2", "詞二", "su-jī")
            learn(conn, "l3", "詞三", "su-sann")
            // cap 2 → one row past the cap once l3 (just written) is set aside;
            // the count-1 row l1 goes, not the count-2 row l2.
            val victims = conn.queryIds(CustomDictionaryCapacityPolicy.LEARNED_PAST_CAP_SQL, "l3", 1)
            assertEquals(listOf("l1"), victims)
            // Under the cap → nothing.
            assertEquals(emptyList<String>(), conn.queryIds(CustomDictionaryCapacityPolicy.LEARNED_PAST_CAP_SQL, "l3", 5))
        }
    }
}
