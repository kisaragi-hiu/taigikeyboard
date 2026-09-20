import Foundation
import SQLite3

/// Capacity policy for the custom-dictionary table — two quotas, one per
/// `CustomDictionaryEntry.Origin`.
///
/// The MANUAL quota (`maxEntries`, aligned with Android's `MAX_ENTRIES`) is a
/// hard cap the user runs into; the LEARNED quota (`maxLearnedEntries`, §50)
/// is enforced by eviction so learning never fails. Helpers run inside a
/// serialized `SQLiteConnectionManager.execute { db in ... }` block.
/// Stateless — all methods operate on a caller-provided `OpaquePointer`.
enum CustomDictionaryCapacityPolicy {
    /// Maximum number of MANUAL rows (`origin = 0`).
    // CROSS-PLATFORM INVARIANT — mirrors android/app/src/main/java/com/siansiansu/taigikeyboard/ime/dictionary/CustomDictionaryCapacityPolicy.kt (MAX_ENTRIES).
    // Drift causes silent divergence.
    static let maxEntries = 30000

    /// Maximum number of LEARNED rows (`origin = 1`); past it the
    /// fewest-composed, then least recently touched, row goes (ChiaKey's
    /// policy) so a learn never fails.
    // CROSS-PLATFORM INVARIANT — mirrors android/app/src/main/java/com/siansiansu/taigikeyboard/ime/dictionary/CustomDictionaryCapacityPolicy.kt (MAX_LEARNED_ENTRIES).
    // Drift causes silent divergence.
    static let maxLearnedEntries = 2000

    /// Drop learned rows past `cap` — side keys first, so the subquery still
    /// resolves against the intact main table. `OFFSET cap` selects exactly
    /// the rows past the cap (none when under it), so no count is needed.
    static func evictLearnedPastCap(db: OpaquePointer, cap: Int = maxLearnedEntries) {
        let pastCap = """
            SELECT id FROM \(CustomDictionarySchema.tableName)
            WHERE origin = 1
            ORDER BY learn_count DESC, updated_at DESC, id
            LIMIT -1 OFFSET ?
        """
        for table in [
            "\(CustomDictionarySchema.searchKeyTableName) WHERE \(CustomDictionarySchema.searchKeyEntryIdColumn) IN (\(pastCap))",
            "\(CustomDictionarySchema.tableName) WHERE id IN (\(pastCap))",
        ] {
            var stmt: OpaquePointer?
            guard sqlite3_prepare_v2(db, "DELETE FROM \(table);", -1, &stmt, nil) == SQLITE_OK else { continue }
            sqlite3_bind_int(stmt, 1, Int32(cap))
            sqlite3_step(stmt)
            sqlite3_finalize(stmt)
        }
    }

    /// True when a row with the given `id` already exists.
    static func entryExists(db: OpaquePointer, id: String) -> Bool {
        var stmt: OpaquePointer?
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_prepare_v2(
            db,
            "SELECT 1 FROM \(CustomDictionarySchema.tableName) WHERE id = ? LIMIT 1;",
            -1, &stmt, nil,
        ) == SQLITE_OK else {
            return false
        }
        stmt.bindText(1, id)
        return sqlite3_step(stmt) == SQLITE_ROW
    }

    /// Current MANUAL row count — learned rows (§50) have their own quota
    /// (`maxLearnedEntries`) and never eat into the user's. Returns 0 on
    /// prepare failure so the caller can treat connection problems as "not
    /// full" — subsequent writes will surface the underlying error.
    static func currentEntryCount(db: OpaquePointer) -> Int {
        (try? sqliteQueryScalarInt(
            db: db,
            "SELECT COUNT(*) FROM \(CustomDictionarySchema.tableName) WHERE origin = 0;",
        )) ?? 0
    }

    /// Throw if inserting would exceed `maxEntries`. Updating an existing
    /// row (same `id`) is not an insert and bypasses the check.
    /// Must run inside the same transaction as the write to avoid TOCTOU.
    static func guardInsertCapacity(db: OpaquePointer, id: String) throws {
        if entryExists(db: db, id: id) {
            return
        }
        guard currentEntryCount(db: db) < maxEntries else {
            throw LexiconError.queryExecutionFailed(
                "Custom dictionary is full (max \(maxEntries) entries)",
            )
        }
    }

    /// Remaining capacity headroom. Negative values clamp to 0.
    static func remainingCapacity(db: OpaquePointer) -> Int {
        max(0, maxEntries - currentEntryCount(db: db))
    }
}
