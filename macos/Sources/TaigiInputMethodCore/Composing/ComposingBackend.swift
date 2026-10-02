// The seam between the controller and whatever runs its key path.

import Foundation

/// What runs one session's key path: classifies a key, drives the engine,
/// and answers with what the controller has to do to its client and its
/// candidate window — in order, as a list it replays after the call returns.
///
/// The controller keeps everything that is IMKit or AppKit: the client calls
/// themselves, the windows, the caret rectangle, the auto-space arm and the
/// swap's client check, the symbol picker, the Telex guide, the global
/// actions. A back end never touches a client; it records, and the
/// controller replays (`docs/architecture/macos-desktop-core-roadmap.md` D3,
/// record-then-replay). Two back ends implement it: `LegacyComposingBackend`
/// (the Swift key path) and `CoreComposingBackend` (desktop-core) — one back
/// end per process (`ComposingBackends`): both drive the one process-wide
/// engine.
///
/// Every request carries the session's token; one whose token does not own
/// the engine is answered `nil` and does nothing — the non-owner rule of
/// `ComposingSessionCoordinator.owns`.
@MainActor
protocol ComposingBackend: AnyObject {
    /// Whether `session` drives the engine now.
    func owns(_ session: ComposingSessionToken) -> Bool

    /// Whether `session` owns the engine and a composition is running.
    func isComposing(_ session: ComposingSessionToken) -> Bool

    /// `session` takes the engine (`activateServer`); a session that already
    /// holds it keeps its composition. Any list held for a session is dropped:
    /// the window went down with the handover.
    ///
    /// Claims `ComposingSessionCoordinator` too: the shortcut-target registry
    /// the controller registers with next accepts only its owner.
    func activate(_ session: ComposingSessionToken)

    /// `session` gives the engine up, if it holds it.
    func release(_ session: ComposingSessionToken)

    /// One key. `bindings` is the chord table the controller resolved once for
    /// this key, so the legacy back end and the controller's own key reads
    /// agree; a back end that classifies from the settings snapshot does not
    /// need it.
    func key(
        _ key: KeyEventSnapshot,
        bindings: ComposingKeyBindings,
        in request: ComposingRequest,
    ) -> ComposingKeyReply?

    /// The lifecycle commit (`commitComposition`, deactivation, close): the
    /// composition as typed, with no auto space — the user did not finish a
    /// word there — and no list effect: the controller took the window down
    /// first.
    func commitComposition(in request: ComposingRequest) -> [ComposingBackendEffect]?

    /// The commit the symbol-picker chord runs before the picker opens: the
    /// highlighted cell while the window shows one, the composition as typed
    /// (with its auto space) otherwise.
    func commitForSymbolPicker(in request: ComposingRequest) -> [ComposingBackendEffect]?

    /// A symbol the picker wrote: at the caret as one string, or swapped with
    /// the auto space a commit left.
    func insertSymbol(_ symbol: String, in request: ComposingRequest) -> [ComposingBackendEffect]?

    /// The list on screen again under the settings in force now, after a
    /// switch outside the key path changed them: fetched again (`refetch`,
    /// Candidate Display) or the same list re-rendered (the Hanji/romanization
    /// swap). No list on screen, nothing to do.
    func represent(refetch: Bool, in request: ComposingRequest) -> CandidateListRepresentation?
}

/// The session a request is for and what only the controller can answer.
@MainActor
struct ComposingRequest {
    let session: ComposingSessionToken

    /// The controller's own store — the one a test points at its own suite —
    /// read for every setting the key path consults.
    let settings: SettingsStore

    let panel: ComposingPanelState
}

/// The controller's side of a request: its window and its client.
///
/// The list on screen is the window's truth: a request that says none is up
/// drops whatever list the back end still holds before anything reads it (a
/// list the client gave no caret rectangle for, a dismissal outside the key
/// path). The three questions are closures so a back end asks them exactly
/// where today's key path does; one that has to send them over a boundary
/// asks them all before it sends.
@MainActor
struct ComposingPanelState {
    let isListOnScreen: Bool

    /// The window's highlighted cell, if it shows one.
    let selectedIndex: @MainActor () -> Int?

    /// The cell the `slot`-th selection key addresses on the page shown.
    let indexForKeySlot: @MainActor (Int) -> Int?

    /// Whether the auto space the last commit left can be swapped now: the
    /// caret still collapsed where the space left it, and the character
    /// before it still that space. `nil` while nothing is armed.
    let canSwapPrecedingSpace: (@MainActor () -> Bool)?
}

/// A key's answer: whether it was consumed, and what to replay.
struct ComposingKeyReply {
    let handled: Bool
    let effects: [ComposingBackendEffect]
}

/// One thing the controller does to its client or its window, in reply order.
enum ComposingBackendEffect: Equatable {
    /// The composition, underlined, with the caret at `caretUTF16`.
    case setMarkedText(String, caretUTF16: Int)

    /// The composition ended without writing anything.
    case clearMarkedText

    /// Written at the insertion point, over the marked region if there is one.
    case insertText(String)

    /// The armed auto space replaced by `replacement` (`guá ` + `?` → `guá? `).
    case swapPrecedingSpace(replacement: String)

    /// Arm the swap at the caret the writes before it left.
    case armSwap

    /// A list to show, anchored at the caret.
    case candidatesChanged(CandidateListUpdate)

    case candidatesClosed

    case navigate(CandidateNavigation)
}

/// What a represent did to the list on screen — repainted in place, never
/// anchored again.
enum CandidateListRepresentation: Equatable {
    case unchanged
    case changed(CandidateListUpdate)
    case closed
}

/// A list as the window takes it, with what anchors it.
struct CandidateListUpdate: Equatable {
    let cells: [CandidateCellContent]

    /// The first cell is the §34 literal, which takes no slot key.
    let leadsWithLiteralRoman: Bool

    /// The composition's length on screen — where the caret walk starts.
    let markedTextLengthUTF16: Int
}

/// The back end this process runs, chosen once from the environment:
/// `TAIGI_COMPOSING_BACKEND` unset or `legacy` is the Swift key path, `core`
/// is desktop-core. A test run picks one per `swift test` process (D9.1); an
/// unknown name stops the process rather than silently testing the wrong
/// back end.
enum ComposingBackends {
    static let environmentKey = "TAIGI_COMPOSING_BACKEND"

    /// Whether this process runs the core back end — for a test whose
    /// expectation differs per back end on an open parity item.
    static let isCore = ProcessInfo.processInfo.environment[environmentKey] == "core"

    @MainActor
    static let shared: any ComposingBackend = make(named: ProcessInfo.processInfo.environment[environmentKey])

    @MainActor
    private static func make(named name: String?) -> any ComposingBackend {
        switch name {
        case nil, "legacy":
            // The one place the shipped Swift composition is assembled, which
            // is why the settings store and the learner's sink are named here
            // rather than defaulted into `ComposingManager`.
            LegacyComposingBackend(
                coordinator: .shared,
                manager: ComposingManager(settingsProvider: SettingsStore(), nextWord: EngineNextWord()),
            )
        case "core":
            CoreComposingBackend(coordinator: .shared)
        case let .some(name):
            preconditionFailure("\(environmentKey)=\(name) names no composing back end (known: legacy, core)")
        }
    }
}
