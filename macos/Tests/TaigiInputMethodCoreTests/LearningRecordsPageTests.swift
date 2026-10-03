@testable import TaigiInputMethodCore
import XCTest

/// The Learning Records page over an in-memory store that pages, filters,
/// orders and matches rows the way the engine does (`FakeUserDataClient`).
/// What the engine itself does with a request is `engine/userdata`'s subject.
@MainActor
final class LearningRecordsPageTests: XCTestCase {
    private func record(
        _ kind: Taigi_Engine_LearningRecordKind,
        id: Int64,
        count: Int64 = 1,
        lastUsedMs: Int64 = 0,
    ) -> Taigi_Engine_LearningRecord {
        var record = Taigi_Engine_LearningRecord()
        record.kind = kind
        record.id = id
        record.text = "字\(id)"
        record.tl = "ji\(id)"
        record.count = count
        record.lastUsedMs = lastUsedMs
        return record
    }

    private func makeModel(_ records: [Taigi_Engine_LearningRecord]) -> (LearningRecordsPageModel, FakeUserDataClient) {
        let client = FakeUserDataClient()
        client.seedLearningRecords(records)
        return (LearningRecordsPageModel(client: client), client)
    }

    /// The pickers' values are what the engine is asked for — word frequency
    /// first, most used first, as the page opens.
    func testLoad_sendsTheKindOrderAndFilterPickedNow() async throws {
        let (model, client) = makeModel([])

        await model.load()
        XCTAssertEqual(client.lastLearningRecordsQuery?.kind, .frequency)
        XCTAssertEqual(client.lastLearningRecordsQuery?.order, .mostUsed)

        model.kind = .learnedPhrase
        model.order = .mostRecent
        model.filter = "ji"
        await model.loadFirstPage()
        XCTAssertEqual(client.lastLearningRecordsQuery?.kind, .learnedPhrase)
        XCTAssertEqual(client.lastLearningRecordsQuery?.order, .mostRecent)
        XCTAssertEqual(client.lastLearningRecordsQuery?.filter, "ji")
    }

    /// One store's rows only: a phrase never shows under word frequency.
    func testRows_comeFromTheKindPicked() async throws {
        let (model, _) = makeModel([record(.frequency, id: 1), record(.learnedPhrase, id: 2)])

        await model.load()
        XCTAssertEqual(model.list.rows.map(\.id), [1])

        model.kind = .learnedPhrase
        await model.loadFirstPage()
        XCTAssertEqual(model.list.rows.map(\.id), [2])
        XCTAssertEqual(model.list.countLabel, "1")
    }

    /// Another kind, order or filter is another list: its first page.
    func testAChangeOfKind_startsAtPageOne() async throws {
        let size = UserDataListMetrics.pageSize
        let (model, _) = makeModel((0 ..< Int64(size + 1)).map { record(.frequency, id: $0) })
        await model.load()
        await model.pageForward()
        XCTAssertEqual(model.list.page, 1)

        model.kind = .learnedPhrase
        await model.loadFirstPage()

        XCTAssertEqual(model.list.page, 0)
        XCTAssertTrue(model.list.rows.isEmpty)
    }

    func testOrder_mostUsedThenMostRecent() async throws {
        let (model, _) = makeModel([
            record(.frequency, id: 1, count: 9, lastUsedMs: 1000),
            record(.frequency, id: 2, count: 1, lastUsedMs: 2000),
        ])

        await model.load()
        XCTAssertEqual(model.list.rows.map(\.id), [1, 2])

        model.order = .mostRecent
        await model.loadFirstPage()
        XCTAssertEqual(model.list.rows.map(\.id), [2, 1])
    }

    func testSetCount_storesItAndReloads() async throws {
        let (model, _) = makeModel([record(.frequency, id: 1, count: 3)])
        await model.load()

        try await model.setCount(of: XCTUnwrap(model.list.rows.first), to: 40)

        XCTAssertEqual(model.list.rows.first?.count, 40)
        XCTAssertNil(model.message)
        XCTAssertFalse(model.activity.isWorking)
    }

    func testDelete_removesTheRowAndReloads() async throws {
        let (model, _) = makeModel([record(.frequency, id: 1), record(.frequency, id: 2)])
        await model.load()

        try await model.delete(XCTUnwrap(model.list.rows.first { $0.id == 1 }))

        XCTAssertEqual(model.list.rows.map(\.id), [2])
        XCTAssertEqual(model.list.countLabel, "1")
        XCTAssertNil(model.message)
    }

    /// A row gone since it was listed is said as such — not a failure — and
    /// the list is reloaded so the row stops showing.
    func testAWriteToARowAlreadyGone_saysItIsGone() async throws {
        let (model, client) = makeModel([record(.frequency, id: 1)])
        await model.load()
        let listed = try XCTUnwrap(model.list.rows.first)
        client.seedLearningRecords([])

        await model.setCount(of: listed, to: 5)
        XCTAssertEqual(model.message, .done(.dictionaryLearningRecordGone))
        XCTAssertTrue(model.list.rows.isEmpty)

        model.message = nil
        await model.delete(listed)
        XCTAssertEqual(model.message, .done(.dictionaryLearningRecordGone))
    }

    /// "Could not be read" is an alert; "nothing learned yet" is an empty
    /// table and no alert.
    func testAnUnreadableStore_alertsWhereAnEmptyOneDoesNot() async throws {
        let (model, client) = makeModel([])

        await model.load()
        XCTAssertNil(model.message)

        client.failsLearningRecordReads = true
        await model.load()
        guard case .failure(.dictionaryLearningRecordsReadFailed, _)? = model.message else {
            return XCTFail("expected the read-failed alert, got \(String(describing: model.message))")
        }
    }

    /// The "above 40 ranks the same" note belongs to word frequency alone.
    func testCountNote_isForWordFrequencyOnly() {
        XCTAssertEqual(LearningRecordsPageModel.countNoteKey(for: .frequency), .dictionaryLearningRecordsCountCapInfo)
        XCTAssertNil(LearningRecordsPageModel.countNoteKey(for: .learnedPhrase))
    }

    func testLastUsedLabel_isTheDayOrEmpty() throws {
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        // trace: 1_759_449_600_000 ms = 2025-10-03T00:00:00Z
        XCTAssertEqual(LearningRecordsPageModel.lastUsedLabel(1_759_449_600_000, timeZone: utc), "2025-10-03")
        XCTAssertEqual(LearningRecordsPageModel.lastUsedLabel(0, timeZone: utc), "")
    }
}

/// The paged list both user-data pages keep: a load started before another
/// never lands over it.
final class UserDataPagedListTests: XCTestCase {
    func testAnOlderLoad_neverLandsOverANewerOne() {
        var list = UserDataPagedList<Int>()
        let older = list.beginLoad()
        let newer = list.beginLoad()

        list.land(UserDataListing(rows: [2], total: 1, matchingTotal: 1, offset: 0), from: newer)
        list.land(UserDataListing(rows: [1], total: 1, matchingTotal: 1, offset: 0), from: older)

        XCTAssertEqual(list.rows, [2])
        XCTAssertFalse(list.isCurrent(older))
        XCTAssertTrue(list.isCurrent(newer))
    }
}
