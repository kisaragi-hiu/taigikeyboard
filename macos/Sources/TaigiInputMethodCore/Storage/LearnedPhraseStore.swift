// The phrases the user composed segment by segment (§50) — learning data in its own file.

import Foundation

/// One learned phrase with its count, for the tests: no product surface
/// lists learned phrases.
struct LearnedPhraseRow: Equatable, Sendable {
    let hanzi: String
    let canonicalTl: String
    let learnCount: Int
}

/// Writes `Effect.PhraseLearned`, answers the exact whole-buffer query on the
/// keystroke path, and is wiped with the other learning data.
///
/// Its own `learned_phrases.db`, not a table in `custom_dictionary.db`
/// (USER 2026-09-21): learned phrases are learning data like
/// `user_frequency.db`, never listed with the user's own words and never
/// carried by a backup. The keys come from the same engine derivation the
/// custom dictionary uses, so the exact whole-buffer query in any input mode
/// finds a phrase.
///
/// CROSS-PLATFORM INVARIANT — the schema and SQL mirror
/// ios/Sources/TaigiKeyboard/Lexicon/Database/LearnedPhraseRepository.swift and
/// the Android `LearnedPhraseService`. Drift causes silent divergence.
final class LearnedPhraseStore: @unchecked Sendable {
    private static let tableName = "learned_phrases"
    private static let searchKeyTableName = "learned_search_key"
    /// `user_version` — a key-derivation change bumps it and adds a backfill step.
    private static let schemaVersion: Int32 = 1

    /// Rows kept; past it the fewest-composed, then least recently touched,
    /// row goes (ChiaKey's policy) so a learn never fails.
    static let maxEntries = 2000
    /// Largest `learn_count` a row can carry.
    static let maxLearnCount = 1_000_000

    private let database: UserDataDatabase
    private let deriveSearchKeys: @Sendable (String) -> [CustomSearchKey]?
    private let limit: Int

    /// - Parameter deriveSearchKeys: how a canonical TL becomes the keys the
    ///   phrase is findable under. Injected so a test can drive the store
    ///   without the engine, and because the shipped implementation is an
    ///   engine round-trip that must happen OUTSIDE the write transaction.
    /// - Parameter limit: injectable ONLY so a test can reach the cap without
    ///   writing 2000 rows.
    init(
        directory: @escaping @Sendable () throws -> URL,
        deriveSearchKeys: @escaping @Sendable (String) -> [CustomSearchKey]? = {
            RustEngineBridge.deriveCustomSearchKeys(roman: $0)
        },
        limit: Int = LearnedPhraseStore.maxEntries,
    ) {
        database = UserDataDatabase(
            fileName: "learned_phrases.db",
            name: "LearnedPhraseStore",
            directory: directory,
            schema: Self.applySchema,
        )
        self.deriveSearchKeys = deriveSearchKeys
        self.limit = limit
    }

    var isReady: Bool {
        database.isReady
    }

    func open() {
        database.openInBackground()
    }

    // MARK: - Learning

    /// Records one `Effect.PhraseLearned`: inserts the `(hanzi, canonical TL)`
    /// pair or bumps its `learn_count`, in one statement on the `(hanzi,
    /// roman)` unique constraint. A fresh row gets its keys and may evict past
    /// the cap; all in one transaction. Best-effort and off the keystroke
    /// path, like the frequency store's `record`. The keys are derived before
    /// the write lock is taken (an FFI round-trip has no business holding it).
    func learnPhrase(hanzi: String, canonicalTl: String) {
        guard !hanzi.isEmpty, !canonicalTl.isEmpty,
              let searchKeys = deriveSearchKeys(canonicalTl), !searchKeys.isEmpty
        else { return }
        let limit = limit
        database.write { connection in
            try connection.withImmediateTransaction {
                let written = try connection.query(
                    """
                    INSERT INTO \(Self.tableName) (roman, hanzi, learn_count, updated_at)
                    VALUES (?, ?, 1, CURRENT_TIMESTAMP)
                    ON CONFLICT(hanzi, roman) DO UPDATE SET
                        learn_count = MIN(learn_count + 1, \(Self.maxLearnCount)),
                        updated_at = CURRENT_TIMESTAMP
                    RETURNING id, learn_count;
                    """,
                    [.text(canonicalTl), .text(hanzi)],
                ) { (id: $0.integer(0), learnCount: $0.integer(1)) }.first
                // `learn_count` reads 1 only for the row this statement just
                // inserted (a bump lands at 2 or more) — the one case that
                // needs keys and can push the table past its cap.
                guard let written, written.learnCount == 1 else { return }
                try Self.writeSearchKeys(connection, phraseID: written.id, searchKeys: searchKeys)
                try Self.evictPastCap(connection, cap: limit, keeping: written.id)
            }
        }
    }

    /// Bumps a phrase the user just committed as one candidate, so a phrase
    /// that is used stays ahead of the eviction line. No-op for an unknown pair.
    func touchPhrase(hanzi: String, canonicalTl: String) {
        guard !hanzi.isEmpty, !canonicalTl.isEmpty else { return }
        database.write { connection in
            try connection.run(
                """
                UPDATE \(Self.tableName)
                SET learn_count = MIN(learn_count + 1, \(Self.maxLearnCount)), updated_at = CURRENT_TIMESTAMP
                WHERE hanzi = ? AND roman = ?;
                """,
                [.text(hanzi), .text(canonicalTl)],
            )
        }
    }

    // MARK: - Reads

    /// The phrases whose derived key EQUALS `queryKey` — the whole typed
    /// buffer, not a prefix — for `FetchAtPos.learned_entries`, most composed
    /// first. Empty when the store cannot answer right now.
    func rows(matching queryKey: CustomSearchKey, limit: Int = 5) -> [LearnedPhraseRow] {
        database.read { connection in
            try connection.query(
                """
                SELECT \(Self.rowColumns)
                FROM \(Self.tableName)
                JOIN \(Self.searchKeyTableName) ON phrase_id = id
                WHERE family = ? AND form = ? AND key = ?
                ORDER BY \(Self.listOrder)
                LIMIT ?;
                """,
                [.text(queryKey.family), .text(queryKey.form), .text(queryKey.key), .integer(limit)],
                decoding: Self.decodeRow,
            )
        } ?? []
    }

    /// Every phrase, most composed first, or `nil` when the store could not
    /// be read. For the tests — nothing in the shipped UI lists learned phrases.
    func allRows() -> [LearnedPhraseRow]? {
        database.read { connection in
            try connection.query(
                """
                SELECT \(Self.rowColumns) FROM \(Self.tableName)
                ORDER BY \(Self.listOrder), id;
                """,
                decoding: Self.decodeRow,
            )
        }
    }

    // MARK: - User-driven administration

    /// Forgets everything, and reports how many rows went — the learning-data
    /// wipe, beside the frequency and association stores'. One transaction:
    /// the keys and the rows land together, and the count taken inside it is
    /// the number that went.
    @discardableResult
    func deleteAll() async throws -> Int {
        try await database.perform { connection in
            let removed = try connection.withImmediateTransaction { () -> Int in
                let existing = try connection.scalar("SELECT COUNT(*) FROM \(Self.tableName);") ?? 0
                try connection.execute("DELETE FROM \(Self.searchKeyTableName);")
                try connection.execute("DELETE FROM \(Self.tableName);")
                return existing
            }
            try? connection.execute("VACUUM;")
            return removed
        }
    }

    // MARK: - Private

    /// The full cross-mode key bundle for a fresh row. `OR IGNORE` on the
    /// side-key unique index absorbs a repeated key.
    private static func writeSearchKeys(
        _ connection: SQLiteConnection,
        phraseID: Int,
        searchKeys: [CustomSearchKey],
    ) throws {
        for searchKey in searchKeys {
            try connection.run(
                """
                INSERT OR IGNORE INTO \(searchKeyTableName) (phrase_id, family, form, key)
                VALUES (?, ?, ?, ?);
                """,
                [.integer(phraseID), .text(searchKey.family), .text(searchKey.form), .text(searchKey.key)],
            )
        }
    }

    /// Drops rows past `cap`, never `keptID` (the row just inserted survives
    /// whatever its timestamp ties with). A cheap `COUNT(*)` first — under the
    /// cap the ordered walk never runs. Keys first, so the subquery still
    /// resolves against the intact main table; `OFFSET cap - 1` selects
    /// exactly the rows past the cap once the kept row is set aside.
    private static func evictPastCap(
        _ connection: SQLiteConnection,
        cap: Int,
        keeping keptID: Int,
    ) throws {
        guard try connection.scalar("SELECT COUNT(*) FROM \(tableName);") ?? 0 > cap else { return }
        let pastCap = """
            SELECT id FROM \(tableName)
            WHERE id <> ?
            ORDER BY learn_count DESC, updated_at DESC, id
            LIMIT -1 OFFSET ?
        """
        let bindings: [SQLiteBinding] = [.integer(keptID), .integer(max(cap - 1, 0))]
        try connection.run("DELETE FROM \(searchKeyTableName) WHERE phrase_id IN (\(pastCap));", bindings)
        try connection.run("DELETE FROM \(tableName) WHERE id IN (\(pastCap));", bindings)
    }

    /// The columns every `LearnedPhraseRow` read selects, next to the decoder
    /// that reads them: the two agree by position. Unqualified — the side
    /// table has none of these names — so the joined query can use them too.
    private static let rowColumns = "hanzi, roman, learn_count"

    /// One order, most composed first.
    private static let listOrder = "learn_count DESC, updated_at DESC"

    private static func decodeRow(_ row: SQLiteRowReader) -> LearnedPhraseRow {
        LearnedPhraseRow(hanzi: row.text(0), canonicalTl: row.text(1), learnCount: row.integer(2))
    }

    // MARK: - Schema

    /// The current shape, created directly.
    private static let applySchema: @Sendable (SQLiteConnection) throws -> Void = { connection in
        try connection.execute(
            """
            CREATE TABLE IF NOT EXISTS \(tableName) (
                id INTEGER PRIMARY KEY,
                roman TEXT NOT NULL,
                hanzi TEXT NOT NULL,
                learn_count INTEGER NOT NULL DEFAULT 1,
                updated_at TEXT NOT NULL,
                UNIQUE(hanzi, roman)
            );
            CREATE TABLE IF NOT EXISTS \(searchKeyTableName) (
                phrase_id INTEGER NOT NULL,
                family TEXT NOT NULL,
                form TEXT NOT NULL,
                key TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lsk_lookup ON \(searchKeyTableName)(family, form, key);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_lsk_phrase ON \(searchKeyTableName)(phrase_id, family, form, key);
            CREATE INDEX IF NOT EXISTS idx_learned_rank ON \(tableName)(learn_count, updated_at);
            """,
        )
        connection.userVersion = schemaVersion
    }
}
