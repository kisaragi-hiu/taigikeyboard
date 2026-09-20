import Foundation
import SQLite3

/// Custom-dictionary DB schema (DDL only).
///
/// Owns the table + indexes for `custom_dictionary.db`. Data migrations
/// (adding missing columns on pre-existing databases, backfilling derived
/// search keys) live in `CustomDictionaryMigrator` so this type stays purely
/// DDL — no domain logic, no derivation, no capacity policy.
/// Callers must serialize access (typically via `SQLiteConnectionManager.execute`).
enum CustomDictionarySchema {
    static let tableName = "custom_dictionary"

    /// Cross-mode search side table (v3.6.1 R3). One row per
    /// (entry, family, form) search key produced by
    /// `RustEngineBridge.deriveCustomSearchKeys`. The NEW query path joins
    /// here by the current input's family; the legacy `notone` / `abbrev` /
    /// `roman_num` columns on `custom_dictionary` stay write-only for
    /// backcompat / rollback.
    static let searchKeyTableName = "custom_search_key"
    static let searchKeyEntryIdColumn = "entry_id"
    static let searchKeyFamilyColumn = "family"
    static let searchKeyFormColumn = "form"
    static let searchKeyKeyColumn = "key"

    /// Bump when `CustomDictionaryDerivation` logic changes or new derived
    /// columns are added — `CustomDictionaryMigrator` re-runs ALTER + backfill
    /// against any DB whose `PRAGMA user_version` is below this value.
    /// v4 (2026-09-18): the search-key abbreviation face became the leading
    /// spelling unit per syllable (`ph` / `th` / `kh` / `tsh` whole,
    /// `behavioral-invariants.md` §46); the side table is re-derived, the
    /// legacy `abbrev` column keeps its first-letter face.
    /// v5 (2026-09-20): learned phrases (§50) — provenance columns `origin`
    /// / `learn_count` + the partial unique index that makes learning one
    /// atomic upsert. Derivation unchanged, so v4 → v5 adds columns only.
    static let schemaVersion = 5

    /// Derived column names backed by `CustomDictionaryDerivation`.
    /// Single source of truth for the `ALTER TABLE` migrator.
    static let derivedColumns = ["notone", "abbrev", "roman_num"]

    /// Provenance columns (§50): `origin` = `CustomDictionaryEntry.Origin`
    /// raw value, `learn_count` = times a learned row was composed / picked.
    /// INTEGER NOT NULL DEFAULT 0 so every pre-v5 row reads as manual.
    static let provenanceColumns = ["origin", "learn_count"]

    /// One learned row per `(hanzi, roman)` pair; manual rows keep today's
    /// duplicate tolerance. Created by both `ensureTables` (fresh DB) and
    /// the v5 migrator (after the columns exist on an older DB).
    static let learnedPairIndexSQL =
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_custom_learned_pair ON \(tableName)(hanzi, roman) WHERE origin = 1;"

    /// Create the primary table + side table + all indexes. Idempotent via
    /// `IF NOT EXISTS`.
    static func ensureTables(db: OpaquePointer) throws {
        try createMainTable(db: db)
        createIndexes(db: db)
        try createSearchKeyTable(db: db)
        createSearchKeyIndexes(db: db)
    }

    /// Check whether a column exists on `custom_dictionary`.
    /// Public so `CustomDictionaryMigrator` can gate `ALTER TABLE` calls.
    static func columnExists(db: OpaquePointer, column: String) -> Bool {
        sqliteColumnExists(db: db, table: tableName, column: column)
    }

    // MARK: - Private

    private static func createMainTable(db: OpaquePointer) throws {
        let sql = """
            CREATE TABLE IF NOT EXISTS \(tableName) (
                id TEXT PRIMARY KEY,
                roman TEXT NOT NULL,
                hanzi TEXT NOT NULL,
                notone TEXT DEFAULT '',
                abbrev TEXT DEFAULT '',
                roman_num TEXT DEFAULT '',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                origin INTEGER NOT NULL DEFAULT 0,
                learn_count INTEGER NOT NULL DEFAULT 0
            );
        """
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
            throw LexiconError.queryPreparationFailed("Create \(tableName) failed: \(String(cString: sqlite3_errmsg(db)))")
        }
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_step(stmt) == SQLITE_DONE else {
            throw LexiconError.queryExecutionFailed("Create \(tableName) failed: \(String(cString: sqlite3_errmsg(db)))")
        }
    }

    private static func createIndexes(db: OpaquePointer) {
        for sql in [
            "CREATE INDEX IF NOT EXISTS idx_custom_roman ON \(tableName)(roman);",
            "CREATE INDEX IF NOT EXISTS idx_custom_notone ON \(tableName)(notone);",
            "CREATE INDEX IF NOT EXISTS idx_custom_abbrev ON \(tableName)(abbrev);",
            "CREATE INDEX IF NOT EXISTS idx_custom_roman_num ON \(tableName)(roman_num);",
            learnedPairIndexSQL,
        ] {
            sqliteExecSimple(db: db, sql)
        }
    }

    // CROSS-PLATFORM INVARIANT — mirrors android/app/src/main/java/com/siansiansu/taigikeyboard/ime/dictionary/CustomDictionaryService.kt (custom_search_key). Drift causes silent divergence.
    private static func createSearchKeyTable(db: OpaquePointer) throws {
        let sql = """
            CREATE TABLE IF NOT EXISTS \(searchKeyTableName) (
                \(searchKeyEntryIdColumn) TEXT NOT NULL,
                \(searchKeyFamilyColumn) TEXT NOT NULL,
                \(searchKeyFormColumn) TEXT NOT NULL,
                \(searchKeyKeyColumn) TEXT NOT NULL
            );
        """
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
            throw LexiconError.queryPreparationFailed("Create \(searchKeyTableName) failed: \(String(cString: sqlite3_errmsg(db)))")
        }
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_step(stmt) == SQLITE_DONE else {
            throw LexiconError.queryExecutionFailed("Create \(searchKeyTableName) failed: \(String(cString: sqlite3_errmsg(db)))")
        }
    }

    private static func createSearchKeyIndexes(db: OpaquePointer) {
        for sql in [
            "CREATE INDEX IF NOT EXISTS idx_csk_lookup ON \(searchKeyTableName)(\(searchKeyFamilyColumn), \(searchKeyFormColumn), \(searchKeyKeyColumn));",
            "CREATE INDEX IF NOT EXISTS idx_csk_entry ON \(searchKeyTableName)(\(searchKeyEntryIdColumn));",
        ] {
            sqliteExecSimple(db: db, sql)
        }
    }
}
