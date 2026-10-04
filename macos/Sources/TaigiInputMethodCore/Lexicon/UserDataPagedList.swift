// What a paged user-data page keeps between loads, and the queue its engine
// requests run on. Shared by Custom Dictionary and Learning Records.

import Foundation

/// The page of a user-data list on screen: its rows, the two counts, which
/// page it is, and which load put it there.
///
/// Rows are fetched one PAGE at a time and replaced on every change rather
/// than held: the settings window outlives every visit to a page, and a
/// 30000-row store kept in a view model would stay in memory for the life of
/// the input method.
///
/// Paged since 2026-08-26. It was a flat `LIMIT 100`, which had two faults a
/// 17000-entry dictionary showed at once (USER, real device): rows 101 and past
/// were unreachable by any means but the filter, and a hundred rows in a
/// fixed-height `Table` inside a `Form` put one scroll view inside another —
/// the list would not scroll. A page that FITS the table has neither problem:
/// nothing is nested to scroll, and every row is reachable by paging
/// (`UserDataListMetrics.pageSize`).
struct UserDataPagedList<Row: Identifiable & Sendable> {
    /// One load of the page on screen: where it reads from, and the number
    /// that tells it apart from every load started after it.
    struct Load {
        fileprivate let generation: Int
        let offset: Int
    }

    private(set) var rows: [Row] = []
    /// Every row of the store, for the section header — what the store
    /// HOLDS, which is not what the current filter matches.
    private(set) var totalCount = 0
    /// How many rows the current filter matches; what the pager divides.
    private(set) var matchCount = 0
    /// Which page is on screen, zero-based.
    private(set) var page = 0
    /// The row the table has selected — the one `−` acts on. Only ever a
    /// row on screen: a load that lands without it drops it, as
    /// desktop-core's `Listing::land` does, so paging back or re-filtering
    /// never brings back a selection the user moved away from
    /// (`INVARIANT_USER_DATA_LIST_FILTER_RELOAD_SELECTION`, §58).
    var selectedID: Row.ID?

    /// Which load the rows on screen came from. A query runs off the main
    /// actor and cannot be cancelled once it is in the engine, so a load
    /// started under an older filter can still come back after a newer one
    /// has — and would put rows on screen that do not match what is in the
    /// box. The newest load wins by number, not by arrival.
    private var loadGeneration = 0

    /// How many pages the matches fill — at least one, so an empty store
    /// still reads as "1 / 1" rather than as a pager with nothing in it.
    var pageCount: Int {
        max(1, (matchCount + UserDataListMetrics.pageSize - 1) / UserDataListMetrics.pageSize)
    }

    /// What the store holds — and, while a filter narrows it, how much of
    /// that the filter matches.
    ///
    /// Against the MATCHES, not against the rows on screen: those are one
    /// page, so that comparison would read "10 / 17000" on every page of an
    /// unfiltered list and say nothing about either number.
    var countLabel: String {
        matchCount < totalCount ? "\(matchCount) / \(totalCount)" : "\(totalCount)"
    }

    /// Moves `delta` pages, or answers false because there is no such page.
    mutating func step(by delta: Int) -> Bool {
        let target = page + delta
        guard (0 ..< pageCount).contains(target) else { return false }
        page = target
        return true
    }

    /// Back to page one. What a new filter, kind or order asks for: the
    /// pages the list had before are pages of a different list.
    mutating func rewind() {
        page = 0
    }

    /// Makes every load already started stale, without starting one. What a
    /// change of filter, kind or order does the moment it happens: the load
    /// it asks for waits for the filter to settle, and a load of the old list
    /// that returns in that gap must neither land its rows nor raise its
    /// failure.
    mutating func invalidate() {
        loadGeneration += 1
    }

    /// Starts a load of the page on screen; every load started before it is
    /// stale from here on.
    mutating func beginLoad() -> Load {
        loadGeneration += 1
        return Load(generation: loadGeneration, offset: page * UserDataListMetrics.pageSize)
    }

    /// Whether `load` is still the newest — a failure from one the user has
    /// already moved past must not raise an alert over the list that
    /// replaced it.
    func isCurrent(_ load: Load) -> Bool {
        load.generation == loadGeneration
    }

    /// Puts `listing` on screen unless a newer load has started. The engine
    /// pulls a page the list shrank under — a delete on the last page, or a
    /// filter that now matches less — back to the last one that exists, and
    /// says which page it answered.
    mutating func land(_ listing: UserDataListing<Row>, from load: Load) {
        guard isCurrent(load) else { return }
        page = listing.offset / UserDataListMetrics.pageSize
        matchCount = listing.matchingTotal
        rows = listing.rows
        totalCount = listing.total
        if selectedRow == nil {
            selectedID = nil
        }
    }

    /// The selected row, or nil when nothing on screen is selected.
    var selectedRow: Row? {
        rows.first { $0.id == selectedID }
    }
}

/// The user-data pages' engine requests, one at a time and off the main
/// actor. A queue of its own rather than a detached task per request: a
/// request can wait on SQLite — or, right after launch, on the engine
/// finishing the takeover of the old files — and that wait must not hold a
/// thread of the shared pool the rest of the process runs its tasks on.
enum UserDataRequests {
    private static let queue = DispatchQueue(label: "UserDataRequests")

    /// Runs one request on `queue`: each is a synchronous round-trip, and
    /// the settings window must not freeze while it runs.
    static func run<Value: Sendable>(
        on client: any UserDataClient,
        _ request: @escaping @Sendable (any UserDataClient) throws -> Value,
    ) async throws -> Value {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                continuation.resume(with: Result { try request(client) })
            }
        }
    }
}
