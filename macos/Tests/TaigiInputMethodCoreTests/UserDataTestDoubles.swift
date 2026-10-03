// In-memory stand-ins for the engine's user data.
//
// This test process never opens the engine's user data: the handle is
// process-wide, so an open here would reach every later fetch in the run.
// What the engine does with a pick, a bigram or a page request is asserted
// by the engine's own tests (`engine/userdata/src/requests.rs`,
// `engine/dispatch/src/user_data.rs`, `engine/dispatch/tests/user_data_*.rs`);
// these record what this side SENDS.

import Foundation
@testable import TaigiInputMethodCore

/// A custom dictionary held in memory, answering the pages' requests the way
/// the engine does: newest edit first, a case-insensitive substring filter.
final class FakeUserDataClient: UserDataClient, @unchecked Sendable {
    private let lock = NSLock()
    private var rows: [CustomDictionaryRow] = []

    func list(filter: String, limit: Int, offset: Int) throws -> CustomDictionaryListing {
        lock.withLock {
            let matching = filter.isEmpty
                ? rows
                : rows.filter {
                    $0.roman.localizedCaseInsensitiveContains(filter) || $0.hanji.contains(filter)
                }
            // Pulled back to the last page that exists, as the engine does.
            let lastPage = max(0, matching.count - 1) / max(1, limit) * limit
            let served = min(offset, lastPage)
            return CustomDictionaryListing(
                rows: Array(matching.dropFirst(served).prefix(limit)),
                total: rows.count,
                matchingTotal: matching.count,
                offset: served,
            )
        }
    }

    func save(_ row: CustomDictionaryRow) throws {
        lock.withLock {
            rows.removeAll { $0.id == row.id }
            rows.insert(row, at: 0)
        }
    }

    func delete(id: String) throws {
        lock.withLock { rows.removeAll { $0.id == id } }
    }

    func deleteAll() throws {
        lock.withLock { rows.removeAll() }
    }

    func exportCSV() throws -> Data {
        Data()
    }

    func importCSV(at _: URL) throws -> CustomDictionaryImportResult {
        CustomDictionaryImportResult(imported: 0, skipped: 0)
    }

    func clearLearningRecords() throws {}
}
