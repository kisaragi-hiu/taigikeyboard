// Learned phrases (§50) on `learned_phrases.db` — the SQL `LearnedPhraseService`
// runs, exercised as the EXACT production strings against an in-memory JDBC
// SQLite (Android's SQLiteDatabase is unavailable in JVM unit tests; the same
// constraint as `CustomDictionaryServiceCrossModeTest`). Pins: the learn
// UPDATE / INSERT pair folds a repeat into one row and the UNIQUE constraint
// rejects a duplicate; the exact query matches the whole buffer only, most
// composed first; eviction past the cap spares the row just written; the
// side-key unique index absorbs a repeated key. Behaviour on device: S62.
package com.siansiansu.taigikeyboard.ime.dictionary

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.sql.Connection
import java.sql.DriverManager
import java.sql.SQLException

class LearnedPhraseServiceSqlTest {
    /** The schema `DatabaseHelper.onCreate` produces. */
    private fun openSchema(): Connection {
        val conn = DriverManager.getConnection("jdbc:sqlite::memory:")
        conn.createStatement().use { stmt ->
            stmt.executeUpdate(LearnedPhraseService.CREATE_TABLE_SQL)
            stmt.executeUpdate(LearnedPhraseService.CREATE_SEARCH_KEY_TABLE_SQL)
            stmt.executeUpdate(LearnedPhraseService.CREATE_SEARCH_KEY_LOOKUP_INDEX_SQL)
            stmt.executeUpdate(LearnedPhraseService.CREATE_SEARCH_KEY_PHRASE_INDEX_SQL)
            stmt.executeUpdate(LearnedPhraseService.CREATE_RANK_INDEX_SQL)
        }
        return conn
    }

    /** The service's learn pair: UPDATE, then INSERT only when nothing was bumped. Returns the phrase id. */
    private fun learn(
        conn: Connection,
        hanzi: String,
        roman: String,
    ): Long {
        val bumped =
            conn.prepareStatement(LearnedPhraseService.LEARN_UPDATE_SQL).use { ps ->
                ps.setString(1, hanzi)
                ps.setString(2, roman)
                ps.executeUpdate()
            }
        if (bumped == 0) {
            conn.prepareStatement(LearnedPhraseService.LEARN_INSERT_SQL).use { ps ->
                ps.setString(1, hanzi)
                ps.setString(2, roman)
                ps.executeUpdate()
            }
        }
        return idOf(conn, hanzi, roman)
    }

    private fun idOf(
        conn: Connection,
        hanzi: String,
        roman: String,
    ): Long =
        conn.prepareStatement("SELECT id FROM learned_phrases WHERE hanzi = ? AND roman = ?").use { ps ->
            ps.setString(1, hanzi)
            ps.setString(2, roman)
            ps.executeQuery().use { rs ->
                check(rs.next())
                rs.getLong(1)
            }
        }

    private fun insertKey(
        conn: Connection,
        phraseId: Long,
        family: String,
        form: String,
        key: String,
    ) {
        conn.prepareStatement(LearnedPhraseService.INSERT_SEARCH_KEY_SQL).use { ps ->
            ps.setLong(1, phraseId)
            ps.setString(2, family)
            ps.setString(3, form)
            ps.setString(4, key)
            ps.executeUpdate()
        }
    }

    private fun rows(conn: Connection): List<Pair<String, Int>> {
        val out = mutableListOf<Pair<String, Int>>()
        conn.createStatement().use { st ->
            st.executeQuery("SELECT hanzi, learn_count FROM learned_phrases ORDER BY hanzi").use { rs ->
                while (rs.next()) out.add(rs.getString(1) to rs.getInt(2))
            }
        }
        return out
    }

    private fun exactMatches(
        conn: Connection,
        key: String,
    ): List<String> =
        conn.prepareStatement(LearnedPhraseService.EXACT_MATCH_SQL).use { ps ->
            ps.setString(1, "tl")
            ps.setString(2, "notone")
            ps.setString(3, key)
            ps.setInt(4, 5)
            ps.executeQuery().use { rs ->
                val out = mutableListOf<String>()
                while (rs.next()) out.add(rs.getString(1))
                out
            }
        }

    @Test
    fun learnPair_foldsARepeatIntoOneRow_andTheUniqueConstraintRejectsADuplicate() {
        openSchema().use { conn ->
            learn(conn, "記起來", "kì--khí-lâi")
            learn(conn, "記起來", "kì--khí-lâi")
            assertEquals(listOf("記起來" to 2), rows(conn))
            // The UPDATE alone (the touch path) bumps a known pair and is a
            // no-op for an unknown one; the count saturates at MAX_LEARN_COUNT.
            conn.createStatement().use { st ->
                st.executeUpdate("UPDATE learned_phrases SET learn_count = ${LearnedPhraseService.MAX_LEARN_COUNT}")
            }
            learn(conn, "記起來", "kì--khí-lâi")
            conn.prepareStatement(LearnedPhraseService.LEARN_UPDATE_SQL).use { ps ->
                ps.setString(1, "台語")
                ps.setString(2, "tâi-gí")
                assertEquals(0, ps.executeUpdate())
            }
            assertEquals(listOf("記起來" to LearnedPhraseService.MAX_LEARN_COUNT), rows(conn))
            assertThrows(SQLException::class.java) {
                conn.prepareStatement(LearnedPhraseService.LEARN_INSERT_SQL).use { ps ->
                    ps.setString(1, "記起來")
                    ps.setString(2, "kì--khí-lâi")
                    ps.executeUpdate()
                }
            }
        }
    }

    @Test
    fun exactMatch_matchesTheWholeBufferOnly_mostComposedFirst() {
        openSchema().use { conn ->
            val ki = learn(conn, "機起來", "ki-khí-lâi")
            insertKey(conn, ki, "tl", "notone", "kikhilai")
            val kii = learn(conn, "記起來", "kì--khí-lâi")
            learn(conn, "記起來", "kì--khí-lâi")
            insertKey(conn, kii, "tl", "notone", "kikhilai")
            // The side-key unique index absorbs a repeated key.
            insertKey(conn, kii, "tl", "notone", "kikhilai")

            assertEquals(listOf("記起來", "機起來"), exactMatches(conn, "kikhilai"))
            assertEquals(emptyList<String>(), exactMatches(conn, "kikhi"))
            assertEquals(emptyList<String>(), exactMatches(conn, "kikhilaia"))
        }
    }

    @Test
    fun pastCap_dropsFewestComposedFirst_andSparesTheKeptRow() {
        openSchema().use { conn ->
            val one = learn(conn, "詞一", "su-it")
            insertKey(conn, one, "tl", "notone", "suit")
            learn(conn, "詞二", "su-jī")
            learn(conn, "詞二", "su-jī")
            val three = learn(conn, "詞三", "su-sann")
            // cap 2 → one row past the cap once 詞三 (just written) is set aside;
            // the count-1 row 詞一 goes, not the count-2 row 詞二.
            assertEquals(listOf(one), pastCap(conn, keptId = three, cap = 2))
            // Under the cap → nothing.
            assertEquals(emptyList<Long>(), pastCap(conn, keptId = three, cap = 5))
            // The service's eviction DELETEs, keys first, then rows.
            for (sql in listOf(LearnedPhraseService.EVICT_KEYS_SQL, LearnedPhraseService.EVICT_ROWS_SQL)) {
                conn.prepareStatement(sql).use { ps ->
                    ps.setLong(1, three)
                    ps.setInt(2, 1)
                    ps.executeUpdate()
                }
            }
            assertEquals(listOf("詞三" to 1, "詞二" to 2), rows(conn))
            assertEquals(emptyList<String>(), exactMatches(conn, "suit"))
        }
    }

    @Test
    fun wipe_clearsRowsAndKeys_andTheStoreLearnsAgain() {
        openSchema().use { conn ->
            val id = learn(conn, "記起來", "kì--khí-lâi")
            insertKey(conn, id, "tl", "notone", "kikhilai")
            conn.createStatement().use { st -> LearnedPhraseService.WIPE_SQL.forEach(st::executeUpdate) }
            assertEquals(emptyList<Pair<String, Int>>(), rows(conn))
            assertEquals(emptyList<String>(), exactMatches(conn, "kikhilai"))
            val again = learn(conn, "記起來", "kì--khí-lâi")
            insertKey(conn, again, "tl", "notone", "kikhilai")
            assertEquals(listOf("記起來"), exactMatches(conn, "kikhilai"))
        }
    }

    private fun pastCap(
        conn: Connection,
        keptId: Long,
        cap: Int,
    ): List<Long> =
        conn.prepareStatement(LearnedPhraseService.PAST_CAP_SQL).use { ps ->
            ps.setLong(1, keptId)
            ps.setInt(2, cap - 1)
            ps.executeQuery().use { rs ->
                val out = mutableListOf<Long>()
                while (rs.next()) out.add(rs.getLong(1))
                out
            }
        }
}
