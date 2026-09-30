// Where a pick is counted — the engine keeps the count
// (docs/architecture/user-data-engine-roadmap.md P7b).

import Foundation

/// One NextWord prediction the user took. This side decides WHAT the pick is
/// — the `(displayText, canonicalTl)` identity it counts under — and the
/// engine keeps the count. A Continuous pick is counted by the engine itself.
struct Usage {
    let displayText: String
    let canonicalTl: String
}

/// CROSS-PLATFORM INVARIANT — mirrors Android `UsageRecorder.kt`; every
/// Continuous pick the engine records itself (R5: `engine/composing/src/transition.rs`
/// `commit_continuous_resolved`, `Applied.usage`).
protocol UsageRecorder: AnyObject {
    func record(_ usage: Usage)
}

/// The engine's own stores (`UserDataRequest.record_usage`) — when this
/// process opened them; otherwise (a keyboard without Full Access, the test
/// process) a pick has nowhere to go.
final class EngineUsageRecorder: UsageRecorder {
    func record(_ usage: Usage) {
        guard UserDataOpening.isOpen else { return }
        RustEngineBridge.userDataRecordUsage(displayText: usage.displayText, canonicalTl: usage.canonicalTl)
    }
}
