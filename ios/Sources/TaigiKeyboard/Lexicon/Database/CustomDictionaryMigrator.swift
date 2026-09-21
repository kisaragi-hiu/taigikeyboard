import Foundation
import SQLite3

/// Custom-dictionary forward data migrations, gated by `PRAGMA user_version`.
///
/// Owns version-gated forward migrations that are not DDL:
/// 1. (v1) Add derived columns (`notone` / `abbrev` / `roman_num`) to
///    databases created before those columns existed + re-derive every row's
///    derived columns so values match the current `CustomDictionaryDerivation`
///    logic (cheap: a single prepared UPDATE reused across rows).
/// 2. (v2) Backfill the `custom_search_key` side table for every existing
///    entry so cross-input-mode search finds pre-v2 entries. Non-destructive —
///    only inserts side rows, never touches `custom_dictionary` user data.
/// 3. (v3) The POJ spelling-glyph fix: `o͘` (U+0358) used to be stripped from
///    a derived key as if it were a tone diacritic and the nasal ⁿ survived
///    into the tone-aware key as a display glyph, while a query key built from
///    the raw keyboard buffer carries the ASCII `oo` / `nn` the user types —
///    so every POJ entry containing either was unreachable from the keyboard
///    until its keys are re-derived.
/// 4. (v6) Learned phrases left this table for `learned_phrases.db` (§50,
///    USER 2026-09-21). A DB that reached the unreleased v5 shape drops the
///    learned-pair index and its `origin = 1` rows (with their side keys),
///    in one checked transaction with the version stamp — a failed step is
///    never stamped as v6 and the next open retries. Derivation did not
///    change, so the O(N) key backfill is skipped for a v4 / v5 database.
///
/// Runs after `CustomDictionarySchema.ensureTables`; callers must serialize
/// access (typically via `SQLiteConnectionManager.execute`).
/// No-op once `PRAGMA user_version` matches `CustomDictionarySchema.schemaVersion`,
/// so the O(N) backfill happens at most once per derivation-logic change
/// instead of on every cold start.
enum CustomDictionaryMigrator {
    /// Apply pending forward migrations. Safe to call repeatedly — fast-path
    /// returns immediately when `PRAGMA user_version` is already current.
    static func runIfNeeded(db: OpaquePointer, logger: DebugLogger) throws {
        let currentVersion = sqliteReadUserVersion(db: db)
        guard currentVersion < CustomDictionarySchema.schemaVersion else { return }

        logger.info("[MIGRATE] custom_dictionary.db v\(currentVersion) -> v\(CustomDictionarySchema.schemaVersion)")

        // Structure — the version branches carry only the DDL each step added;
        // the derived VALUES are then rebuilt once, below. Every reachable
        // version is behind the current derivation (that is what a bump means),
        // so backfilling per branch would only repeat the same work.
        if currentVersion < 1 {
            addMissingDerivedColumns(db: db)
        }

        if currentVersion < 2 {
            // `ensureTables` already created the side table before the migrator
            // ran, but re-create idempotently so the backfill never targets a
            // missing table if call order ever changes.
            try? CustomDictionarySchema.ensureTables(db: db)
        }

        // Derivation last changed in v4; a v4 / v5 database only loses §50 rows.
        if currentVersion < 4 {
            backfillDerivedColumns(db: db)
            backfillSearchKeys(db: db)
        }

        try sqliteTransaction(db: db) {
            if currentVersion == 5 {
                try dropLearnedRows(db: db)
            }
            try sqliteExecChecked(db: db, "PRAGMA user_version = \(CustomDictionarySchema.schemaVersion);")
        }
    }

    // MARK: - Private

    /// v6: the §50 leftovers of a v5 DB — the partial unique index and the
    /// rows its `origin` column marked as learned (with their side keys).
    /// A v5 stamp implies both exist: the v5 step ran its ALTER and stamp in
    /// one transaction.
    private static func dropLearnedRows(db: OpaquePointer) throws {
        let learnedIds = "SELECT id FROM \(CustomDictionarySchema.tableName) WHERE origin = 1"
        try sqliteExecChecked(db: db, "DROP INDEX IF EXISTS idx_custom_learned_pair;")
        try sqliteExecChecked(
            db: db,
            "DELETE FROM \(CustomDictionarySchema.searchKeyTableName) WHERE \(CustomDictionarySchema.searchKeyEntryIdColumn) IN (\(learnedIds));",
        )
        try sqliteExecChecked(db: db, "DELETE FROM \(CustomDictionarySchema.tableName) WHERE origin = 1;")
    }

    /// Add derived columns via `ALTER TABLE` when missing. The whitelist is
    /// required because SQLite DDL cannot parameterize column identifiers.
    private static func addMissingDerivedColumns(db: OpaquePointer) {
        for column in CustomDictionarySchema.derivedColumns
            where !CustomDictionarySchema.columnExists(db: db, column: column)
        {
            sqliteExecSimple(
                db: db,
                "ALTER TABLE \(CustomDictionarySchema.tableName) ADD COLUMN \(column) TEXT DEFAULT '';",
            )
        }
    }

    /// Re-derive `notone` / `abbrev` / `roman_num` for every row. Reuses a
    /// single prepared UPDATE (reset + clear bindings per iteration) so the
    /// cost scales O(N) in rows rather than O(N) in prepared statements.
    private static func backfillDerivedColumns(db: OpaquePointer) {
        var selectStmt: OpaquePointer?
        guard sqlite3_prepare_v2(
            db,
            "SELECT id, roman FROM \(CustomDictionarySchema.tableName);",
            -1, &selectStmt, nil,
        ) == SQLITE_OK else { return }
        defer { sqlite3_finalize(selectStmt) }

        var updateStmt: OpaquePointer?
        let updateSQL = """
            UPDATE \(CustomDictionarySchema.tableName)
            SET notone = ?, abbrev = ?, roman_num = ?
            WHERE id = ?;
        """
        guard sqlite3_prepare_v2(db, updateSQL, -1, &updateStmt, nil) == SQLITE_OK else { return }
        defer { sqlite3_finalize(updateStmt) }

        while sqlite3_step(selectStmt) == SQLITE_ROW {
            let id = String(cString: sqlite3_column_text(selectStmt, 0))
            let roman = String(cString: sqlite3_column_text(selectStmt, 1))

            sqlite3_reset(updateStmt)
            sqlite3_clear_bindings(updateStmt)

            updateStmt.bindText(1, CustomDictionaryDerivation.generateNotone(roman))
            updateStmt.bindText(2, CustomDictionaryDerivation.generateAbbrev(roman))
            updateStmt.bindText(3, CustomDictionaryDerivation.generateRomanNum(roman))
            updateStmt.bindText(4, id)
            sqlite3_step(updateStmt)
        }
    }

    /// Backfill the `custom_search_key` side table for every existing entry.
    /// Delete-then-insert per entry so a re-run (after a derivation-logic
    /// change) replaces stale rows rather than duplicating them. Reuses three
    /// prepared statements across rows. Non-destructive to `custom_dictionary`.
    private static func backfillSearchKeys(db: OpaquePointer) {
        var selectStmt: OpaquePointer?
        guard sqlite3_prepare_v2(
            db,
            "SELECT id, roman FROM \(CustomDictionarySchema.tableName);",
            -1, &selectStmt, nil,
        ) == SQLITE_OK else { return }
        defer { sqlite3_finalize(selectStmt) }

        var deleteStmt: OpaquePointer?
        guard sqlite3_prepare_v2(
            db,
            "DELETE FROM \(CustomDictionarySchema.searchKeyTableName) WHERE \(CustomDictionarySchema.searchKeyEntryIdColumn) = ?;",
            -1, &deleteStmt, nil,
        ) == SQLITE_OK else { return }
        defer { sqlite3_finalize(deleteStmt) }

        var insertStmt: OpaquePointer?
        let insertSQL = """
            INSERT INTO \(CustomDictionarySchema.searchKeyTableName)
                (\(CustomDictionarySchema.searchKeyEntryIdColumn), \(CustomDictionarySchema.searchKeyFamilyColumn), \(CustomDictionarySchema.searchKeyFormColumn), \(CustomDictionarySchema.searchKeyKeyColumn))
            VALUES (?, ?, ?, ?);
        """
        guard sqlite3_prepare_v2(db, insertSQL, -1, &insertStmt, nil) == SQLITE_OK else { return }
        defer { sqlite3_finalize(insertStmt) }

        while sqlite3_step(selectStmt) == SQLITE_ROW {
            let id = String(cString: sqlite3_column_text(selectStmt, 0))
            let roman = String(cString: sqlite3_column_text(selectStmt, 1))

            sqlite3_reset(deleteStmt)
            sqlite3_clear_bindings(deleteStmt)
            deleteStmt.bindText(1, id)
            sqlite3_step(deleteStmt)

            for searchKey in CustomDictionaryDerivation.searchKeys(for: roman) {
                sqlite3_reset(insertStmt)
                sqlite3_clear_bindings(insertStmt)
                insertStmt.bindText(1, id)
                insertStmt.bindText(2, searchKey.family)
                insertStmt.bindText(3, searchKey.form)
                insertStmt.bindText(4, searchKey.key)
                sqlite3_step(insertStmt)
            }
        }
    }
}
