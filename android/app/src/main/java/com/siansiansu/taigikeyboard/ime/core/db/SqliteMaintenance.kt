// SQLite helpers shared across the user-data services.

package com.siansiansu.taigikeyboard.ime.core.db

import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteStatement
import com.siansiansu.taigikeyboard.ime.core.logging.LoggerBackend

/**
 * Reclaim freed pages after a user-initiated bulk clear (R6).
 *
 * Best-effort by design: VACUUM needs exclusive DB access and ~2x the file
 * size in temp space, so a failure (DB locked, low disk) leaves the file
 * larger but fully intact — we just log and move on. MUST be called outside
 * any open transaction and after the clearing DELETE has committed; VACUUM
 * cannot run inside a transaction. Not for the prune hot path — bounded LRU
 * tables reuse their freelist, so VACUUM there would only add latency.
 */
fun vacuumBestEffort(
    db: SQLiteDatabase,
    logger: LoggerBackend,
    tag: String,
) {
    try {
        db.execSQL("VACUUM")
    } catch (e: Exception) {
        logger.w(tag, "vacuum.skipped", e)
    }
}

/**
 * `SELECT COUNT(*)` over [table]; [fallback] is returned when the cursor is
 * empty. [table] is interpolated into the SQL, so it MUST be a hardcoded table
 * constant — never user input (`.claude/rules/security-rules.md` § SQL).
 */
internal fun SQLiteDatabase.rowCount(
    table: String,
    fallback: Int,
): Int =
    rawQuery("SELECT COUNT(*) FROM $table", null).use {
        if (it.moveToFirst()) it.getInt(0) else fallback
    }

/** Bind [args] positionally (1-based); `Int` widens to `INTEGER`. */
internal fun SQLiteStatement.bindArgs(vararg args: Any?) {
    clearBindings()
    args.forEachIndexed { index, arg ->
        val i = index + 1
        when (arg) {
            null -> bindNull(i)
            is String -> bindString(i, arg)
            is Long -> bindLong(i, arg)
            is Int -> bindLong(i, arg.toLong())
            else -> throw IllegalArgumentException("Unsupported bind type: ${arg::class}")
        }
    }
}

/**
 * Upsert without UPSERT: run [update], and only when it matched no row run
 * [insert]. minSdk 28 bundles SQLite 3.22, which predates
 * `ON CONFLICT … DO UPDATE` (3.24) — `.claude/rules/android-guidelines.md`
 * §8a. Both statements bind the same [args] (write the INSERT column list in
 * the UPDATE's bind order). Equivalent to UPSERT given a UNIQUE / PRIMARY
 * KEY on the row key; the UPDATE keeps the rowid and any `created_at`.
 *
 * The caller holds the transaction so no other writer can slip between the
 * two statements — the same thread-safety the single UPSERT statement had.
 *
 * Returns the inserted rowid, or `-1` when the UPDATE matched (nothing was
 * inserted) — for a caller that hangs side rows off a fresh row only.
 */
internal fun upsert(
    update: SQLiteStatement,
    insert: SQLiteStatement,
    vararg args: Any?,
): Long {
    update.bindArgs(*args)
    if (update.executeUpdateDelete() > 0) return -1L
    insert.bindArgs(*args)
    return insert.executeInsert()
}

/** [upsert] for a one-off write; batch loops compile the pair once instead. */
internal fun SQLiteDatabase.upsert(
    updateSql: String,
    insertSql: String,
    vararg args: Any?,
): Long {
    check(inTransaction()) { "upsert needs the caller's transaction" }
    return compileStatement(updateSql).use { update ->
        compileStatement(insertSql).use { insert -> upsert(update, insert, *args) }
    }
}
