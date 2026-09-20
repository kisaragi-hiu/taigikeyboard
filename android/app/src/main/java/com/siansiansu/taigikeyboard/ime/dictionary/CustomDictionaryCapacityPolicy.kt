package com.siansiansu.taigikeyboard.ime.dictionary

import android.database.sqlite.SQLiteDatabase

/**
 * Capacity policy for the custom-dictionary table. Owns the row-count cap
 * ([MAX_ENTRIES]) and the helpers needed to enforce it inside the same
 * transaction as the write (TOCTOU-safe). Stateless — every method operates on
 * a caller-provided [SQLiteDatabase], mirroring iOS `CustomDictionaryCapacityPolicy`
 * (which takes an `OpaquePointer`). Platform DB-policy code, NOT shared-core.
 *
 * The pure decision helpers ([wouldExceedCap], [remainingCapacity]) are the
 * JVM-testable seam — they take a row count instead of a DB handle, so the
 * boundary can be exercised without inserting 30000 rows.
 */
internal object CustomDictionaryCapacityPolicy {
    // CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Lexicon/Database/CustomDictionaryCapacityPolicy.swift (maxEntries).
    // Drift causes silent divergence.
    /** Maximum number of MANUAL rows (`origin = 0`). */
    const val MAX_ENTRIES = 30_000

    /**
     * Maximum number of LEARNED rows (`origin = 1`, §50); past it the
     * fewest-composed, then least recently touched, row goes so a learn
     * never fails.
     */
    // CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Lexicon/Database/CustomDictionaryCapacityPolicy.swift (maxLearnedEntries).
    // Drift causes silent divergence.
    const val MAX_LEARNED_ENTRIES = 2_000

    private const val TABLE_NAME = "custom_dictionary"

    /** MANUAL row count — learned rows have their own quota. 0 on query failure ("not full"). */
    fun currentEntryCount(db: SQLiteDatabase): Int =
        db.rawQuery("SELECT COUNT(*) FROM $TABLE_NAME WHERE origin = 0", null).use {
            if (it.moveToFirst()) it.getInt(0) else 0
        }

    /** True when a MANUAL row with [id] exists — adopting a learned row is a new manual row for the quota. */
    fun entryExists(
        db: SQLiteDatabase,
        id: String,
    ): Boolean =
        db.rawQuery("SELECT 1 FROM $TABLE_NAME WHERE id = ? AND origin = 0 LIMIT 1", arrayOf(id)).use {
            it.moveToFirst()
        }

    /**
     * Drop learned rows past [cap], never [keptId] (the row the caller just
     * wrote). Side keys first, so the subquery still resolves against the
     * intact main table; `OFFSET cap - 1` selects exactly the rows past the
     * cap once the kept row is set aside. Caller holds the transaction.
     */
    // CROSS-PLATFORM INVARIANT — mirrors ios CustomDictionaryCapacityPolicy.evictLearnedPastCap. Drift causes silent divergence.
    fun evictLearnedPastCap(
        db: SQLiteDatabase,
        keptId: String,
        cap: Int = MAX_LEARNED_ENTRIES,
    ) {
        val args = arrayOf<Any>(keptId, (cap - 1).coerceAtLeast(0))
        db.execSQL("DELETE FROM custom_search_key WHERE entry_id IN ($LEARNED_PAST_CAP_SQL)", args)
        db.execSQL("DELETE FROM $TABLE_NAME WHERE id IN ($LEARNED_PAST_CAP_SQL)", args)
    }

    /** The learned rows past the cap, excluding one kept id — `internal` for the JVM SQL test. */
    internal const val LEARNED_PAST_CAP_SQL =
        "SELECT id FROM $TABLE_NAME WHERE origin = 1 AND id <> ? " +
            "ORDER BY learn_count DESC, updated_at DESC, id LIMIT -1 OFFSET ?"

    /**
     * Pure decision: would inserting push past the cap? Updating an existing row
     * ([isExistingRow] = true) is not an insert and never exceeds.
     */
    fun wouldExceedCap(
        currentCount: Int,
        isExistingRow: Boolean,
    ): Boolean = !isExistingRow && currentCount >= MAX_ENTRIES

    /** Remaining capacity headroom. Negative values clamp to 0. */
    fun remainingCapacity(currentCount: Int): Int = (MAX_ENTRIES - currentCount).coerceAtLeast(0)

    /**
     * Throw [CustomDictionaryFullException] if inserting a NEW [id] would exceed
     * [MAX_ENTRIES]. Updating an existing row bypasses the check. Must run in the
     * same transaction as the write to avoid a TOCTOU race with concurrent writers.
     */
    fun guardInsertCapacity(
        db: SQLiteDatabase,
        id: String,
    ) {
        if (wouldExceedCap(currentEntryCount(db), entryExists(db, id))) {
            throw CustomDictionaryFullException(MAX_ENTRIES)
        }
    }
}

/** Thrown when a custom-dictionary insert would exceed the row cap. */
class CustomDictionaryFullException(
    maxEntries: Int,
) : Exception("Custom dictionary is full (max $maxEntries entries)")
