@testable import TaigiKeyboard
import XCTest

/// Dictionary-tab search policy pins, mirrored by Android
/// `DictionarySearchServiceTest`. The lexicon and the custom dictionary are
/// fakes, so only the platform-side policy is observed: the custom-dictionary
/// toggle gates the tab as it gates the keyboard, and the TPS layout searches
/// the `tps:` family.
final class DictionarySearchServiceTests: XCTestCase {
    private final class FakeLexicon: LexiconClient, @unchecked Sendable {
        private let rows: [RustEngineBridge.LexiconRow]
        private(set) var romanModes: [RustEngineBridge.LexiconInputMode] = []
        private(set) var hanziModes: [RustEngineBridge.LexiconInputMode] = []

        init(rows: [RustEngineBridge.LexiconRow] = []) {
            self.rows = rows
        }

        /// CJK by the BMP range only — enough for the fixtures; production asks the engine.
        func isHanzi(_ text: String) -> Bool {
            text.unicodeScalars.contains { (0x4E00 ... 0x9FFF).contains($0.value) }
        }

        func dictionaryFilters(toggles _: RustEngineBridge.DictionaryToggles) -> RustEngineBridge.DictionaryFilters {
            .allSourcesEnabled
        }

        func searchWithSources(
            input _: String,
            inputMode: RustEngineBridge.LexiconInputMode,
            limit _: UInt32,
            enabledSourcesBitmask _: UInt32,
        ) -> [RustEngineBridge.LexiconRow] {
            romanModes.append(inputMode)
            return rows
        }

        func searchByHanzi(
            query _: String,
            inputMode: RustEngineBridge.LexiconInputMode,
            limit _: UInt32,
            enabledSourcesBitmask _: UInt32,
        ) -> [RustEngineBridge.LexiconRow] {
            hanziModes.append(inputMode)
            return rows
        }
    }

    private final class FakeUserData: UserDataClient, @unchecked Sendable {
        private let entries: [CustomDictionaryEntry]
        private(set) var searchCalls = 0

        init(entries: [CustomDictionaryEntry]) {
            self.entries = entries
        }

        func search(query: String, mode _: InputMode, limit _: Int) async -> [CustomDictionaryEntry] {
            searchCalls += 1
            return entries.filter { $0.roman.hasPrefix(query) }
        }

        func listAll() async throws -> [CustomDictionaryEntry] {
            entries
        }

        func save(_: CustomDictionaryEntry) async throws {
            XCTFail("unused")
        }

        func delete(id _: String) async throws {
            XCTFail("unused")
        }

        func deleteAll() async throws {
            XCTFail("unused")
        }

        func exportCSV() async throws -> Data {
            XCTFail("unused")
            return Data()
        }

        func importCSV(url _: URL) async throws -> (imported: Int, skipped: Int) {
            XCTFail("unused")
            return (0, 0)
        }

        func clearLearningRecords() async throws {
            XCTFail("unused")
        }

        func exportBackup(appVersion _: String) async throws -> Data {
            XCTFail("unused")
            return Data()
        }

        func importBackup(url _: URL) async throws -> BackupImportResult {
            XCTFail("unused")
            throw CancellationError()
        }
    }

    private let myWord = CustomDictionaryEntry(roman: "taigi", hanzi: "我的台語")
    private let kautianRow = RustEngineBridge.LexiconRow(id: 1, roman: "tâi-gí", hanzi: "台語", lengthScore: 10, sourceBitmask: 1)

    private func makeService(
        settings: StubEngineSettings = StubEngineSettings(),
        lexicon: FakeLexicon = FakeLexicon(),
        userData: FakeUserData,
    ) -> DictionarySearchService {
        DictionarySearchService(lexicon: lexicon, userData: userData, settingsProvider: StubEngineSettingsProvider(settings))
    }

    // CROSS-PLATFORM INVARIANT — mirrors Android DictionarySearchServiceTest
    // `custom dictionary off - its entries are neither searched nor listed`.
    func testCustomDictionaryOff_itsEntriesAreNotSearched() async throws {
        var settings = StubEngineSettings()
        settings.isCustomDictEnabled = false
        let userData = FakeUserData(entries: [myWord])

        let results = try await makeService(settings: settings, userData: userData).search(query: "taigi")

        XCTAssertEqual(userData.searchCalls, 0)
        XCTAssertFalse(results.contains { $0.sources == [.custom] })
    }

    // CROSS-PLATFORM INVARIANT — mirrors Android `custom dictionary on - its entries lead the system rows`.
    func testCustomDictionaryOn_itsEntriesLeadTheSystemRows() async throws {
        let userData = FakeUserData(entries: [myWord])

        let results = try await makeService(lexicon: FakeLexicon(rows: [kautianRow]), userData: userData).search(query: "taigi")

        XCTAssertEqual(userData.searchCalls, 1)
        XCTAssertEqual(results.first?.sources, [.custom])
        XCTAssertEqual(results.first?.id, DictionarySearchResult.customDictMarkerId)
        XCTAssertEqual(results.last?.sources, [.kautian])
        XCTAssertEqual(results.count, 2)
    }

    func testAHanjiQuery_takesTheHanziPathAndNeverConsultsTheCustomDictionary() async throws {
        let lexicon = FakeLexicon()
        let userData = FakeUserData(entries: [CustomDictionaryEntry(roman: "taigi", hanzi: "台語")])

        _ = try await makeService(lexicon: lexicon, userData: userData).search(query: "台語")

        XCTAssertEqual(lexicon.hanziModes.count, 1)
        XCTAssertTrue(lexicon.romanModes.isEmpty)
        XCTAssertEqual(userData.searchCalls, 0)
    }

    // CROSS-PLATFORM INVARIANT — mirrors Android `the TPS layout searches the TPS family`.
    func testTpsLayout_searchesTheTpsFamily() async throws {
        var settings = StubEngineSettings()
        settings.inputMode = .tps
        let lexicon = FakeLexicon()

        _ = try await makeService(settings: settings, lexicon: lexicon, userData: FakeUserData(entries: [])).search(query: "ㄉㄞ")

        XCTAssertEqual(lexicon.romanModes, [.tps])
    }

    func testLexiconMode_mapsEveryInputMode() {
        XCTAssertEqual(DictionarySearchService.lexiconMode(.tl), .tl)
        XCTAssertEqual(DictionarySearchService.lexiconMode(.poj), .poj)
        XCTAssertEqual(DictionarySearchService.lexiconMode(.tps), .tps)
        XCTAssertEqual(DictionarySearchService.lexiconMode(.english), .tl)
    }
}
