// Pins the UPSERT semantics the `upsert` UPDATE + INSERT OR IGNORE pairs
// replace (SQLite 3.22 ceiling, android-guidelines.md §8a): absent row →
// inserted; present row → updated in place (same rowid, `created_at` kept);
// batch merge → MAX(count). Runs the EXACT production DDL + pair strings
// against an in-memory JDBC SQLite; Android's SQLiteDatabase is unavailable
// in JVM unit tests, so the pair is driven the way `upsert` drives it.

package com.siansiansu.taigikeyboard.ime.core.db

import com.siansiansu.taigikeyboard.ime.dictionary.CustomDictionaryService
import com.siansiansu.taigikeyboard.ime.dictionary.NextWordService
import com.siansiansu.taigikeyboard.ime.text.composing.UserFrequencyService
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test
import java.sql.Connection
import java.sql.DriverManager

class SqliteUpsertPairTest {
    private fun open(ddl: String): Connection =
        DriverManager.getConnection("jdbc:sqlite::memory:").also { conn ->
            conn.createStatement().use { it.executeUpdate(ddl) }
        }

    /** Executes [sql] with positional [args]; returns the changed-row count. */
    private fun Connection.exec(
        sql: String,
        vararg args: Any,
    ): Int =
        prepareStatement(sql).use { stmt ->
            args.forEachIndexed { i, arg ->
                when (arg) {
                    is Long -> stmt.setLong(i + 1, arg)
                    else -> stmt.setString(i + 1, arg.toString())
                }
            }
            stmt.executeUpdate()
        }

    /** Mirrors `upsert`: UPDATE, and INSERT only when the UPDATE matched nothing. */
    private fun Connection.upsert(
        updateSql: String,
        insertSql: String,
        vararg args: Any,
    ) {
        if (exec(updateSql, *args) == 0) exec(insertSql, *args)
    }

    private fun Connection.row(sql: String): List<Any?> =
        createStatement().use { stmt ->
            stmt.executeQuery(sql).use { rs ->
                check(rs.next()) { "no row for: $sql" }
                (1..rs.metaData.columnCount).map { rs.getObject(it) }
            }
        }

    // ---- user_association (NextWordService) ----

    private val assocDdl = NextWordService.userAssocTableSql("user_association")

    @Test
    fun `association record inserts once then increments in place`() {
        open(assocDdl).use { db ->
            repeat(3) {
                db.upsert(NextWordService.RECORD_ASSOCIATION_UPDATE_SQL, NextWordService.RECORD_ASSOCIATION_INSERT_SQL, "重", "tîng", "複", "hok")
            }
            // trace: absent → UPDATE 0 rows, INSERT id=1 count=1; ×2 → UPDATE count 2, 3; no INSERT
            assertEquals(listOf<Any?>(1, 1, 3), db.row("SELECT COUNT(*), id, count FROM user_association"))
        }
    }

    @Test
    fun `association batch import merges by max count`() {
        open(assocDdl).use { db ->
            fun import(count: Long) =
                db.upsert(
                    NextWordService.BATCH_IMPORT_ASSOCIATION_UPDATE_SQL,
                    NextWordService.BATCH_IMPORT_ASSOCIATION_INSERT_SQL,
                    count,
                    "重",
                    "tîng",
                    "複",
                    "hok",
                )
            import(5) // absent → inserted with 5
            import(3) // present → MAX(5, 3) = 5
            import(9) // present → MAX(5, 9) = 9
            assertEquals(listOf<Any?>(1, 1, 9), db.row("SELECT COUNT(*), id, count FROM user_association"))
        }
    }

    // ---- user_frequency (UserFrequencyService) ----

    private val freqDdl = UserFrequencyService.CREATE_TABLE_SQL

    @Test
    fun `frequency record keeps one row per reading and increments`() {
        open(freqDdl).use { db ->
            repeat(2) { db.upsert(UserFrequencyService.RECORD_USAGE_UPDATE_SQL, UserFrequencyService.RECORD_USAGE_INSERT_SQL, "重", "tîng") }
            db.upsert(UserFrequencyService.RECORD_USAGE_UPDATE_SQL, UserFrequencyService.RECORD_USAGE_INSERT_SQL, "重", "tāng")
            // trace: 重/tîng → id=1 count=2; 重/tāng → separate bucket id=2 count=1
            assertEquals(listOf<Any?>(1, "tîng", 2), db.row("SELECT id, tl, count FROM user_frequency WHERE tl = 'tîng'"))
            assertEquals(listOf<Any?>(2, "tāng", 1), db.row("SELECT id, tl, count FROM user_frequency WHERE tl = 'tāng'"))
        }
    }

    @Test
    fun `frequency batch import merges by max count`() {
        open(freqDdl).use { db ->
            fun import(count: Long) = db.upsert(UserFrequencyService.BATCH_IMPORT_UPDATE_SQL, UserFrequencyService.BATCH_IMPORT_INSERT_SQL, count, "重", "tîng")
            import(5)
            import(3)
            assertEquals(listOf<Any?>(1, 5), db.row("SELECT id, count FROM user_frequency"))
            import(9)
            assertEquals(listOf<Any?>(1, 9), db.row("SELECT id, count FROM user_frequency"))
        }
    }

    // ---- custom_dictionary (CustomDictionaryService) ----

    @Test
    fun `custom entry upsert replaces fields by id and keeps created_at`() {
        open(CustomDictionaryService.CREATE_TABLE_SQL).use { db ->
            fun upsert(roman: String) = db.upsert(CustomDictionaryService.UPSERT_UPDATE_SQL, CustomDictionaryService.UPSERT_INSERT_SQL, roman, "台語", "notone", "abbrev", "num", "id-1")
            upsert("tâi-gí")
            // Pin old timestamps so the second write is observable.
            db.exec("UPDATE custom_dictionary SET created_at = '2000-01-01 00:00:00', updated_at = '2000-01-01 00:00:00'")
            upsert("tâi-gú")
            // trace: UPDATE hits id-1 → roman replaced, created_at untouched, updated_at = now; no INSERT
            val (rows, roman, createdAt, updatedAt) = db.row("SELECT COUNT(*), roman, created_at, updated_at FROM custom_dictionary")
            assertEquals(1, rows)
            assertEquals("tâi-gú", roman)
            assertEquals("2000-01-01 00:00:00", createdAt)
            assertNotEquals("2000-01-01 00:00:00", updatedAt)
        }
    }
}
