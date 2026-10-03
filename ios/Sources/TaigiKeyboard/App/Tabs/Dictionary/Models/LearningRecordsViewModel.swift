import Foundation

/// ViewModel for `LearningRecordsView`: what the keyboard learned, one kind
/// at a time, in pages the engine cuts (`docs/architecture/learning-records-page-roadmap.md`).
///
/// The kind, the order and the filter go to the engine; the rows on screen
/// are its answer. A change of any of the three starts a load from the top
/// and makes every answer still in flight stale — the newest request wins,
/// as on the desktop's `Listing`. Form state (the edit alert's field) stays
/// in the view.
@MainActor
final class LearningRecordsViewModel: ObservableObject {
    /// Rows per engine page; the next page loads when the last row shows.
    static let pageSize: UInt32 = 100
    /// The filter reloads once typing pauses this long — the desktop's
    /// `FILTER_SETTLE`, so a word typed letter by letter is one query.
    static let filterSettle: Duration = .milliseconds(200)

    @Published private(set) var kind: Taigi_Engine_LearningRecordKind = .frequency
    @Published private(set) var order: Taigi_Engine_LearningRecordOrder = .mostUsed
    @Published private(set) var records: [Taigi_Engine_LearningRecord] = []
    /// Only until the first answer: later loads leave the rows on screen.
    @Published private(set) var isLoading = true
    /// The latest load of what is asked now failed. With no rows listed the
    /// page says "could not read", never "nothing learned yet".
    @Published private(set) var lastLoadFailed = false
    @Published var notice: LearningRecordsNotice?

    private var filter = ""
    /// The rows the current filter matches — what paging runs up to.
    private var matchingTotal = 0
    /// Bumped by every change of what is asked; an answer started under an
    /// older number is dropped.
    private var generation = 0
    /// The generation the rows on screen answer. The next page is asked
    /// only while it is the current one: rows of an older kind, order or
    /// filter must not get the new query's next page appended.
    private var listedGeneration = -1
    private var isLoadingNextPage = false
    private var settleTask: Task<Void, Never>?

    private let userData: any UserDataClient
    private let filterSettle: Duration

    init(
        userData: any UserDataClient = CompositionRoot.userData,
        filterSettle: Duration = LearningRecordsViewModel.filterSettle,
    ) {
        self.userData = userData
        self.filterSettle = filterSettle
    }

    func load() async {
        await reload().value
    }

    @discardableResult
    func selectKind(_ kind: Taigi_Engine_LearningRecordKind) -> Task<Void, Never> {
        self.kind = kind
        return reload()
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

    /// Asks for the next page when `record` is the last row listed and the
    /// filter matches more.
    func loadNextPageIfNeeded(after record: Taigi_Engine_LearningRecord) async {
        guard record == records.last,
              records.count < matchingTotal,
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
        } catch {
            guard generation == self.generation else { return }
            isLoadingNextPage = false
            notice = .readFailed(detail: error.localizedDescription)
        }
    }

    func setCount(_ record: Taigi_Engine_LearningRecord, to count: Int64) async {
        await write { try await $0.setLearningRecordCount(record, count: count) }
    }

    func delete(_ record: Taigi_Engine_LearningRecord) async {
        await write { try await $0.deleteLearningRecord(record) }
    }

    /// The count the edit alert's field holds: a whole number, at least 1
    /// (the engine clamps the top).
    static func count(from text: String) -> Int64? {
        guard let count = Int64(text.trimmingCharacters(in: .whitespaces)), count >= 1 else {
            return nil
        }
        return count
    }

    // MARK: - Loading

    /// Every write reloads: the row moved, went, or was never there. A row
    /// already gone is said, not reported as a failure.
    private func write(_ change: (any UserDataClient) async throws -> Bool) async {
        do {
            if try await !change(userData) {
                notice = .gone
            }
        } catch {
            notice = .writeFailed(detail: error.localizedDescription)
        }
        await reloadInPlace()
    }

    /// Reloads as many rows as are listed, so the list keeps its place.
    private func reloadInPlace() async {
        await reload(limit: max(Self.pageSize, UInt32(clamping: records.count))).value
    }

    /// Lists `limit` rows from the top for what is asked now; every answer
    /// still in flight is stale from here.
    @discardableResult
    private func reload(limit: UInt32 = pageSize) -> Task<Void, Never> {
        generation += 1
        let generation = generation
        isLoadingNextPage = false
        let (kind, order, filter) = (kind, order, filter)
        return Task {
            do {
                let page = try await userData.listLearningRecords(
                    kind: kind,
                    order: order,
                    filter: filter,
                    limit: limit,
                    offset: 0,
                )
                guard generation == self.generation else { return }
                records = page.records
                matchingTotal = Int(page.matchingTotal)
                listedGeneration = generation
                lastLoadFailed = false
            } catch {
                guard generation == self.generation else { return }
                lastLoadFailed = true
                // A write's own notice, still up, says more than its reload's.
                if notice == nil {
                    notice = .readFailed(detail: error.localizedDescription)
                }
            }
            isLoading = false
        }
    }
}

/// What the page tells the user after a load or a write; the failures
/// carry the engine's own detail.
enum LearningRecordsNotice: Equatable {
    case readFailed(detail: String)
    case writeFailed(detail: String)
    /// An edit or delete found the row deleted, evicted, or its id taken.
    case gone
}

extension Taigi_Engine_LearningRecord {
    /// The word; an association reads `previous → next`.
    var wordLabel: String {
        kind == .association ? "\(previousText) → \(text)" : text
    }

    /// The TL reading, paired the same way; empty when none was stored.
    var readingLabel: String {
        guard kind == .association, !(previousTl.isEmpty && tl.isEmpty) else { return tl }
        return "\(previousTl) → \(tl)"
    }

    /// The day the row was last used, in this device's calendar and
    /// locale; empty when the store held no readable time.
    var lastUsedLabel: String {
        guard lastUsedMs > 0 else { return "" }
        return Date(timeIntervalSince1970: TimeInterval(lastUsedMs) / 1000)
            .formatted(date: .numeric, time: .omitted)
    }
}
