// What the two candidate operations answer: a query's three kinds of nothing,
// and what a commit actually did to the document.

import Foundation

/// The answer to a candidate query.
///
/// Three cases rather than an optional array because the caller acts on each
/// differently, and collapsing any two of them shows the user a stale list. A
/// failed round-trip left the engine exactly as it was, so the candidates on
/// screen may still be describing an older buffer; an engine that answers "not
/// composing" is authoritative that there is nothing to show.
enum CandidateFetchOutcome: Equatable {
    /// The round-trip never reached the engine.
    case unavailable
    /// The engine answered, and it is not in the continuous phase.
    case notComposing
    /// The engine answered. An empty array means it found nothing for a
    /// composition it IS holding, which is a different thing from `notComposing`.
    case found([ContinuousCandidate])
}

/// What committing a candidate did, as the engine reported it
/// (`ComposingResponse.commit`).
///
/// Never read off the composing mirror: a generation mismatch silently resets
/// the engine to Idle before the intent runs (`engine/composing/src/handle.rs`),
/// which turns the commit into a noop while flipping `isComposing` to false —
/// so a mirror read reports "the composition ended" for a commit that never
/// happened.
enum CandidateCommitOutcome: Equatable {
    /// The round-trip never reached the engine.
    case unavailable
    /// The engine rejected the commit — a stale byte offset, a composition
    /// that had already gone, or a script the candidate does not have (Space
    /// on a one-script cell). Nothing changed.
    case ignored
    /// The segment was nailed and the composition continues. Under Model B this
    /// writes nothing to the document; the marked region is re-rendered with the
    /// nailed prefix in front of the remaining tail.
    case nailed
    /// The whole composition was consumed, written to the document in one
    /// mutation, and the engine returned to Idle. `earnsAutoSpace` is the
    /// engine's §23 verdict on what the pick wrote (romanization, no trailing
    /// `-`); the live Auto-Space setting is still the caller's.
    case finalized(earnsAutoSpace: Bool)

    /// CROSS-PLATFORM INVARIANT — mirrors
    /// `desktop/crates/taigi-desktop-core/src/composing/outcomes.rs`
    /// `CandidateCommitOutcome::from_resolution`.
    init(_ resolution: Taigi_Engine_CommitResolution) {
        switch resolution.outcome {
        case .nailed: self = .nailed
        case .finalized: self = .finalized(earnsAutoSpace: resolution.earnsAutoSpace)
        // UNSPECIFIED: an answer without a resolution changed nothing this
        // side can tell.
        case .ignored, .unspecified, .UNRECOGNIZED: self = .ignored
        }
    }
}
