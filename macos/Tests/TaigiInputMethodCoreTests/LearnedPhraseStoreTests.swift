// Learned phrases (§50) in their own file: what the store learns, recalls,
// evicts and forgets — and that the custom dictionary keeps its dev-only
// leftovers out.

@testable import TaigiInputMethodCore
import XCTest

/// Real SQLite in a scratch directory, the engine's own key derivation, so
/// the exact whole-buffer recall reads the store the way the ranker does.
final class LearnedPhraseStoreTests: XCTestCase {
    private func makeStore(limit: Int = LearnedPhraseStore.maxEntries) throws -> LearnedPhraseStore {
        let store = LearnedPhraseStore(directory: { try TestFixtures.scratchDirectory() }, limit: limit)
        store.open()
        XCTAssertTrue(TestFixtures.spinRunLoop(until: { store.isReady }, timeout: 5), "the store never opened")
        return store
    }

    private func matches(_ store: LearnedPhraseStore, _ input: String, mode: InputMode = .tl) throws -> [String] {
        let key = try XCTUnwrap(RustEngineBridge.deriveCustomQueryKey(input: input, mode: mode))
        return store.rows(matching: key).map(\.hanzi)
    }

    private func rows(_ store: LearnedPhraseStore, untilCountIs expected: Int) throws -> [LearnedPhraseRow] {
        try TestFixtures.waitFor(untilCountIs: expected) { store.allRows() }
    }

    func testLearnPhrase_twiceIsOneRowWithCountTwo_foundByTheWholeBufferOnly() throws {
        let store = try makeStore()
        store.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        store.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        store.learnPhrase(hanzi: "", canonicalTl: "kì--khí-lâi")
        store.learnPhrase(hanzi: "記起來", canonicalTl: "")

        let rows = try rows(store, untilCountIs: 1)
        XCTAssertEqual(rows, [LearnedPhraseRow(hanzi: "記起來", canonicalTl: "kì--khí-lâi", learnCount: 2)])
        XCTAssertEqual(try matches(store, "kikhilai"), ["記起來"], "exact toneless key")
        XCTAssertEqual(try matches(store, "ki3khi2lai5"), ["記起來"], "exact toned key")
        XCTAssertEqual(try matches(store, "kikhilai", mode: .poj), ["記起來"], "POJ-mode query key")
        XCTAssertTrue(try matches(store, "kikhi").isEmpty, "a strict prefix must not match")
        XCTAssertTrue(try matches(store, "kikhilaia").isEmpty, "a longer buffer must not match")
    }

    func testTouchPhrase_bumpsAKnownPairAndIgnoresAnUnknownOne_mostComposedFirst() throws {
        let store = try makeStore()
        store.learnPhrase(hanzi: "機起來", canonicalTl: "ki-khí-lâi")
        store.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        store.touchPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        store.touchPhrase(hanzi: "台語", canonicalTl: "tâi-gí")

        let rows = try rows(store, untilCountIs: 2)
        XCTAssertEqual(rows.map(\.hanzi), ["記起來", "機起來"])
        XCTAssertEqual(rows.map(\.learnCount), [2, 1])
        XCTAssertEqual(try matches(store, "kikhilai"), ["記起來", "機起來"])
    }

    func testLearnPhrase_pastTheCapEvictsTheFewestComposedRowAndItsKeys_neverTheNewest() throws {
        let cap = 3
        let store = try makeStore(limit: cap)
        for i in 0 ..< cap {
            store.learnPhrase(hanzi: "詞\(i)", canonicalTl: "su-\(i)")
        }
        store.learnPhrase(hanzi: "詞0", canonicalTl: "su-0")
        store.learnPhrase(hanzi: "新詞", canonicalTl: "sin-su")

        let hanzi = try rows(store, untilCountIs: cap).map(\.hanzi)
        XCTAssertTrue(hanzi.contains("詞0"), "the twice-composed row survives; got \(hanzi)")
        XCTAssertTrue(hanzi.contains("新詞"), "the newest learn is kept; got \(hanzi)")
        let evicted = hanzi.contains("詞1") ? "su2" : "su1"
        XCTAssertTrue(try matches(store, evicted).isEmpty, "the evicted row's keys are gone")
    }

    func testDeleteAll_clearsRowsAndKeys_andTheStoreLearnsAgain() async throws {
        let store = try makeStore()
        store.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        _ = try rows(store, untilCountIs: 1)

        let removed = try await store.deleteAll()
        XCTAssertEqual(removed, 1)
        XCTAssertEqual(store.allRows(), [])
        XCTAssertTrue(try matches(store, "kikhilai").isEmpty, "the keys went with the rows")
        store.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")
        _ = try rows(store, untilCountIs: 1)
        XCTAssertEqual(try matches(store, "kikhilai"), ["記起來"])
    }
}
