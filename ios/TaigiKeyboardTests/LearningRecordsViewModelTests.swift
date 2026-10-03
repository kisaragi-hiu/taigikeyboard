@testable import TaigiKeyboard
import XCTest

/// Learning Records page policy over a fake engine: rows are paged by the
/// engine (100 at a time, the next page when the last row shows), a change
/// of kind / order / filter lists from the top and drops answers still in
/// flight, every write reloads, and a row already gone is a notice, not a
/// failure. Mirrors `desktop-core` `settings/learning_records.rs` + `listing.rs`.
@MainActor
final class LearningRecordsViewModelTests: XCTestCase {
    /// The engine's user-data surface, answering from `rows` with the
    /// engine's paging rule — or, while `isHolding`, parking each list
    /// request until the test resolves it.
    private final class FakeLearningRecords: UserDataClient, @unchecked Sendable {
        struct ListCall: Equatable {
            let kind: Taigi_Engine_LearningRecordKind
            let order: Taigi_Engine_LearningRecordOrder
            let filter: String
            let limit: UInt32
            let offset: UInt32
        }

        struct Unreadable: LocalizedError {
            var errorDescription: String? {
                "disk I/O error"
            }
        }

        private let lock = NSLock()
        private var storedRows: [Taigi_Engine_LearningRecord]
        private var storedCalls: [ListCall] = []
        private var parked: [(call: ListCall, answer: CheckedContinuation<Taigi_Engine_LearningRecords, Never>)] = []
        private var storedIsHolding = false
        private var storedFailure: Unreadable?

        init(rows: [Taigi_Engine_LearningRecord]) {
            storedRows = rows
        }

        var rows: [Taigi_Engine_LearningRecord] {
            get { lock.withLock { storedRows } }
            set { lock.withLock { storedRows = newValue } }
        }

        var calls: [ListCall] {
            lock.withLock { storedCalls }
        }

        var isHolding: Bool {
            get { lock.withLock { storedIsHolding } }
            set { lock.withLock { storedIsHolding = newValue } }
        }

        /// Every request fails while set.
        var failure: Unreadable? {
            get { lock.withLock { storedFailure } }
            set { lock.withLock { storedFailure = newValue } }
        }

        var parkedCount: Int {
            lock.withLock { parked.count }
        }

        /// Answers the parked request at `index` from the rows as they are now.
        func resolve(_ index: Int) {
            let (call, answer) = lock.withLock { parked.remove(at: index) }
            answer.resume(returning: page(for: call))
        }

        func listLearningRecords(
            kind: Taigi_Engine_LearningRecordKind,
            order: Taigi_Engine_LearningRecordOrder,
            filter: String,
            limit: UInt32,
            offset: UInt32,
        ) async throws -> Taigi_Engine_LearningRecords {
            let call = ListCall(kind: kind, order: order, filter: filter, limit: limit, offset: offset)
            let (isHolding, failure) = lock.withLock {
                storedCalls.append(call)
                return (storedIsHolding, storedFailure)
            }
            if let failure {
                throw failure
            }
            guard isHolding else { return page(for: call) }
            return await withCheckedContinuation { answer in
                lock.withLock { parked.append((call, answer)) }
            }
        }

        func setLearningRecordCount(_ record: Taigi_Engine_LearningRecord, count: Int64) async throws -> Bool {
            if let failure {
                throw failure
            }
            return lock.withLock {
                guard let index = storedRows.firstIndex(where: { $0.isSameRow(as: record) }) else { return false }
                storedRows[index].count = count
                return true
            }
        }

        func deleteLearningRecord(_ record: Taigi_Engine_LearningRecord) async throws -> Bool {
            if let failure {
                throw failure
            }
            return lock.withLock {
                guard let index = storedRows.firstIndex(where: { $0.isSameRow(as: record) }) else { return false }
                storedRows.remove(at: index)
                return true
            }
        }

        /// The engine's page: substring filter on text / TL, an offset past
        /// the end pulled back to the last page (`engine/userdata/src/paging.rs`).
        private func page(for call: ListCall) -> Taigi_Engine_LearningRecords {
            let ofKind = rows.filter { $0.kind == call.kind }
            let matching = ofKind.filter {
                call.filter.isEmpty || $0.text.contains(call.filter) || $0.tl.contains(call.filter)
            }
            let limit = Int(call.limit)
            var offset = Int(call.offset)
            if offset >= matching.count, offset > 0 {
                offset = max(matching.count - 1, 0) / limit * limit
            }
            var page = Taigi_Engine_LearningRecords()
            page.records = Array(matching.dropFirst(offset).prefix(limit))
            page.total = UInt32(ofKind.count)
            page.matchingTotal = UInt32(matching.count)
            page.offset = UInt32(offset)
            return page
        }

        func listAll() async throws -> [CustomDictionaryEntry] {
            XCTFail("unused")
            return []
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

        func search(query _: String, mode _: InputMode, limit _: Int) async -> [CustomDictionaryEntry] {
            XCTFail("unused")
            return []
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

    private func record(
        _ id: Int64,
        _ text: String,
        tl: String = "",
        kind: Taigi_Engine_LearningRecordKind = .frequency,
    ) -> Taigi_Engine_LearningRecord {
        var record = Taigi_Engine_LearningRecord()
        record.kind = kind
        record.id = id
        record.text = text
        record.tl = tl
        record.count = 3
        return record
    }

    private func numberedRows(_ count: Int) -> [Taigi_Engine_LearningRecord] {
        (1 ... count).map { record(Int64($0), "詞\($0)") }
    }

    private func makeViewModel(_ fake: FakeLearningRecords) -> LearningRecordsViewModel {
        LearningRecordsViewModel(userData: fake, filterSettle: .zero)
    }

    /// Yields until `count` list requests are parked in `fake`.
    private func waitForParked(_ count: Int, in fake: FakeLearningRecords) async {
        for _ in 0 ..< 10000 where fake.parkedCount < count {
            await Task.yield()
        }
        XCTAssertEqual(fake.parkedCount, count, "list requests parked")
    }

    // MARK: - Paging

    func testPaging_theNextPageAppendsWhenTheLastRowShows() async {
        // trace: 250 rows, pages of 100 → offsets 0, 100, 200; the third page holds 50.
        let fake = FakeLearningRecords(rows: numberedRows(250))
        let viewModel = makeViewModel(fake)

        await viewModel.load()
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertEqual(viewModel.records.map(\.id), Array(1 ... 100))

        await viewModel.loadNextPage()
        await viewModel.loadNextPage()
        XCTAssertEqual(viewModel.records.map(\.id), Array(1 ... 250))
        XCTAssertFalse(viewModel.hasMoreRows)

        await viewModel.loadNextPage()
        XCTAssertEqual(fake.calls.map(\.offset), [0, 100, 200], "nothing past the matches")
        XCTAssertEqual(fake.calls.map(\.limit), [100, 100, 100])
    }

    func testPaging_matchesShrunkUnderTheList_reloadsWhatIsListed() async {
        // trace: 150 rows → 100 listed; 60 go behind the page's back → 90 left. Offset 100
        // is past the end, so the engine serves offset 0 (last_page_offset(90, 100, 100) = 0),
        // which does not line up: the list reloads from the top, 100 rows → the 90 left.
        let fake = FakeLearningRecords(rows: numberedRows(150))
        let viewModel = makeViewModel(fake)
        await viewModel.load()
        fake.rows = Array(fake.rows.prefix(90))

        await viewModel.loadNextPage()

        XCTAssertEqual(fake.calls.map(\.offset), [0, 100, 0])
        XCTAssertEqual(viewModel.records.map(\.id), Array(1 ... 90))
    }

    func testThePagingKey_changesAfterAReloadThatKeepsTheLastRow() async {
        // trace: 150 rows, 100 listed, tail id 100. A reorder answers the same 100 rows
        // (the fake ignores order), so the tail row keeps its id — the key must still move.
        let fake = FakeLearningRecords(rows: numberedRows(150))
        let viewModel = makeViewModel(fake)
        await viewModel.load()
        let before = viewModel.pagingKey

        fake.isHolding = true
        let reorder = viewModel.selectOrder(.mostRecent)
        // Asked while the rows answer the old order: nothing is asked.
        await viewModel.loadNextPage()
        await waitForParked(1, in: fake)
        fake.resolve(0)
        await reorder.value
        fake.isHolding = false

        XCTAssertEqual(viewModel.records.last?.id, 100)
        XCTAssertNotEqual(viewModel.pagingKey, before, "the sentinel task re-fires")
        await viewModel.loadNextPage()
        XCTAssertEqual(viewModel.records.count, 150)
        XCTAssertEqual(fake.calls.map(\.offset), [0, 0, 100])
    }

    func testAFailedNextPage_waitsForARetry() async {
        let fake = FakeLearningRecords(rows: numberedRows(150))
        let viewModel = makeViewModel(fake)
        await viewModel.load()

        fake.failure = FakeLearningRecords.Unreadable()
        await viewModel.loadNextPage()
        XCTAssertTrue(viewModel.nextPageFailed)
        XCTAssertEqual(viewModel.records.count, 100)

        fake.failure = nil
        await viewModel.loadNextPage()
        XCTAssertFalse(viewModel.nextPageFailed)
        XCTAssertEqual(viewModel.records.map(\.id), Array(1 ... 150))
    }

    // MARK: - Kind / order / filter

    func testKindOrderAndFilter_goToTheEngineAndListFromTheTop() async {
        let fake = FakeLearningRecords(rows: [
            record(1, "台灣", tl: "tâi-uân"),
            record(2, "食飯", tl: "tsia̍h-pn̄g"),
            record(3, "台語", tl: "tâi-gí", kind: .learnedPhrase),
        ])
        let viewModel = makeViewModel(fake)
        await viewModel.load()

        await viewModel.filterChanged("台").value
        XCTAssertEqual(viewModel.records.map(\.text), ["台灣"])

        await viewModel.selectKind(.learnedPhrase).value
        XCTAssertEqual(viewModel.records.map(\.text), ["台語"])

        await viewModel.selectOrder(.mostRecent).value
        XCTAssertEqual(fake.calls.last, .init(kind: .learnedPhrase, order: .mostRecent, filter: "台", limit: 100, offset: 0))
        XCTAssertEqual(fake.calls.map(\.offset), [0, 0, 0, 0])
    }

    func testANewerRequestWins_anOlderAnswerIsDropped() async {
        let fake = FakeLearningRecords(rows: [
            record(1, "台灣"),
            record(2, "食飯"),
            record(3, "台語", kind: .learnedPhrase),
        ])
        let viewModel = makeViewModel(fake)
        await viewModel.load()
        fake.isHolding = true

        let toPhrases = viewModel.selectKind(.learnedPhrase)
        await waitForParked(1, in: fake)
        let toFrequency = viewModel.selectKind(.frequency)
        await waitForParked(2, in: fake)

        // The newer (frequency) answer lands first; the older (phrases) one after it.
        fake.resolve(1)
        await toFrequency.value
        fake.resolve(0)
        await toPhrases.value

        XCTAssertEqual(viewModel.records.map(\.text), ["台灣", "食飯"], "the phrases answer is stale")
    }

    func testAFilterChange_dropsTheAnswerAlreadyInFlight() async {
        let fake = FakeLearningRecords(rows: [record(1, "台灣"), record(2, "食飯")])
        let viewModel = makeViewModel(fake)
        fake.isHolding = true

        let unfiltered = viewModel.selectOrder(.mostRecent)
        await waitForParked(1, in: fake)
        // Stale at the keystroke, before the box settles.
        let filtered = viewModel.filterChanged("食")
        fake.resolve(0)
        await unfiltered.value
        XCTAssertTrue(viewModel.records.isEmpty, "the unfiltered answer is dropped")

        await waitForParked(1, in: fake)
        fake.resolve(0)
        await filtered.value
        XCTAssertEqual(viewModel.records.map(\.text), ["食飯"])
    }

    func testAKindChange_isNotGivenTheOldListsNextPage() async {
        let fake = FakeLearningRecords(rows: numberedRows(150))
        let viewModel = makeViewModel(fake)
        await viewModel.load()
        fake.isHolding = true

        let toPhrases = viewModel.selectKind(.learnedPhrase)
        await viewModel.loadNextPage()
        await waitForParked(1, in: fake)
        fake.resolve(0)
        await toPhrases.value

        XCTAssertEqual(fake.calls.map(\.offset), [0, 0], "no next page while the old kind is listed")
        XCTAssertTrue(viewModel.records.isEmpty)
    }

    // MARK: - Writes

    func testSetCountAndDelete_reloadTheListedRows() async {
        let fake = FakeLearningRecords(rows: numberedRows(3))
        let viewModel = makeViewModel(fake)
        await viewModel.load()

        await viewModel.setCount(viewModel.records[0], to: 40)
        XCTAssertEqual(viewModel.records.map(\.count), [40, 3, 3])

        await viewModel.delete(viewModel.records[1])
        XCTAssertEqual(viewModel.records.map(\.id), [1, 3])
        XCTAssertNil(viewModel.notice)
        // trace: the reloads ask for max(100, rows listed) = 100 from the top.
        XCTAssertEqual(fake.calls.map(\.limit), [100, 100, 100])
    }

    func testAWrite_reReadsTheListedRowsInPagesOfAtMost100() async {
        // trace: 250 rows listed after offsets 0, 100, 200. The reload re-reads
        // max(100, 250) = 250 rows as limits 100, 100, 50 from offsets 0, 100, 200.
        let fake = FakeLearningRecords(rows: numberedRows(250))
        let viewModel = makeViewModel(fake)
        await viewModel.load()
        await viewModel.loadNextPage()
        await viewModel.loadNextPage()

        await viewModel.setCount(viewModel.records[200], to: 9)

        let reload = fake.calls.dropFirst(3)
        XCTAssertEqual(reload.map(\.offset), [0, 100, 200])
        XCTAssertEqual(reload.map(\.limit), [100, 100, 50])
        XCTAssertEqual(viewModel.records.map(\.id), Array(1 ... 250))
        XCTAssertEqual(viewModel.records[200].count, 9)
    }

    func testARowAlreadyGone_isANoticeAndTheListReloads() async {
        let fake = FakeLearningRecords(rows: numberedRows(2))
        let viewModel = makeViewModel(fake)
        await viewModel.load()
        let listed = viewModel.records[0]
        fake.rows = [fake.rows[1]]

        await viewModel.delete(listed)

        XCTAssertEqual(viewModel.notice, .gone)
        XCTAssertEqual(viewModel.records.map(\.id), [2])
    }

    func testFailures_carryTheEngineDetail() async {
        let fake = FakeLearningRecords(rows: numberedRows(2))
        let viewModel = makeViewModel(fake)
        await viewModel.load()
        fake.failure = FakeLearningRecords.Unreadable()

        await viewModel.setCount(viewModel.records[0], to: 5)
        XCTAssertEqual(viewModel.notice, .writeFailed(detail: "disk I/O error"), "the write's notice outlives its reload's")
        XCTAssertEqual(viewModel.records.map(\.id), [1, 2], "a failed read keeps the rows on screen")

        viewModel.notice = nil
        await viewModel.selectOrder(.mostRecent).value
        XCTAssertEqual(viewModel.notice, .readFailed(detail: "disk I/O error"))
        XCTAssertEqual(viewModel.records.map(\.id), [1, 2])
    }

    func testAnUnreadableStore_isNotNothingLearned() async {
        let fake = FakeLearningRecords(rows: numberedRows(2))
        fake.failure = FakeLearningRecords.Unreadable()
        let viewModel = makeViewModel(fake)

        await viewModel.load()
        XCTAssertTrue(viewModel.records.isEmpty)
        XCTAssertTrue(viewModel.lastLoadFailed, "could not read ≠ nothing learned yet")

        fake.failure = nil
        await viewModel.selectOrder(.mostRecent).value
        XCTAssertFalse(viewModel.lastLoadFailed)
        XCTAssertEqual(viewModel.records.map(\.id), [1, 2])
    }

    // MARK: - Edit field and labels

    func testCountFromText_isAWholeNumberOfAtLeastOne() {
        let cases: [(String, Int64?)] = [
            ("12", 12), (" 3 ", 3), ("1", 1), ("1000000", 1_000_000), ("1000001", nil),
            ("0", nil), ("-1", nil), ("1.5", nil), ("", nil), ("abc", nil),
        ]
        for (text, expected) in cases {
            XCTAssertEqual(LearningRecordsViewModel.count(from: text), expected, "count(from: \"\(text)\")")
        }
    }

    func testAnAssociationRow_readsPreviousThenNext() {
        var association = record(1, "愛", tl: "ài", kind: .association)
        association.previousText = "我"
        association.previousTl = "guá"
        XCTAssertEqual(association.wordLabel, "我 → 愛")
        XCTAssertEqual(association.readingLabel, "guá → ài")

        let word = record(2, "台灣", tl: "tâi-uân")
        XCTAssertEqual(word.wordLabel, "台灣")
        XCTAssertEqual(word.readingLabel, "tâi-uân")
        XCTAssertEqual(word.lastUsedLabel, "", "no readable time")
    }
}

private extension Taigi_Engine_LearningRecord {
    /// The engine's identity guard: the id AND the identity columns.
    func isSameRow(as other: Self) -> Bool {
        kind == other.kind && id == other.id && text == other.text && tl == other.tl
            && previousText == other.previousText && previousTl == other.previousTl
    }
}
