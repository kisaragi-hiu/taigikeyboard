import Foundation

/// ViewModel for `LearningRecordsView`: what the keyboard learned of one
/// kind, in pages the engine cuts
/// (`docs/architecture/learning-records-page-roadmap.md`).
///
/// The order and the filter go to the engine; the rows on screen are its
/// answer. A change of either starts a load from the top and makes every answer still in
/// flight stale — the newest request wins, as on the desktop's `Listing`. Form state (the edit alert's field) stays
/// in the view.
@MainActor
final class LearningRecordsViewModel: ObservableObject {
    /// Rows per engine page; the next page loads when the last row shows.
    /// `nonisolated`: `reload(rowCount:)`'s default argument reads it outside the main actor.
    nonisolated static let pageSize: UInt32 = 100
    /// The filter reloads once typing pauses this long — the desktop's
    /// `FILTER_SETTLE`, so a word typed letter by letter is one query.
    /// `nonisolated`: `init`'s default argument reads it outside the main actor.
    nonisolated static let filterSettle: Duration = .milliseconds(200)
    /// The highest count the engine stores (`engine/userdata` `MAX_COUNT`).
    static let maxCount: Int64 = 1_000_000

    @Published private(set) var order: Taigi_Engine_LearningRecordOrder = .mostUsed
    @Published private(set) var records: [Taigi_Engine_LearningRecord] = []
    /// Only until the first answer: later loads leave the rows on screen.
    @Published private(set) var isLoading = true
    /// The read that failed and waits for `retry()`. With no rows listed the
    /// page says "could not read", never "nothing learned yet".
    @Published private(set) var failedRead: LearningRecordsRead?
    @Published var notice: LearningRecordsNotice?
    /// The rows the current filter matches — what paging runs up to.
    @Published private(set) var matchingTotal = 0
    /// The generation the rows on screen answer. The next page is asked
    /// only while it is the current one: rows of an older order or filter must not get the new query's next page appended.
    @Published private(set) var listedGeneration = -1

    private var filter = ""
    /// Bumped by every change of what is asked; an answer started under an
    /// older number is dropped.
    private var generation = 0
    private var isLoadingNextPage = false
    private var settleTask: Task<Void, Never>?

    /// Frequency or learned phrases; fixed for the page's life.
    let kind: Taigi_Engine_LearningRecordKind
    private let userData: any UserDataClient
    private let filterSettle: Duration

    init(
        kind: Taigi_Engine_LearningRecordKind,
        userData: any UserDataClient = CompositionRoot.userData,
        filterSettle: Duration = LearningRecordsViewModel.filterSettle,
    ) {
        self.kind = kind
        self.userData = userData
        self.filterSettle = filterSettle
    }

    /// The first appearance lists the first page; a later one (back from
    /// another tab, where the keyboard may have learned more) reads the
    /// listed rows again, keeping the place in the list.
    func load() async {
        guard listedGeneration >= 0 else {
            await reload().value
            return
        }
        await reloadInPlace()
    }

    /// Repeats the read that failed: the next page, or the listed rows from
    /// the first (the first page when none is listed).
    func retry() async {
        switch failedRead {
        case .nextPage: await loadNextPage()
        case .list: await reloadInPlace()
        case nil: return
        }
    }

    @discardableResult
    func selectOrder(_ order: Taigi_Engine_LearningRecordOrder) -> Task<Void, Never> {
        self.order = order
        return reload()
    }

    /// The search box changed: answers for the old text are stale at once,
    /// and the reload waits for the box to settle.
    @discardableResult
    func filterChanged(_ filter: String) -> Task<Void, Never> {
        self.filter = filter
        generation += 1
        settleTask?.cancel()
        let settle = filterSettle
        let task = Task {
            try? await Task.sleep(for: settle)
            guard !Task.isCancelled else { return }
            await reload().value
        }
        settleTask = task
        return task
    }

    /// The filter matches rows not listed yet.
    var hasMoreRows: Bool {
        records.count < matchingTotal
    }

    /// What the list-end sentinel's `.task(id:)` keys on: it changes after
    /// every load that lands, so the next page is asked again even when the
    /// last row kept its id.
    var pagingKey: [Int] {
        [listedGeneration, records.count]
    }

    /// Asks for the next page while the filter matches more and the rows
    /// listed answer what is asked now.
    func loadNextPage() async {
        guard hasMoreRows,
              listedGeneration == generation,
              !isLoadingNextPage
        else { return }
        isLoadingNextPage = true
        let generation = generation
        let offset = UInt32(clamping: records.count)
        do {
            let page = try await userData.listLearningRecords(
                kind: kind,
                order: order,
                filter: filter,
                limit: Self.pageSize,
                offset: offset,
            )
            guard generation == self.generation else { return }
            isLoadingNextPage = false
            // The matches shrank under the list (the engine pulled the
            // offset back): the rows listed no longer line up with the store.
            guard page.offset == offset else {
                await reloadInPlace()
                return
            }
            // A row the keyboard moved while the page was read is not listed twice.
            let listed = Set(records.map(\.id))
            records += page.records.filter { !listed.contains($0.id) }
            matchingTotal = Int(page.matchingTotal)
            failedRead = nil
        } catch {
            guard generation == self.generation else { return }
            isLoadingNextPage = false
            failedRead = .nextPage
            notice = .readFailed(detail: error.localizedDescription)
        }
    }

    func setCount(_ record: Taigi_Engine_LearningRecord, to count: Int64) async {
        await write { try await $0.setLearningRecordCount(record, count: count) ? nil : .gone }
    }

    func delete(_ record: Taigi_Engine_LearningRecord) async {
        await write { try await $0.deleteLearningRecord(record) ? nil : .gone }
    }

    /// Files the learned phrase as a custom word and forgets it; the reload
    /// drops its row. A refusal keeps the phrase and says why.
    func moveToCustomDictionary(_ record: Taigi_Engine_LearningRecord) async {
        await write {
            try await $0.moveLearningRecordToCustomDictionary(record)
            return .moved
        }
    }

    /// The count the edit alert's field holds: a whole number in
    /// `1 ... maxCount`; anything else keeps Save disabled.
    static func count(from text: String) -> Int64? {
        guard let count = Int64(text.trimmingCharacters(in: .whitespaces)), (1 ... maxCount).contains(count) else {
            return nil
        }
        return count
    }

    // MARK: - Loading

    /// Every write reloads: the row moved, went, or was never there. `change`
    /// answers the notice its outcome earns (a row already gone is said, not
    /// reported as a failure), or `nil` for none.
    private func write(_ change: (any UserDataClient) async throws -> LearningRecordsNotice?) async {
        do {
            if let outcome = try await change(userData) {
                notice = outcome
            }
        } catch {
            notice = .writeFailed(detail: error.localizedDescription)
        }
        await reloadInPlace()
    }

    /// Reloads as many rows as are listed, so the list keeps its place.
    private func reloadInPlace() async {
        await reload(rowCount: max(Int(Self.pageSize), records.count)).value
    }

    /// Lists `rowCount` rows from the top for what is asked now, in engine
    /// pages of at most `pageSize`, and shows them in one go; every answer
    /// still in flight is stale from here.
    @discardableResult
    private func reload(rowCount: Int = Int(pageSize)) -> Task<Void, Never> {
        generation += 1
        let generation = generation
        // An order change, a write or a retry overtakes a filter still settling.
        settleTask?.cancel()
        isLoadingNextPage = false
        let (order, filter) = (order, filter)
        return Task {
            do {
                var rows: [Taigi_Engine_LearningRecord] = []
                var matching = 0
                while true {
                    let offset = rows.count
                    let page = try await userData.listLearningRecords(
                        kind: kind,
                        order: order,
                        filter: filter,
                        limit: UInt32(clamping: min(Int(Self.pageSize), rowCount - offset)),
                        offset: UInt32(clamping: offset),
                    )
                    guard generation == self.generation else { return }
                    matching = Int(page.matchingTotal)
                    // Served from an earlier offset (the matches shrank under
                    // the re-read): it is the last page there is, and it
                    // overrides the rows read from that offset on.
                    let served = Int(page.offset)
                    if served < offset {
                        rows.removeSubrange(served...)
                    }
                    rows += page.records
                    if served != offset || page.records.isEmpty || rows.count >= min(rowCount, matching) {
                        break
                    }
                }
                // A row the keyboard moved between pages is not listed twice.
                var listed = Set<Int64>()
                records = rows.filter { listed.insert($0.id).inserted }
                matchingTotal = matching
                listedGeneration = generation
                failedRead = nil
            } catch {
                guard generation == self.generation else { return }
                failedRead = .list
                // A write's own notice, still up, says more than its reload's.
                if notice == nil {
                    notice = .readFailed(detail: error.localizedDescription)
                }
            }
            isLoading = false
        }
    }
}

/// A read `LearningRecordsViewModel.retry()` repeats.
enum LearningRecordsRead {
    /// The page after the listed rows.
    case nextPage
    /// The listed rows, from the first.
    case list
}

/// What the page tells the user after a load or a write; the failures
/// carry the engine's own detail.
enum LearningRecordsNotice: Equatable {
    case readFailed(detail: String)
    case writeFailed(detail: String)
    /// An edit or delete found the row deleted, evicted, or its id taken.
    case gone
    /// A learned phrase was filed in the custom dictionary and forgotten here.
    case moved
}

extension Taigi_Engine_LearningRecord {
    /// The day the row was last used, in this device's calendar and
    /// locale; empty when the store held no readable time.
    var lastUsedLabel: String {
        guard lastUsedMs > 0 else { return "" }
        return Date(timeIntervalSince1970: TimeInterval(lastUsedMs) / 1000)
            .formatted(date: .numeric, time: .omitted)
    }
}
