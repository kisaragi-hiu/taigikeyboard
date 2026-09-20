import SQLite3
@testable import TaigiKeyboard
import XCTest

/// Learned phrases (§50) on `custom_dictionary.db`: a learned `(hanzi, roman)`
/// pair is one row with a `learn_count`, never a duplicate, never a
/// downgrade of a manual row, found only by an EXACT whole-buffer key, and
/// evicted fewest-then-oldest past the learned cap (the v4 → v5 migration
/// is pinned beside the other migrations in `CustomDictionaryRepositoryCrossModeTests`).
///
/// Exercises the real repository against a temp-file `SQLiteConnectionManager`
/// (the Android counterpart cannot run SQLite in a JVM test; it pins the
/// shared SQL + S62 dogfood).
final class CustomDictionaryRepositoryLearnedPhraseTests: XCTestCase {
    private var dbPath: String!
    private var repository: CustomDictionaryRepository!

    /// Small learned cap so eviction is a handful of inserts, not 2001.
    private static let learnedCap = 3

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
        repository = CustomDictionaryRepository(connectionManager: manager, maxLearnedEntries: Self.learnedCap)
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

    /// The manual write primitive takes over a learned row for the pair —
    /// list editor, CSV import and `.taigi` import all go through it.
    func test_upsert_takesOverALearnedRowForTheSamePair() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.ensureInitialized()
        try await repository.upsert(CustomDictionaryEntry(roman: "kì--khí-lâi", hanzi: "記起來"))

        let all = try await repository.fetchAll()
        XCTAssertEqual(all.count, 1, "got \(all)")
        XCTAssertEqual(all.first?.origin, .manual)
        XCTAssertTrue(try learnedExact("kikhilai").isEmpty, "the learned row's side keys went with it")
        // The CSV path obeys the same rule.
        try await repository.learnPhrase(hanzi: "台語", canonicalTl: "tâi-gí")
        let imported = try await repository.batchImport([CustomDictionaryEntry(roman: "tâi-gí", hanzi: "台語")])
        XCTAssertEqual(imported, 1)
        let taigi = try await repository.fetchAll().filter { $0.hanzi == "台語" }
        XCTAssertEqual(taigi.map(\.origin), [.manual])
    }

    /// Editing a learned row in the list adopts it: the same id, now manual —
    /// and it counts against the manual quota like any new manual row.
    func test_upsert_ofALearnedEntryMakesItManual() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        let learnedFirst = try await learnedRows().first
        let learned = try XCTUnwrap(learnedFirst)
        let manualBefore = try await repository.count()
        try await repository.upsert(learned)
        let all = try await repository.fetchAll()
        XCTAssertEqual(all.map(\.id), [learned.id])
        XCTAssertEqual(all.first?.origin, .manual)
        let manualAfter = try await repository.count()
        XCTAssertEqual(manualAfter, manualBefore + 1, "adopting a learned row is a manual insert for the quota")
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

    /// Codex post-impl 2026-09-20 BLOCK: a learned row must never reach
    /// `custom_entries` (whose walker override is unconditional) — the prefix
    /// search is manual-only, the exact learned query is learned-only.
    func test_prefixSearch_excludesLearnedRows() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.upsert(CustomDictionaryEntry(roman: "kì-khí", hanzi: "記起"))
        let q = try XCTUnwrap(CustomDictionaryDerivation.queryKey(for: "kikhi", mode: .tl))
        let rows = try await repository.search(family: q.family, form: q.form, key: q.key, limit: 20)
        XCTAssertEqual(rows.map(\.hanzi), ["記起"], "only the manual row rides the prefix search; got \(rows)")
    }

    func test_learnPhrase_countFromBackupAddsAndClamps() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi", count: 7)
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        let afterAdd = try await learnedRows().first?.learnCount
        XCTAssertEqual(afterAdd, 8)
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi", count: Int.max)
        let clamped = try await learnedRows().first?.learnCount
        XCTAssertEqual(clamped, CustomDictionaryRepository.maxLearnCount, "an untrusted backup count is clamped")
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
        XCTAssertEqual(CustomDictionaryCapacityPolicy.maxLearnedEntries, 2000, "must stay aligned with Android MAX_LEARNED_ENTRIES")
        // Fill to the cap; 詞0 composed twice must survive; the row just
        // written always survives; one of the count-1 rows goes (they share a
        // second-resolution `updated_at`, so which one is not asserted), and
        // its side keys go with it.
        for i in 0 ..< Self.learnedCap {
            try await repository.learnPhrase(hanzi: "詞\(i)", canonicalTl: "su-\(i)")
        }
        try await repository.learnPhrase(hanzi: "詞0", canonicalTl: "su-0")
        try await repository.learnPhrase(hanzi: "新詞", canonicalTl: "sin-su")

        let rows = try await learnedRows()
        XCTAssertEqual(rows.count, Self.learnedCap, "learned rows stay at the cap; got \(rows.map(\.hanzi))")
        XCTAssertTrue(rows.contains { $0.hanzi == "詞0" }, "the twice-composed row survives")
        XCTAssertTrue(rows.contains { $0.hanzi == "新詞" }, "the newest learn is kept")
        let survivingOnes = rows.filter { $0.hanzi == "詞1" || $0.hanzi == "詞2" }
        XCTAssertEqual(survivingOnes.count, 1, "exactly one count-1 row is evicted; got \(rows.map(\.hanzi))")
        try await repository.ensureInitialized()
        let evicted = survivingOnes.first?.hanzi == "詞1" ? "su2" : "su1"
        XCTAssertTrue(try learnedExact(evicted).isEmpty, "the evicted row's side keys are gone")
    }
}
