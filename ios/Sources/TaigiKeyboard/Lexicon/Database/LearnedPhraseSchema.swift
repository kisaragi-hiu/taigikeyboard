import Foundation
import SQLite3

/// Learned-phrase DB schema (DDL only) — `learned_phrases.db` (§50).
///
/// A phrase the user composed segment by segment (`Effect.PhraseLearned`)
/// is one `(hanzi, roman)` row with a `learn_count`, plus its cross-mode
/// search keys in a side table shaped like the custom dictionary's
/// `custom_search_key`, so the exact whole-buffer query in any input mode
/// finds it. Its own file, not a table in `custom_dictionary.db`: learned
/// phrases are learning data (like `user_frequency.db`), never listed with
/// the user's own words and never carried by a backup (USER 2026-09-21).
/// Callers must serialize access (typically via `SQLiteConnectionManager.execute`).
enum LearnedPhraseSchema {
    static let tableName = "learned_phrases"
    static let searchKeyTableName = "learned_search_key"

    /// `PRAGMA user_version` — a key-derivation change bumps it and adds a backfill step.
    static let schemaVersion = 1

    static func ensureTables(db: OpaquePointer) throws {
        try sqliteExecChecked(db: db, """
            CREATE TABLE IF NOT EXISTS \(tableName) (
                id INTEGER PRIMARY KEY,
                roman TEXT NOT NULL,
                hanzi TEXT NOT NULL,
                learn_count INTEGER NOT NULL DEFAULT 1,
                updated_at TEXT NOT NULL,
                UNIQUE(hanzi, roman)
            );
        """)
        try sqliteExecChecked(db: db, """
            CREATE TABLE IF NOT EXISTS \(searchKeyTableName) (
                phrase_id INTEGER NOT NULL,
                family TEXT NOT NULL,
                form TEXT NOT NULL,
                key TEXT NOT NULL
            );
        """)
        for sql in [
            "CREATE INDEX IF NOT EXISTS idx_lsk_lookup ON \(searchKeyTableName)(family, form, key);",
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_lsk_phrase ON \(searchKeyTableName)(phrase_id, family, form, key);",
            "CREATE INDEX IF NOT EXISTS idx_learned_rank ON \(tableName)(learn_count, updated_at);",
        ] {
            try sqliteExecChecked(db: db, sql)
        }
        sqliteSetUserVersion(db: db, version: schemaVersion)
    }
}
