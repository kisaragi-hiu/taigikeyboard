import SQLite3
@testable import TaigiKeyboard
import XCTest

/// Learned phrases (§50) on their own `learned_phrases.db`: a learned
/// `(hanzi, roman)` pair is one row with a `learn_count`, never a duplicate,
/// found only by an EXACT whole-buffer key, evicted fewest-then-oldest past
/// the cap, and cleared as learning data (USER 2026-09-21: not the custom
/// dictionary, no UI, no backup).
///
/// Exercises the real repository against a temp-file `SQLiteConnectionManager`
/// (the Android counterpart cannot run SQLite in a JVM test; it pins the
/// shared SQL + S62 dogfood).
final class LearnedPhraseRepositoryTests: XCTestCase {
    private var dbPath: String!
    private var repository: LearnedPhraseRepository!

    /// Small cap so eviction is a handful of inserts, not 2001.
    private static let cap = 3

    override func setUpWithError() throws {
        try super.setUpWithError()
        dbPath = NSTemporaryDirectory()
            .appending("learned_phrases_\(UUID().uuidString).db")
        let path = dbPath!
        let manager = SQLiteConnectionManager(
            databasePath: { path },
            queueLabel: "test.learnedphrases.\(UUID().uuidString)",
            loggerCategory: "LearnedPhraseRepositoryTests",
        )
        repository = LearnedPhraseRepository(connectionManager: manager, maxEntries: Self.cap)
    }

    override func tearDownWithError() throws {
        repository = nil
        for suffix in ["", "-wal", "-shm", "-journal"] where dbPath != nil {
            let path = dbPath! + suffix
            if FileManager.default.fileExists(atPath: path) {
                try? FileManager.default.removeItem(atPath: path)
            }
        }
        dbPath = nil
        try super.tearDownWithError()
    }

    private func matches(_ input: String, mode: InputMode = .tl) throws -> [LearnedPhrase] {
        let q = try XCTUnwrap(CustomDictionaryDerivation.queryKey(for: input, mode: mode))
        return repository.matchesSync(family: q.family, form: q.form, key: q.key)
    }

    // MARK: - Learning

    func test_learnPhrase_twiceIsOneRowWithCountTwo() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")

        let rows = try await repository.allPhrases()
        XCTAssertEqual(
            rows,
            [LearnedPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi", learnCount: 2)],
            "the (hanzi, roman) unique constraint folds the second learn into the first row",
        )
    }

    func test_learnPhrase_emptyPartsAreIgnored() async throws {
        try await repository.learnPhrase(hanzi: "", canonicalTl: "kì--khí-lâi")
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "")
        let rows = try await repository.allPhrases()
        XCTAssertTrue(rows.isEmpty, "got \(rows)")
    }

    // MARK: - Recall

    func test_matchesSync_matchesTheWholeBufferOnly() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        // The sync path answers only once the connection is open.
        try await repository.ensureInitialized()

        XCTAssertEqual(try matches("kikhilai").map(\.hanzi), ["記起來"], "exact toneless key")
        XCTAssertEqual(try matches("ki3khi2lai5").map(\.hanzi), ["記起來"], "exact toned key")
        XCTAssertEqual(try matches("kikhilai", mode: .poj).map(\.hanzi), ["記起來"], "POJ-mode query key")
        XCTAssertTrue(try matches("kikhi").isEmpty, "a strict prefix of the phrase must not match")
        XCTAssertTrue(try matches("kikhilaia").isEmpty, "a longer buffer must not match")
    }

    func test_matchesSync_ordersMostComposedFirst() async throws {
        try await repository.learnPhrase(hanzi: "機起來", canonicalTl: "ki-khí-lâi")
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.ensureInitialized()
        XCTAssertEqual(try matches("kikhilai").map(\.hanzi), ["記起來", "機起來"])
    }

    // MARK: - Touch + eviction

    func test_touchPhrase_bumpsTheRowAndIgnoresUnknownPairs() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.touchPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.touchPhrase(hanzi: "台語", canonicalTl: "tâi-gí")

        let rows = try await repository.allPhrases()
        XCTAssertEqual(rows, [LearnedPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi", learnCount: 2)])
    }

    func test_evictsFewestComposedThenOldestPastTheCap() async throws {
        XCTAssertEqual(LearnedPhraseRepository.maxEntries, 2000, "must stay aligned with Android MAX_ENTRIES")
        // Fill to the cap; 詞0 composed twice must survive; the row just
        // written always survives; one of the count-1 rows goes (they share a
        // second-resolution `updated_at`, so which one is not asserted), and
        // its side keys go with it.
        for i in 0 ..< Self.cap {
            try await repository.learnPhrase(hanzi: "詞\(i)", canonicalTl: "su-\(i)")
        }
        try await repository.learnPhrase(hanzi: "詞0", canonicalTl: "su-0")
        try await repository.learnPhrase(hanzi: "新詞", canonicalTl: "sin-su")

        let rows = try await repository.allPhrases()
        XCTAssertEqual(rows.count, Self.cap, "rows stay at the cap; got \(rows.map(\.hanzi))")
        XCTAssertTrue(rows.contains { $0.hanzi == "詞0" }, "the twice-composed row survives")
        XCTAssertTrue(rows.contains { $0.hanzi == "新詞" }, "the newest learn is kept")
        let survivingOnes = rows.filter { $0.hanzi == "詞1" || $0.hanzi == "詞2" }
        XCTAssertEqual(survivingOnes.count, 1, "exactly one count-1 row is evicted; got \(rows.map(\.hanzi))")
        let evicted = survivingOnes.first?.hanzi == "詞1" ? "su2" : "su1"
        XCTAssertTrue(try matches(evicted).isEmpty, "the evicted row's side keys are gone")
    }

    // MARK: - Wipe

    func test_deleteAll_clearsRowsAndKeys() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        try await repository.deleteAll()

        let rows = try await repository.allPhrases()
        XCTAssertTrue(rows.isEmpty, "got \(rows)")
        XCTAssertTrue(try matches("kikhilai").isEmpty, "the keys went with the rows")
        // The store keeps working after a wipe (rows, not the file, were deleted).
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        XCTAssertEqual(try matches("kikhilai").map(\.hanzi), ["記起來"])
    }
}
