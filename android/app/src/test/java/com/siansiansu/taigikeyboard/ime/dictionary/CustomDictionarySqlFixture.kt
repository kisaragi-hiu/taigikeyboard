// In-memory JDBC SQLite carrying the EXACT `custom_dictionary.db` production
// DDL (`CustomDictionaryService` companion constants), so JVM tests run the
// live SQL strings without Android's `SQLiteDatabase`.
package com.siansiansu.taigikeyboard.ime.dictionary

import java.sql.Connection
import java.sql.DriverManager

/** The schema `DatabaseHelper.onCreate` produces: main table, side table + indexes. */
internal fun openCustomDictionarySchema(): Connection {
    val conn = DriverManager.getConnection("jdbc:sqlite::memory:")
    conn.createStatement().use { stmt ->
        stmt.executeUpdate(CustomDictionaryService.CREATE_TABLE_SQL)
        stmt.executeUpdate(CustomDictionaryService.CREATE_SEARCH_KEY_TABLE_SQL)
        stmt.executeUpdate(CustomDictionaryService.CREATE_SEARCH_KEY_LOOKUP_INDEX_SQL)
        stmt.executeUpdate(CustomDictionaryService.CREATE_SEARCH_KEY_ENTRY_INDEX_SQL)
    }
    return conn
}

/**
 * The dev-only v9 shape (#111, never released): the v8 tables plus
 * `origin` / `learn_count` and the partial learned-pair index — what the
 * v9 → v10 arm has to clean.
 */
internal fun openDevV9CustomDictionarySchema(): Connection =
    openCustomDictionarySchema().also { conn ->
        conn.createStatement().use { stmt ->
            stmt.executeUpdate("ALTER TABLE custom_dictionary ADD COLUMN origin INTEGER NOT NULL DEFAULT 0")
            stmt.executeUpdate("ALTER TABLE custom_dictionary ADD COLUMN learn_count INTEGER NOT NULL DEFAULT 0")
            stmt.executeUpdate("CREATE UNIQUE INDEX idx_custom_learned_pair ON custom_dictionary(hanzi, roman) WHERE origin = 1")
        }
    }

internal fun Connection.insertEntry(
    id: String,
    roman: String,
    hanzi: String,
) {
    prepareStatement("INSERT INTO custom_dictionary (id, roman, hanzi) VALUES (?, ?, ?)").use { ps ->
        ps.setString(1, id)
        ps.setString(2, roman)
        ps.setString(3, hanzi)
        ps.executeUpdate()
    }
}

internal fun Connection.insertSearchKey(
    entryId: String,
    family: String,
    form: String,
    key: String,
) {
    prepareStatement("INSERT INTO custom_search_key (entry_id, family, form, key) VALUES (?, ?, ?, ?)").use { ps ->
        ps.setString(1, entryId)
        ps.setString(2, family)
        ps.setString(3, form)
        ps.setString(4, key)
        ps.executeUpdate()
    }
}

/** First column of every row `sql` returns, with positional `String` / `Int` binds. */
internal fun Connection.queryIds(
    sql: String,
    vararg args: Any,
): List<String> {
    val out = mutableListOf<String>()
    prepareStatement(sql).use { ps ->
        args.forEachIndexed { i, arg ->
            when (arg) {
                is Int -> ps.setInt(i + 1, arg)
                else -> ps.setString(i + 1, arg.toString())
            }
        }
        ps.executeQuery().use { rs -> while (rs.next()) out.add(rs.getString(1)) }
    }
    return out
}
