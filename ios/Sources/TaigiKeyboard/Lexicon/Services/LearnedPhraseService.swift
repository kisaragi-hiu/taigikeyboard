import Foundation

/// Learned-phrase service (§50) — a thin wrapper over `LearnedPhraseRepository`
/// that owns the swallow-and-log boundary, like `UserFrequencyService` does
/// for `user_frequency.db`: learning is a side effect of a keystroke that has
/// already been rendered, so a failed write is logged, never surfaced.
final class LearnedPhraseService: @unchecked Sendable {
    // MARK: - Properties

    private let repository: LearnedPhraseRepository
    private let logger = DebugLogger(category: "LearnedPhraseService")

    // MARK: - Initialization

    init(repository: LearnedPhraseRepository = CompositionRoot.learnedPhraseRepository) {
        self.repository = repository
    }

    // MARK: - Lifecycle

    /// Open the DB and apply the schema before the first keystroke asks.
    func ensureInitialized() async throws {
        try await repository.ensureInitialized()
    }

    // MARK: - Learning

    /// The engine's `Effect.PhraseLearned` — the pair the user just composed
    /// segment by segment.
    func recordLearnedPhrase(hanzi: String, canonicalTl: String) {
        fireAndForget("learnPhrase") { [repository] in
            try await repository.learnPhrase(hanzi: hanzi, canonicalTl: canonicalTl)
        }
    }

    /// A learned phrase the user just picked as one candidate — keeps it
    /// ahead of the eviction line (no-op for any other candidate).
    func recordLearnedPick(hanzi: String, canonicalTl: String) {
        fireAndForget("touchPhrase") { [repository] in
            try await repository.touchPhrase(hanzi: hanzi, canonicalTl: canonicalTl)
        }
    }

    /// The learning-data wipe (settings reset): every phrase and its keys.
    func deleteAll() {
        fireAndForget("deleteAll") { [repository] in
            try await repository.deleteAll()
        }
    }

    // MARK: - Private

    private func fireAndForget(_ op: StaticString, _ body: @escaping @Sendable () async throws -> Void) {
        Task { [logger] in
            do {
                try await body()
            } catch {
                logger.error("[LEARN] \(op) failed: \(error)")
            }
        }
    }
}
