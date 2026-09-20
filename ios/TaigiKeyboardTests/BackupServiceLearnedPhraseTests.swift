@testable import TaigiKeyboard
import XCTest

/// Learned phrases (§50) through the `.taigi` backup: v3 rows carry
/// `origin` / `learnCount`; an older file is all manual; a manual row in the
/// file promotes an existing learned row (manual wins, never the reverse); a
/// learned row goes through the learn path (cap, eviction, no manual
/// downgrade); a pair repeated inside the file does not trip the learned-pair
/// unique index.
///
/// Only the custom-dictionary section is exercised — the frequency /
/// association arrays are empty, so those repositories are never opened.
final class BackupServiceLearnedPhraseTests: XCTestCase {
    private var dbPath: String!
    private var repository: CustomDictionaryRepository!
    private var backup: BackupService!

    override func setUpWithError() throws {
        try super.setUpWithError()
        dbPath = NSTemporaryDirectory()
            .appending("backup_learned_\(UUID().uuidString).db")
        let path = dbPath!
        let manager = SQLiteConnectionManager(
            databasePath: { path },
            queueLabel: "test.backup.learned.\(UUID().uuidString)",
            loggerCategory: "BackupServiceLearnedPhraseTests",
        )
        repository = CustomDictionaryRepository(connectionManager: manager)
        backup = BackupService(
            customDictionaryService: CustomDictionaryService(repository: repository),
            userFrequencyRepository: UserFrequencyRepository(),
            nextWordService: NextWordService(settingsProvider: SharedSettings.shared),
        )
    }

    override func tearDownWithError() throws {
        try? repository.deleteDatabase()
        repository = nil
        backup = nil
        for suffix in ["", "-wal", "-shm"] where dbPath != nil {
            let path = dbPath! + suffix
            if FileManager.default.fileExists(atPath: path) {
                try? FileManager.default.removeItem(atPath: path)
            }
        }
        dbPath = nil
        try super.tearDownWithError()
    }

    private func file(version: Int, rows: [[String: Any]]) throws -> Data {
        let json: [String: Any] = [
            "version": version,
            "exportedAt": "2026-09-20T00:00:00Z",
            "platform": "ios",
            "appVersion": "test",
            "customDictionary": rows,
            "userFrequency": [],
            "userAssociation": [],
        ]
        return try JSONSerialization.data(withJSONObject: json)
    }

    func test_v2FileImportsEveryRowAsManual() async throws {
        let data = try file(version: 2, rows: [["roman": "kì--khí-lâi", "hanzi": "記起來"]])
        let result = try await backup.importAll(from: data)
        XCTAssertEqual(result.customDict, 1)
        let all = try await repository.fetchAll()
        XCTAssertEqual(all.map(\.origin), [.manual])
        XCTAssertEqual(all.first?.learnCount, 0)
    }

    func test_v3LearnedRowImportsWithItsCountAndFoldsAnInFileDuplicate() async throws {
        let data = try file(version: 3, rows: [
            ["roman": "kì--khí-lâi", "hanzi": "記起來", "origin": 1, "learnCount": 4],
            ["roman": "kì--khí-lâi", "hanzi": "記起來", "origin": 1, "learnCount": 9],
        ])
        let result = try await backup.importAll(from: data)
        XCTAssertEqual(result.customDict, 1, "the second copy of the pair is skipped, not a unique-index error")
        let all = try await repository.fetchAll()
        XCTAssertEqual(all.count, 1)
        XCTAssertEqual(all.first?.origin, .learned)
        XCTAssertEqual(all.first?.learnCount, 4)
    }

    func test_manualRowInFileTakesOverAnExistingLearnedRow() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi")

        let data = try file(version: 3, rows: [["roman": "kì--khí-lâi", "hanzi": "記起來", "origin": 0]])
        let result = try await backup.importAll(from: data)
        XCTAssertEqual(result.customDict, 1)
        let all = try await repository.fetchAll()
        XCTAssertEqual(all.count, 1, "one row for the pair; got \(all)")
        XCTAssertEqual(all.first?.origin, .manual, "manual wins over learned")
        XCTAssertEqual(all.first?.learnCount, 0)
    }

    func test_learnedRowInFileNeverDowngradesAnExistingManualRow() async throws {
        try await repository.upsert(CustomDictionaryEntry(roman: "kì--khí-lâi", hanzi: "記起來"))
        let data = try file(version: 3, rows: [["roman": "kì--khí-lâi", "hanzi": "記起來", "origin": 1, "learnCount": 3]])
        let result = try await backup.importAll(from: data)
        XCTAssertEqual(result.customDict, 0)
        let all = try await repository.fetchAll()
        XCTAssertEqual(all.map(\.origin), [.manual])
    }

    func test_exportCarriesProvenance() async throws {
        try await repository.learnPhrase(hanzi: "記起來", canonicalTl: "kì--khí-lâi", count: 2)
        try await repository.upsert(CustomDictionaryEntry(roman: "tâi-gí", hanzi: "台語"))
        let data = try await backup.exportAll()
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(json["version"] as? Int, 3)
        let custom = try XCTUnwrap(json["customDictionary"] as? [[String: Any]])
        let learned = try XCTUnwrap(custom.first { $0["hanzi"] as? String == "記起來" })
        XCTAssertEqual(learned["origin"] as? Int, 1)
        XCTAssertEqual(learned["learnCount"] as? Int, 2)
        let manual = try XCTUnwrap(custom.first { $0["hanzi"] as? String == "台語" })
        XCTAssertEqual(manual["origin"] as? Int, 0)
    }
}
