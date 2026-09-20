import Foundation
import SQLite3

/// Repository for user custom dictionary entries.
///
/// Stores data in the App Group shared container so the keyboard extension
/// can read it. Public API covers CRUD, search (async + sync hot path), and
/// batched CSV import. Split responsibilities:
/// - `CustomDictionarySchema`: DDL (CREATE TABLE / CREATE INDEX)
/// - `CustomDictionaryMigrator`: forward data migrations (ALTER + backfill)
/// - `CustomDictionaryCapacityPolicy`: row-count cap + TOCTOU-safe guard
/// - `CustomDictionaryDerivation`: pure derivation of search-key variants
final class CustomDictionaryRepository: @unchecked Sendable {
    // MARK: - Properties

    private let connectionManager: SQLiteConnectionManager
    private let logger = DebugLogger(category: "CustomDictionaryRepository")

    /// Lock protecting mutable state (`_tableCreationTask`, `_tableCreationGeneration`).
    private let stateLock = NSLock()
    /// Async-once gate for schema + migration — concurrent callers await
    /// the same `Task`; nil cache on failure allows retry.
    private var _tableCreationTask: Task<Void, Error>?
    /// Bumped every time `_tableCreationTask` is replaced so error handlers
    /// only clear the cache they created (Task is a struct, no `===`).
    private var _tableCreationGeneration: UInt64 = 0

    /// Learned-row quota (§50); the production default is the
    /// cross-platform constant, tests lower it to exercise eviction cheaply.
    private let maxLearnedEntries: Int

    // MARK: - Initialization

    init(
        connectionManager: SQLiteConnectionManager? = nil,
        maxLearnedEntries: Int = CustomDictionaryCapacityPolicy.maxLearnedEntries,
    ) {
        self.connectionManager = connectionManager ?? SQLiteConnectionManager(
            databasePath: { try SharedDatabasePath.resolve(filename: "custom_dictionary.db") },
            queueLabel: "com.siansiansu.taigikeyboard.customdictionary",
            loggerCategory: "CustomDictionaryRepository",
        )
        self.maxLearnedEntries = maxLearnedEntries
    }

    func ensureInitialized() async throws {
        try await connectionManager.ensureInitialized(
            flags: SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
        )
        try await createTablesIfNeeded()
    }

    // MARK: - CRUD

    /// Insert or update an entry (upsert by id) — the user's own word. §50:
    /// a manual row for a `(roman, hanzi)` pair a learned row already holds
    /// TAKES OVER that row (the learned row and its side keys go, after the
    /// manual write landed), so the pair is listed once and the rule "manual
    /// wins, never the reverse" holds for the list editor, the CSV importer
    /// and the backup importer alike; a learned `entry` written through here
    /// becomes manual too (editing a learned row adopts it — and counts
    /// against the manual quota like any new manual row). One transaction:
    /// the guard, the write, the side keys and the takeover commit together
    /// or not at all.
    func upsert(_ entry: CustomDictionaryEntry) async throws {
        try await ensureInitialized()
        try await connectionManager.execute { db in
            try sqliteTransaction(db: db) {
                let entry = entry.asManual
                // Capacity guard runs in the same transaction as the write
                // to avoid a TOCTOU race with concurrent writers.
                try CustomDictionaryCapacityPolicy.guardInsertCapacity(db: db, id: entry.id)
                try Self.writeManualRow(db: db, entry)
            }
        }
    }

    /// The manual write itself: upsert by id, side keys, then the takeover
    /// of any learned row for the pair. Caller holds the transaction.
    private static func writeManualRow(db: OpaquePointer, _ entry: CustomDictionaryEntry) throws {
        var stmt: OpaquePointer?
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_prepare_v2(db, upsertEntrySQL, -1, &stmt, nil) == SQLITE_OK else {
            throw LexiconError.queryPreparationFailed(
                "Upsert failed: \(String(cString: sqlite3_errmsg(db)))",
            )
        }
        bindEntry(stmt, entry)
        guard sqlite3_step(stmt) == SQLITE_DONE else {
            throw LexiconError.queryExecutionFailed(
                "Upsert failed: \(String(cString: sqlite3_errmsg(db)))",
            )
        }
        try writeSearchKeys(db: db, entryId: entry.id, roman: entry.roman)
        try removeLearnedRow(db: db, roman: entry.roman, hanzi: entry.hanzi, except: entry.id)
    }

    /// Fetch all entries ordered by updated_at descending.
    func fetchAll() async throws -> [CustomDictionaryEntry] {
        try await ensureInitialized()
        return try await connectionManager.execute { db in
            let sql = """
                SELECT \(Self.entryColumns)
                FROM \(CustomDictionarySchema.tableName)
                ORDER BY updated_at DESC;
            """
            var stmt: OpaquePointer?
            guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
                return []
            }
            defer { sqlite3_finalize(stmt) }

            return Self.readEntries(from: stmt)
        }
    }

    /// Cross-mode prefix search (v3.6.1 R3) over MANUAL rows. Query keys come
    /// from `CustomDictionaryDerivation.queryKey(for:mode:)`, which is
    /// family-native to the current input mode, so an entry stored in any
    /// mode is found. Learned rows (§50) are excluded — they ride
    /// `FetchAtPos.learned_entries` through `learnedEntriesSync`, never the
    /// custom-dictionary override.
    /// - Parameters:
    ///   - family: Search-key family (`tl` / `poj` / `tps`).
    ///   - form: Search-key form (`num` / `notone` / `abbrev`).
    ///   - key: Fused search string (already normalized + lowercased by the
    ///     engine — bind verbatim, do NOT re-lowercase).
    func search(family: String, form: String, key: String, limit: Int = 50) async throws -> [CustomDictionaryEntry] {
        try await ensureInitialized()
        return try await connectionManager.execute { db in
            Self.runPrefixSearch(db: db, family: family, form: form, key: key, limit: limit)
        }
    }

    /// `search` for the keyboard extension hot path (manual rows only).
    /// Returns `[]` when the DB is not yet connected — callers must accept
    /// empty results on the very first keystroke rather than blocking.
    func searchSync(family: String, form: String, key: String, limit: Int = 50) -> [CustomDictionaryEntry] {
        guard connectionManager.isConnected() else { return [] }
        do {
            return try connectionManager.executeSync { db in
                Self.runPrefixSearch(db: db, family: family, form: form, key: key, limit: limit)
            }
        } catch {
            return []
        }
    }

    // MARK: - Learned Phrases (§50)

    /// Largest `learn_count` a row can carry — a backup file is untrusted
    /// input and the column is bound as `Int32`.
    static let maxLearnCount = 1_000_000

    /// Record one `Effect.PhraseLearned` (or one backup row, `count` > 1):
    /// insert the `(hanzi, canonical TL)` pair as a learned row or add
    /// `count` to its `learn_count`, in one statement on the learned-pair
    /// unique index. A manual row for the same pair wins — the learn is a
    /// no-op (never downgraded). Manual check, upsert, side keys and
    /// eviction run in ONE transaction so another connection (the host app
    /// editing the list while the extension learns) cannot interleave and
    /// a crash cannot leave a row without its search keys.
    func learnPhrase(hanzi: String, canonicalTl: String, count: Int = 1) async throws {
        guard !hanzi.isEmpty, !canonicalTl.isEmpty else { return }
        let count = min(max(count, 1), Self.maxLearnCount)
        let cap = maxLearnedEntries
        try await ensureInitialized()
        try await connectionManager.execute { db in
            try sqliteTransaction(db: db) {
                guard !Self.manualRowExists(db: db, roman: canonicalTl, hanzi: hanzi) else { return }
                let entry = CustomDictionaryEntry(
                    roman: canonicalTl,
                    hanzi: hanzi,
                    origin: .learned,
                    learnCount: count,
                )
                var stmt: OpaquePointer?
                guard sqlite3_prepare_v2(db, Self.learnPhraseSQL, -1, &stmt, nil) == SQLITE_OK else {
                    throw LexiconError.queryPreparationFailed(
                        "Learn phrase failed: \(String(cString: sqlite3_errmsg(db)))",
                    )
                }
                defer { sqlite3_finalize(stmt) }
                Self.bindEntry(stmt, entry)
                // `RETURNING id` answers with the row that took the write —
                // `entry.id` on a fresh insert, the existing id on a bump —
                // so the side keys are written for the right row either way
                // (a bump rewrites identical keys; delete + insert is idempotent).
                guard sqlite3_step(stmt) == SQLITE_ROW,
                      let idText = sqlite3_column_text(stmt, 0)
                else {
                    throw LexiconError.queryExecutionFailed(
                        "Learn phrase failed: \(String(cString: sqlite3_errmsg(db)))",
                    )
                }
                let rowId = String(cString: idText)
                try Self.writeSearchKeys(db: db, entryId: rowId, roman: entry.roman)
                try CustomDictionaryCapacityPolicy.evictLearnedPastCap(db: db, cap: cap, keeping: rowId)
            }
        }
    }

    /// Bump a learned row the user just committed as a whole candidate, so a
    /// phrase that is used stays ahead of the eviction line. No-op for a
    /// manual row or an unknown pair.
    func touchLearnedPhrase(hanzi: String, canonicalTl: String) async throws {
        guard !hanzi.isEmpty, !canonicalTl.isEmpty else { return }
        try await ensureInitialized()
        try await connectionManager.execute { db in
            let sql = """
                UPDATE \(CustomDictionarySchema.tableName)
                SET learn_count = MIN(learn_count + 1, \(Self.maxLearnCount)), updated_at = ?
                WHERE origin = 1 AND hanzi = ? AND roman = ?;
            """
            var stmt: OpaquePointer?
            guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return }
            defer { sqlite3_finalize(stmt) }
            stmt.bindText(1, Self.dateFormatter.string(from: Date()))
            stmt.bindText(2, hanzi)
            stmt.bindText(3, canonicalTl)
            sqlite3_step(stmt)
        }
    }

    /// Learned rows whose derived key EQUALS the query key — the whole typed
    /// buffer, not a prefix — for `FetchAtPos.learned_entries`. Exact so a
    /// learned whole-buffer match can never be truncated out of the prefix
    /// search's `LIMIT`; learned-only, the mirror of `search` /
    /// `searchSync` being manual-only. Sync for the keyboard hot path; `[]`
    /// before the connection is open, like `searchSync`.
    func learnedEntriesSync(family: String, form: String, key: String, limit: Int = 5) -> [CustomDictionaryEntry] {
        guard connectionManager.isConnected() else { return [] }
        do {
            return try connectionManager.executeSync { db in
                Self.runSearchKeyJoin(
                    db: db,
                    sql: Self.learnedExactSearchSQL,
                    family: family,
                    form: form,
                    key: key,
                    limit: limit,
                )
            }
        } catch {
            return []
        }
    }

    /// Delete an entry by id (and its `custom_search_key` side rows).
    func delete(id: String) async throws {
        try await ensureInitialized()
        try await connectionManager.execute { db in
            let sql = "DELETE FROM \(CustomDictionarySchema.tableName) WHERE id = ?;"
            var stmt: OpaquePointer?
            guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return }
            defer { sqlite3_finalize(stmt) }

            stmt.bindText(1, id)
            sqlite3_step(stmt)

            try Self.deleteSearchKeys(db: db, entryId: id)
        }
    }

    /// Delete all entries (and all `custom_search_key` side rows).
    func deleteAll() async throws {
        try await ensureInitialized()
        try await connectionManager.execute { db in
            sqliteExecSimple(db: db, "DELETE FROM \(CustomDictionarySchema.tableName);")
            sqliteExecSimple(db: db, "DELETE FROM \(CustomDictionarySchema.searchKeyTableName);")
            // R6: reclaim freed pages after a full clear. Best-effort —
            // VACUUM needs exclusive access + ~2x temp; a failure leaves the
            // file larger but intact (sqliteExecSimple is silent). Runs after
            // the DELETEs committed, outside any transaction.
            sqliteExecSimple(db: db, "VACUUM")
        }
    }

    /// MANUAL entry count (the rows under the user's quota); learned rows are
    /// not included.
    func count() async throws -> Int {
        try await ensureInitialized()
        return try await connectionManager.execute { db in
            CustomDictionaryCapacityPolicy.currentEntryCount(db: db)
        }
    }

    // MARK: - Batch Import

    private static let importBatchSize = 500

    /// Import a CSV-sourced batch. Commits every `importBatchSize` entries
    /// so a single transaction can never hold locks for long, and stops
    /// early when `maxEntries` is reached. Duplicates (same `roman|hanzi`
    /// key) are skipped.
    func batchImport(_ entries: [CustomDictionaryEntry]) async throws -> Int {
        try await ensureInitialized()
        return try await connectionManager.execute { db in
            let remainingCapacity = CustomDictionaryCapacityPolicy.remainingCapacity(db: db)
            guard remainingCapacity > 0 else {
                self.logger.debug("[IMPORT] Custom dictionary is full (\(CustomDictionaryCapacityPolicy.maxEntries) entries)")
                return 0
            }

            var existingKeys = Self.existingRomanHanziKeys(db: db)
            var insertedCount = 0

            for batchStart in stride(from: 0, to: entries.count, by: Self.importBatchSize) {
                let batchEnd = min(batchStart + Self.importBatchSize, entries.count)

                guard sqlite3_exec(db, "BEGIN TRANSACTION;", nil, nil, nil) == SQLITE_OK else {
                    throw LexiconError.queryExecutionFailed("Failed to begin transaction")
                }

                for i in batchStart ..< batchEnd {
                    if insertedCount >= remainingCapacity {
                        break
                    }

                    let entry = entries[i]
                    let key = "\(entry.roman)|\(entry.hanzi)"
                    if existingKeys.contains(key) {
                        continue
                    }

                    // A row that fails is skipped, its learned twin untouched:
                    // the takeover runs only after the manual write landed.
                    guard (try? Self.writeManualRow(db: db, entry)) != nil else { continue }
                    existingKeys.insert(key)
                    insertedCount += 1
                }

                guard sqlite3_exec(db, "COMMIT;", nil, nil, nil) == SQLITE_OK else {
                    sqlite3_exec(db, "ROLLBACK;", nil, nil, nil)
                    throw LexiconError.queryExecutionFailed("Failed to commit batch transaction")
                }

                if insertedCount >= remainingCapacity {
                    break
                }
            }

            return insertedCount
        }
    }

    // MARK: - Lifecycle

    func deleteDatabase() throws {
        // Cancel the in-flight init Task (if any) BEFORE closing the
        // connection so it bails out rather than racing against a fresh
        // Task installed by the next caller.
        let priorTask = stateLock.withLock { () -> Task<Void, Error>? in
            let task = _tableCreationTask
            _tableCreationTask = nil
            _tableCreationGeneration &+= 1
            return task
        }
        priorTask?.cancel()
        connectionManager.close()

        let path = try SharedDatabasePath.resolve(filename: "custom_dictionary.db")
        if FileManager.default.fileExists(atPath: path) {
            try FileManager.default.removeItem(atPath: path)
        }
    }

    func isConnected() -> Bool {
        connectionManager.isConnected()
    }

    // MARK: - Schema (async-once gate)

    /// Single-flight schema + migration initialization. Concurrent callers
    /// await the same `Task`; once it succeeds subsequent calls await a
    /// completed task (near-free). Failures clear the cache for retry.
    private func createTablesIfNeeded() async throws {
        let (task, generation) = stateLock.withLock { () -> (Task<Void, Error>, UInt64) in
            if let existing = _tableCreationTask {
                return (existing, _tableCreationGeneration)
            }
            _tableCreationGeneration &+= 1
            let gen = _tableCreationGeneration
            let connection = self.connectionManager
            let logger = self.logger
            let new = Task {
                try await connection.execute { db in
                    try CustomDictionarySchema.ensureTables(db: db)
                    try CustomDictionaryMigrator.runIfNeeded(db: db, logger: logger)
                }
            }
            _tableCreationTask = new
            return (new, gen)
        }
        do {
            try await task.value
        } catch {
            stateLock.withLock {
                if _tableCreationGeneration == generation {
                    _tableCreationTask = nil
                }
            }
            throw error
        }
    }

    // MARK: - Query Helpers

    /// The columns `readEntry` expects, in order.
    private static let entryColumns = "id, roman, hanzi, created_at, updated_at, origin, learn_count"

    private static let insertColumnsSQL =
        "(id, roman, hanzi, notone, abbrev, roman_num, created_at, updated_at, origin, learn_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"

    private static let upsertEntrySQL = """
        INSERT INTO \(CustomDictionarySchema.tableName) \(insertColumnsSQL)
        ON CONFLICT(id) DO UPDATE SET
            roman = excluded.roman,
            hanzi = excluded.hanzi,
            notone = excluded.notone,
            abbrev = excluded.abbrev,
            roman_num = excluded.roman_num,
            updated_at = excluded.updated_at,
            origin = excluded.origin,
            learn_count = excluded.learn_count;
    """

    /// §50 — the learned-pair index (`WHERE origin = 1`) is the conflict
    /// target, so learning the same pair again is one row with `learn_count
    /// + excluded.learn_count`, never a duplicate. Bound through `bindEntry`
    /// with a learned entry (`learn_count` = 1 from the engine, the saved
    /// count from a backup).
    private static let learnPhraseSQL = """
        INSERT INTO \(CustomDictionarySchema.tableName) \(insertColumnsSQL)
        ON CONFLICT(hanzi, roman) WHERE origin = 1 DO UPDATE SET
            learn_count = MIN(learn_count + excluded.learn_count, \(maxLearnCount)),
            updated_at = excluded.updated_at
        RETURNING id;
    """

    private static func bindEntry(_ stmt: OpaquePointer?, _ entry: CustomDictionaryEntry) {
        stmt.bindText(1, entry.id)
        stmt.bindText(2, entry.roman)
        stmt.bindText(3, entry.hanzi)
        stmt.bindText(4, CustomDictionaryDerivation.generateNotone(entry.roman))
        stmt.bindText(5, CustomDictionaryDerivation.generateAbbrev(entry.roman))
        stmt.bindText(6, CustomDictionaryDerivation.generateRomanNum(entry.roman))
        stmt.bindText(7, dateFormatter.string(from: entry.createdAt))
        stmt.bindText(8, dateFormatter.string(from: entry.updatedAt))
        sqlite3_bind_int(stmt, 9, Int32(entry.origin.rawValue))
        sqlite3_bind_int(stmt, 10, Int32(entry.learnCount))
    }

    private static func manualRowExists(db: OpaquePointer, roman: String, hanzi: String) -> Bool {
        var stmt: OpaquePointer?
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_prepare_v2(
            db,
            "SELECT 1 FROM \(CustomDictionarySchema.tableName) WHERE origin = 0 AND roman = ? AND hanzi = ? LIMIT 1;",
            -1, &stmt, nil,
        ) == SQLITE_OK else {
            return false
        }
        stmt.bindText(1, roman)
        stmt.bindText(2, hanzi)
        return sqlite3_step(stmt) == SQLITE_ROW
    }

    /// Delete the learned row (and its side keys) for `(roman, hanzi)` unless
    /// it is `except` itself — the manual write that just landed takes it
    /// over. Caller holds the transaction.
    private static func removeLearnedRow(db: OpaquePointer, roman: String, hanzi: String, except id: String) throws {
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(
            db,
            "SELECT id FROM \(CustomDictionarySchema.tableName) WHERE origin = 1 AND roman = ? AND hanzi = ? AND id <> ?;",
            -1, &stmt, nil,
        ) == SQLITE_OK else {
            throw LexiconError.queryPreparationFailed("Learned takeover: \(String(cString: sqlite3_errmsg(db)))")
        }
        stmt.bindText(1, roman)
        stmt.bindText(2, hanzi)
        stmt.bindText(3, id)
        var learnedIds: [String] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            learnedIds.append(String(cString: sqlite3_column_text(stmt, 0)))
        }
        sqlite3_finalize(stmt)
        for learnedId in learnedIds {
            try sqliteExecBound(db: db, "DELETE FROM \(CustomDictionarySchema.tableName) WHERE id = ?;", learnedId)
            try deleteSearchKeys(db: db, entryId: learnedId)
        }
    }

    /// One bound-text statement, throwing on prepare / step failure.
    private static func sqliteExecBound(db: OpaquePointer, _ sql: String, _ text: String) throws {
        var stmt: OpaquePointer?
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
            throw LexiconError.queryPreparationFailed(String(cString: sqlite3_errmsg(db)))
        }
        stmt.bindText(1, text)
        guard sqlite3_step(stmt) == SQLITE_DONE else {
            throw LexiconError.queryExecutionFailed(String(cString: sqlite3_errmsg(db)))
        }
    }

    /// `entryColumns` qualified for the side-table join.
    private static let joinedEntryColumns = entryColumns
        .split(separator: ",")
        .map { "c.\($0.trimmingCharacters(in: .whitespaces))" }
        .joined(separator: ", ")

    // CROSS-PLATFORM INVARIANT — mirrors android/app/src/main/java/com/siansiansu/taigikeyboard/ime/dictionary/CustomDictionaryService.kt (custom_search_key query). Drift causes silent divergence.
    //
    // `origin = 0`: manual rows only (§50) — see `search`. `form IN (?,
    // 'abbrev')` lets an abbrev side row satisfy any query in the same
    // family — preserving the old `notone LIKE ? OR abbrev LIKE ?` behavior.
    // `DISTINCT` because one entry can match several side rows. `key` is
    // bound verbatim (engine already normalized + lowercased it).
    private static let prefixSearchSQL = """
        SELECT DISTINCT \(joinedEntryColumns)
        FROM \(CustomDictionarySchema.tableName) c
        JOIN \(CustomDictionarySchema.searchKeyTableName) k
            ON k.\(CustomDictionarySchema.searchKeyEntryIdColumn) = c.id
        WHERE c.origin = 0
          AND k.\(CustomDictionarySchema.searchKeyFamilyColumn) = ?
          AND k.\(CustomDictionarySchema.searchKeyFormColumn) IN (?, 'abbrev')
          AND k.\(CustomDictionarySchema.searchKeyKeyColumn) LIKE ? || '%'
        ORDER BY c.roman
        LIMIT ?;
    """

    /// §50 — the learned-only, exact (`=`, not `LIKE`) counterpart of
    /// `prefixSearchSQL` (see `learnedEntriesSync`).
    private static let learnedExactSearchSQL = """
        SELECT \(joinedEntryColumns)
        FROM \(CustomDictionarySchema.tableName) c
        JOIN \(CustomDictionarySchema.searchKeyTableName) k
            ON k.\(CustomDictionarySchema.searchKeyEntryIdColumn) = c.id
        WHERE c.origin = 1
          AND k.\(CustomDictionarySchema.searchKeyFamilyColumn) = ?
          AND k.\(CustomDictionarySchema.searchKeyFormColumn) = ?
          AND k.\(CustomDictionarySchema.searchKeyKeyColumn) = ?
        ORDER BY c.learn_count DESC, c.updated_at DESC
        LIMIT ?;
    """

    private static func runPrefixSearch(
        db: OpaquePointer,
        family: String,
        form: String,
        key: String,
        limit: Int,
    ) -> [CustomDictionaryEntry] {
        runSearchKeyJoin(db: db, sql: prefixSearchSQL, family: family, form: form, key: key, limit: limit)
    }

    /// One side-table join with the shared `(family, form, key, limit)` binds.
    private static func runSearchKeyJoin(
        db: OpaquePointer,
        sql: String,
        family: String,
        form: String,
        key: String,
        limit: Int,
    ) -> [CustomDictionaryEntry] {
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return [] }
        defer { sqlite3_finalize(stmt) }
        stmt.bindText(1, family)
        stmt.bindText(2, form)
        stmt.bindText(3, key)
        sqlite3_bind_int(stmt, 4, Int32(limit))
        return readEntries(from: stmt)
    }

    // MARK: - Search-Key Side Table

    private static let insertSearchKeySQL = """
        INSERT INTO \(CustomDictionarySchema.searchKeyTableName)
            (\(CustomDictionarySchema.searchKeyEntryIdColumn), \(CustomDictionarySchema.searchKeyFamilyColumn), \(CustomDictionarySchema.searchKeyFormColumn), \(CustomDictionarySchema.searchKeyKeyColumn))
        VALUES (?, ?, ?, ?);
    """

    /// Replace an entry's `custom_search_key` rows: delete by entry id, then
    /// insert the full cross-mode bundle from
    /// `CustomDictionaryDerivation.searchKeys(for:)`. Called after every upsert
    /// and batch insert so the side table never drifts from the entry's roman.
    /// Throws so the caller's transaction rolls back instead of committing a
    /// row without its keys.
    private static func writeSearchKeys(db: OpaquePointer, entryId: String, roman: String) throws {
        try deleteSearchKeys(db: db, entryId: entryId)

        var stmt: OpaquePointer?
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_prepare_v2(db, insertSearchKeySQL, -1, &stmt, nil) == SQLITE_OK else {
            throw LexiconError.queryPreparationFailed("Search keys: \(String(cString: sqlite3_errmsg(db)))")
        }

        for searchKey in CustomDictionaryDerivation.searchKeys(for: roman) {
            sqlite3_reset(stmt)
            sqlite3_clear_bindings(stmt)
            stmt.bindText(1, entryId)
            stmt.bindText(2, searchKey.family)
            stmt.bindText(3, searchKey.form)
            stmt.bindText(4, searchKey.key)
            guard sqlite3_step(stmt) == SQLITE_DONE else {
                throw LexiconError.queryExecutionFailed("Search keys: \(String(cString: sqlite3_errmsg(db)))")
            }
        }
    }

    private static func deleteSearchKeys(db: OpaquePointer, entryId: String) throws {
        try sqliteExecBound(
            db: db,
            "DELETE FROM \(CustomDictionarySchema.searchKeyTableName) WHERE \(CustomDictionarySchema.searchKeyEntryIdColumn) = ?;",
            entryId,
        )
    }

    private static func existingRomanHanziKeys(db: OpaquePointer) -> Set<String> {
        var stmt: OpaquePointer?
        defer { sqlite3_finalize(stmt) }
        guard sqlite3_prepare_v2(
            db,
            "SELECT roman, hanzi FROM \(CustomDictionarySchema.tableName) WHERE origin = 0;",
            -1, &stmt, nil,
        ) == SQLITE_OK else {
            return []
        }
        var keys = Set<String>()
        while sqlite3_step(stmt) == SQLITE_ROW {
            let roman = String(cString: sqlite3_column_text(stmt, 0))
            let hanzi = String(cString: sqlite3_column_text(stmt, 1))
            keys.insert("\(roman)|\(hanzi)")
        }
        return keys
    }

    // MARK: - Row Reader

    private static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm:ss"
        formatter.timeZone = TimeZone(identifier: "UTC")
        return formatter
    }()

    /// Every remaining row of `stmt` through `readEntry`.
    private static func readEntries(from stmt: OpaquePointer?) -> [CustomDictionaryEntry] {
        var results: [CustomDictionaryEntry] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            if let entry = readEntry(from: stmt) {
                results.append(entry)
            }
        }
        return results
    }

    private static func readEntry(from stmt: OpaquePointer?) -> CustomDictionaryEntry? {
        guard let stmt else { return nil }

        let id = sqlite3_column_text(stmt, 0).map(String.init(cString:)) ?? ""
        let roman = sqlite3_column_text(stmt, 1).map(String.init(cString:)) ?? ""
        let hanzi = sqlite3_column_text(stmt, 2).map(String.init(cString:)) ?? ""
        let createdStr = sqlite3_column_text(stmt, 3).map(String.init(cString:)) ?? ""
        let updatedStr = sqlite3_column_text(stmt, 4).map(String.init(cString:)) ?? ""

        let createdAt = dateFormatter.date(from: createdStr) ?? Date()
        let updatedAt = dateFormatter.date(from: updatedStr) ?? Date()
        let origin = CustomDictionaryEntry.Origin(rawValue: Int(sqlite3_column_int(stmt, 5))) ?? .manual
        let learnCount = Int(sqlite3_column_int(stmt, 6))

        return CustomDictionaryEntry(
            id: id,
            roman: roman,
            hanzi: hanzi,
            createdAt: createdAt,
            updatedAt: updatedAt,
            origin: origin,
            learnCount: learnCount,
        )
    }
}
