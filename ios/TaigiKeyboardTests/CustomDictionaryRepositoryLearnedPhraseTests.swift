import SQLite3
@testable import TaigiKeyboard
import XCTest

/// Learned phrases (§50) on `custom_dictionary.db`: a learned `(hanzi, roman)`
/// pair is one row with a `learn_count`, never a duplicate, never a
/// downgrade of a manual row, found only by an EXACT whole-buffer key, and
/// evicted fewest-then-oldest past `maxLearnedEntries`. A v4 database gains
/// the provenance columns on open.
///
/// Exercises the real repository against a temp-file `SQLiteConnectionManager`
/// (the Android counterpart cannot run SQLite in a JVM test; it pins the
/// shared SQL + S62 dogfood).
final class CustomDictionaryRepositoryLearnedPhraseTests: XCTestCase {
    private var dbPath: String!
    private var repository: CustomDictionaryRepository!

    override func setUpWithError() throws {
        try super.setUpWithError()
        dbPath = NSTemporaryDirectory()
            .appending("custom_dict_learned_\(UUID().uuidString).db")
        let path = dbPath!
        let manager = SQLiteConnectionManager(
            databasePath: { path },
            queueLabel: "test.customdict.learned.\(UUID().uuidString)",
            loggerCategory: "CustomDictionaryRepositoryLearnedPhraseTests",
        )
        repository = CustomDictionaryRepository(connectionManager: manager)
    }

    override func tearDownWithError() throws {
        try? repository.deleteDatabase()
        repository = nil
        for suffix in ["", "-wal", "-shm"] where dbPath != nil {
            let path = dbPath! + suffix
            if FileManager.default.fileExists(atPath: path) {
                try? FileManager.default.removeItem(atPath: path)
            }
        }
        dbPath = nil
        try super.tearDownWithError()
    }

    private func learnedRows() async throws -> [CustomDictionaryEntry] {
        try await repository.fetchAll().filter(\.isLearned)
    }

    private func learnedExact(_ input: String) throws -> [CustomDictionaryEntry] {
        let q = try XCTUnwrap(CustomDictionaryDerivation.queryKey(for: input, mode: .tl))
        return repository.learnedEntriesSync(family: q.family, form: q.form, key: q.key)
    }

    // MARK: - Learning

    func test_learnPhrase_twiceIsOneRowWithCountTwo() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")

        let rows = try await learnedRows()
        XCTAssertEqual(rows.count, 1, "the learned-pair index folds the second learn into the first row; got \(rows)")
        XCTAssertEqual(rows.first?.learnCount, 2)
        XCTAssertEqual(rows.first?.roman, "kì--khí-lâi")
        XCTAssertEqual(rows.first?.hanzi, "記起來")
        // Manual quota untouched: learned rows do not count against it.
        let manualCount = try await repository.count()
        XCTAssertEqual(manualCount, 0, "learned rows are outside the manual capacity count")
    }

    func test_learnPhrase_manualRowForTheSamePairWins() async throws {
        let manual = CustomDictionaryEntry(roman: "kì--khí-lâi", hanzi: "記起來")
        try await repository.upsert(manual)
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")

        let all = try await repository.fetchAll()
        XCTAssertEqual(all.count, 1, "learning a pair the user already added is a no-op; got \(all)")
        XCTAssertEqual(all.first?.origin, .manual)
        XCTAssertEqual(all.first?.learnCount, 0)
    }

    func test_learnPhrase_emptyPartsAreIgnored() async throws {
        try await repository.learnPhrase(hanzi: "", canonicalTl: "kì--khí-lâi")
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "")
        let rows = try await repository.fetchAll()
        XCTAssertTrue(rows.isEmpty, "got \(rows)")
    }

    // MARK: - Recall

    func test_learnedEntriesSync_matchesTheWholeBufferOnly() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        // The sync path answers only once the connection is open.
        try await repository.ensureInitialized()

        XCTAssertEqual(try learnedExact("kikhilai").map(\.hanzi), ["記起來"], "exact toneless key")
        XCTAssertEqual(try learnedExact("ki3khi2lai5").map(\.hanzi), ["記起來"], "exact toned key")
        XCTAssertTrue(try learnedExact("kikhi").isEmpty, "a strict prefix of the phrase must not match")
        XCTAssertTrue(try learnedExact("kikhilaia").isEmpty, "a longer buffer must not match")
    }

    func test_learnedEntriesSync_skipsManualRows() async throws {
        try await repository.upsert(CustomDictionaryEntry(roman: "kì--khí-lâi", hanzi: "記起來"))
        try await repository.ensureInitialized()
        XCTAssertTrue(try learnedExact("kikhilai").isEmpty, "manual rows ride `custom_entries`, not `learned_entries`")
    }

    // MARK: - Touch + eviction

    func test_touchLearnedPhrase_bumpsOnlyTheLearnedRow() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.upsert(CustomDictionaryEntry(roman: "tâi-gí", hanzi: "台語"))

        try await repository.touchLearnedPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.touchLearnedPhrase(hanzi: "台語", canonicalTl: "tâi-gí")

        let all = try await repository.fetchAll()
        XCTAssertEqual(all.first { $0.hanzi == "記起來" }?.learnCount, 2)
        XCTAssertEqual(all.first { $0.hanzi == "台語" }?.learnCount, 0, "a manual row never gains a count")
    }

    func test_evictsFewestComposedThenOldestPastTheLearnedCap() async throws {
        // Fill to the cap, with one row composed twice so it must survive.
        let cap = CustomDictionaryRepository.maxLearnedEntries
        for i in 0 ..< cap {
            try await repository.learnPhrase(hanzi: "詞\(i)", canonicalTl: "su-\(i)")
        }
        try await repository.learnPhrase(hanzi: "詞0", canonicalTl: "su-0")
        // One past the cap evicts exactly one row: a count-1 row, never 詞0.
        try await repository.learnPhrase(hanzi: "新詞", canonicalTl: "sin-su")

        let rows = try await learnedRows()
        XCTAssertEqual(rows.count, cap, "learned rows stay at the cap; got \(rows.count)")
        XCTAssertTrue(rows.contains { $0.hanzi == "詞0" }, "the twice-composed row survives")
        XCTAssertTrue(rows.contains { $0.hanzi == "新詞" }, "the newest learn is kept")
    }

    // MARK: - Migration

    /// A v4 database (no provenance columns) opens as v5: every existing row
    /// reads as manual, and learning works on the same table.
    func test_v4DatabaseGainsProvenanceColumnsAndKeepsRowsManual() async throws {
        try seedV4Row(id: "v4-1", roman: "tâi-gí", hanzi: "台語")

        try await repository.ensureInitialized()
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")

        let all = try await repository.fetchAll()
        XCTAssertEqual(all.first { $0.hanzi == "台語" }?.origin, .manual, "pre-v5 rows are the user's")
        XCTAssertEqual(all.first { $0.hanzi == "記起來" }?.origin, .learned)
        XCTAssertEqual(try learnedExact("kikhilai").map(\.hanzi), ["記起來"])
    }

    private func seedV4Row(id: String, roman: String, hanzi: String) throws {
        var db: OpaquePointer?
        guard sqlite3_open(dbPath, &db) == SQLITE_OK else {
            throw XCTSkip("could not open temp sqlite for v4 seed")
        }
        defer { sqlite3_close(db) }
        let ddl = """
            CREATE TABLE custom_dictionary (
                id TEXT PRIMARY KEY,
                roman TEXT NOT NULL,
                hanzi TEXT NOT NULL,
                notone TEXT DEFAULT '',
                abbrev TEXT DEFAULT '',
                roman_num TEXT DEFAULT '',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE custom_search_key (
                entry_id TEXT NOT NULL,
                family   TEXT NOT NULL,
                form     TEXT NOT NULL,
                key      TEXT NOT NULL
            );
            PRAGMA user_version = 4;
        """
        XCTAssertEqual(sqlite3_exec(db, ddl, nil, nil, nil), SQLITE_OK)
        var stmt: OpaquePointer?
        XCTAssertEqual(
            sqlite3_prepare_v2(db, "INSERT INTO custom_dictionary (id, roman, hanzi) VALUES (?, ?, ?);", -1, &stmt, nil),
            SQLITE_OK,
        )
        defer { sqlite3_finalize(stmt) }
        stmt.bindText(1, id)
        stmt.bindText(2, roman)
        stmt.bindText(3, hanzi)
        XCTAssertEqual(sqlite3_step(stmt), SQLITE_DONE)
    }
}
