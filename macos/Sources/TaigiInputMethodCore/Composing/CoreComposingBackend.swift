// The key path in desktop-core, behind `ComposingBackend`.

import Foundation

/// Runs every request through the desktop-core session over the shell seam
/// (`desktop_request_bytes`; docs/architecture/macos-desktop-core-roadmap.md
/// D3) and answers with the effects the core recorded, for the controller to
/// replay. Selected with `TAIGI_COMPOSING_BACKEND=core` until the cut-over
/// (roadmap P12).
///
/// What the core cannot know is asked here before a request is encoded: the
/// window's selection and slots, and the auto-space swap's client check.
/// The settings travel inside each request, read from the request's own
/// store, so the core classifies under exactly what the controller reads.
@MainActor
final class CoreComposingBackend: ComposingBackend {
    /// One encoded request in, one encoded response out.
    typealias Transport = ([UInt8]) -> [UInt8]

    private static let logger = DebugLogger(category: "CoreComposingBackend")

    /// Who owns the engine, and the shortcut-target registry the controller
    /// registers with right after `activate` — which accepts only this
    /// owner (roadmap D3, "What stays in the controller"). The core keeps
    /// the same record from the same `Activate` and `Release`.
    private let coordinator: ComposingSessionCoordinator

    /// The process's runtime, asked per request: its settings whitelist is
    /// what a snapshot is built from.
    private let runtime: () -> DesktopCoreRuntime?

    /// `DesktopCoreBridge.send`; a test substitutes it to answer what the
    /// core cannot be made to.
    private let transport: Transport

    /// The owner's composition as the last reply described it.
    private var isOwnerComposing = false

    init(
        coordinator: ComposingSessionCoordinator,
        runtime: @escaping () -> DesktopCoreRuntime? = { DesktopCoreRuntime.configured },
        transport: @escaping Transport = DesktopCoreBridge.send,
    ) {
        self.coordinator = coordinator
        self.runtime = runtime
        self.transport = transport
    }

    func owns(_ session: ComposingSessionToken) -> Bool {
        coordinator.owns(session)
    }

    func isComposing(_ session: ComposingSessionToken) -> Bool {
        owns(session) && isOwnerComposing
    }

    /// An `Activate` that takes the engine from another session drops that
    /// session's composition and sends no `ClearMarkedText`: the region is
    /// the outgoing controller's, cleared on its own way out (`endSession`).
    func activate(_ session: ComposingSessionToken) {
        coordinator.claim(session)
        var message = Taigi_DesktopShell_ActivateRequest()
        message.token = session.value
        // A session that already held the engine keeps its composition.
        if case let .reply(_, _, isComposing) = exchange(.activate(message), settings: nil, op: "activate") {
            isOwnerComposing = isComposing
        } else {
            isOwnerComposing = false
        }
    }

    func release(_ session: ComposingSessionToken) {
        if coordinator.release(session) {
            isOwnerComposing = false
        }
        var message = Taigi_DesktopShell_ReleaseRequest()
        message.token = session.value
        _ = exchange(.release(message), settings: nil, op: "release")
    }

    func key(
        _ key: KeyEventSnapshot,
        bindings _: ComposingKeyBindings,
        in request: ComposingRequest,
    ) -> ComposingKeyReply? {
        var message = Taigi_DesktopShell_KeyRequest()
        message.token = request.session.value
        message.event = Self.event(key)
        // A swap is a pass-through key's, and only an idle session passes an
        // attaching character through: composing, it commits with it.
        let attaching = isComposing(request.session) ? nil : Self.swapCandidate(for: key, settings: request.settings)
        message.panel = Self.panel(request, swapping: attaching)
        return session(.key(message), in: request, op: "key").map {
            ComposingKeyReply(handled: $0.handled, effects: $0.effects)
        }
    }

    func commitComposition(in request: ComposingRequest) -> [ComposingBackendEffect]? {
        var message = Taigi_DesktopShell_CommitCompositionRequest()
        message.token = request.session.value
        message.panel = Self.panel(request, swapping: nil)
        return session(.commitComposition(message), in: request, op: "commitComposition")?.effects
    }

    func commitForSymbolPicker(in request: ComposingRequest) -> [ComposingBackendEffect]? {
        var message = Taigi_DesktopShell_CommitForSymbolPickerRequest()
        message.token = request.session.value
        message.panel = Self.panel(request, swapping: nil)
        return session(.commitForSymbolPicker(message), in: request, op: "commitForSymbolPicker")?.effects
    }

    func insertSymbol(_ symbol: String, in request: ComposingRequest) -> [ComposingBackendEffect]? {
        var message = Taigi_DesktopShell_InsertSymbolRequest()
        message.token = request.session.value
        message.symbol = symbol
        message.panel = Self.panel(request, swapping: symbol)
        return session(.insertSymbol(message), in: request, op: "insertSymbol")?.effects
    }

    /// After `FAIL_INTERNAL` the list goes and nothing else: a represent
    /// has no client to clear marked text from, and a refetch never changes
    /// the composition, so the core's is left alone.
    func represent(refetch: Bool, in request: ComposingRequest) -> CandidateListRepresentation? {
        var message = Taigi_DesktopShell_RepresentRequest()
        message.token = request.session.value
        message.refetch = refetch
        message.panel = Self.panel(request, swapping: nil)
        switch exchange(.represent(message), settings: snapshot(for: request), op: "represent") {
        case let .reply(_, effects, isComposing):
            isOwnerComposing = isComposing
            switch effects.first {
            case nil:
                return .unchanged
            case let .candidatesChanged(list):
                return .changed(list)
            default:
                return .closed
            }
        case .failedInternally:
            return .closed
        case .nothing:
            return nil
        }
    }

    // MARK: - The seam

    /// What became of one request.
    private enum Outcome {
        /// The owner's reply, every effect translated.
        case reply(handled: Bool, effects: [ComposingBackendEffect], isComposing: Bool)
        /// Nothing ran, or the session does not own the engine: nothing to
        /// replay, and the key goes to the host.
        case nothing
        /// The engine may have moved and the reply cannot say how (D4).
        case failedInternally
    }

    /// A session request under the settings `request` carries. After
    /// `FAIL_INTERNAL` the key is consumed — handed back, the host would type
    /// it twice — and the composition and the list are dropped on both
    /// sides (roadmap D4).
    private func session(
        _ message: Taigi_DesktopShell_DesktopRequest.OneOf_Request,
        in request: ComposingRequest,
        op: String,
    ) -> (handled: Bool, effects: [ComposingBackendEffect])? {
        switch exchange(message, settings: snapshot(for: request), op: op) {
        case let .reply(handled, effects, isComposing):
            isOwnerComposing = isComposing
            return (handled, effects)
        case .nothing:
            return nil
        case .failedInternally:
            return (true, recoverFromInternalFailure(session: request.session))
        }
    }

    /// The whitelisted settings as `request`'s store holds them now; `nil`
    /// — logged — before the runtime exists, which the core would refuse
    /// anyway.
    private func snapshot(for request: ComposingRequest) -> Taigi_DesktopShell_SettingsSnapshot? {
        guard let runtime = runtime() else {
            Self.logger.error("no desktop-core runtime — the request goes without settings")
            return nil
        }
        return DesktopCoreRuntime.settingsSnapshot(runtime.settings, in: request.settings.userDefaults)
    }

    /// Classified by whether the engine may have run: a request refused
    /// before dispatch (`FAIL_PARSE`, `FAIL_INVARIANT`, an encode failure)
    /// changed nothing; a response that does not decode, an OK without a
    /// session reply, or an effect this side cannot replay may follow work
    /// the reply does not describe. Logged with the op and the error, never
    /// the request — it holds what the user typed.
    private func exchange(
        _ message: Taigi_DesktopShell_DesktopRequest.OneOf_Request,
        settings: Taigi_DesktopShell_SettingsSnapshot?,
        op: String,
    ) -> Outcome {
        guard let response = DesktopCoreBridge.response(
            to: message, settings: settings, op: op, transport: transport,
        ) else { return .nothing }
        switch response.error {
        case .ok:
            guard case let .session(reply) = response.reply else {
                Self.logger.error("[\(op)] OK without a session reply")
                return .failedInternally
            }
            if reply.ignored {
                return .nothing
            }
            let effects = reply.effects.compactMap(Self.effect)
            guard effects.count == reply.effects.count else {
                Self.logger.error("[\(op)] an effect this side cannot replay")
                return .failedInternally
            }
            return .reply(handled: reply.handled, effects: effects, isComposing: reply.isComposing)
        case .failInternal:
            Self.logger.error("[\(op)] core failed internally")
            return .failedInternally
        default:
            Self.logger.error("[\(op)] core refused: \(response.error)")
            return .nothing
        }
    }

    /// Cancels the session in the core — once, whatever it answers — and
    /// answers what the controller does to its own side: the marked text the
    /// last reply left, and the list.
    private func recoverFromInternalFailure(session: ComposingSessionToken) -> [ComposingBackendEffect] {
        let wasComposing = isOwnerComposing
        isOwnerComposing = false
        var cancel = Taigi_DesktopShell_CancelRequest()
        cancel.token = session.value
        cancel.panel = Taigi_DesktopShell_PanelState()
        _ = exchange(.cancel(cancel), settings: nil, op: "cancel")
        return (wasComposing ? [.clearMarkedText] : []) + [.candidatesClosed]
    }

    // MARK: - Requests

    /// The `NSEvent` fields the core translates (`key_translation.rs`).
    private static func event(_ key: KeyEventSnapshot) -> Taigi_DesktopShell_KeyEvent {
        assert(
            key.specialKeyRawValue != nil || !key.isNamedSpecialKey,
            "a named key with no raw value would reach the core as text",
        )
        var event = Taigi_DesktopShell_KeyEvent()
        if let keyCode = key.keyCode {
            event.keyCode = UInt32(keyCode)
        }
        if let characters = key.characters {
            event.characters = characters
        }
        if let characters = key.charactersIgnoringModifiers {
            event.charactersIgnoringModifiers = characters
        }
        event.modifierFlags = UInt64(key.modifiers.rawValue)
        if let specialKey = key.specialKeyRawValue {
            event.specialKey = specialKey
        }
        return event
    }

    /// The window's state, every question asked now, before the request
    /// crosses. The highlight is asked whether or not a list is up, as the
    /// legacy picker commit asks it; the slots only while one is. The swap's
    /// client check — two client queries — is asked only for `attaching`,
    /// the text a swap would write, and only when a swap can happen: an arm,
    /// an attaching character, Auto-Space on — the legacy order
    /// (`LegacyComposingBackend.swapAutoSpace`).
    private static func panel(
        _ request: ComposingRequest,
        swapping attaching: String?,
    ) -> Taigi_DesktopShell_PanelState {
        let state = request.panel
        var panel = Taigi_DesktopShell_PanelState()
        panel.isListOnScreen = state.isListOnScreen
        if let selected = state.selectedIndex() {
            panel.selectedIndex = UInt32(selected)
        }
        if state.isListOnScreen {
            for slot in 0 ..< HorizontalPageLayout.pageSize {
                if let index = state.indexForKeySlot(slot) {
                    panel.slotIndices[UInt32(slot)] = UInt32(index)
                }
            }
        }
        if let canSwapPrecedingSpace = state.canSwapPrecedingSpace,
           let attaching,
           AutoSpacePunctuation.isAttaching(attaching),
           request.settings.isAutoSpaceEnabled
        {
            panel.swapAvailable = canSwapPrecedingSpace()
        }
        return panel
    }

    /// What a pass-through `key` would attach to the armed space: the
    /// character it types — or, under the width-flip chord, the punctuation
    /// it maps to, which is what the swap writes then.
    private static func swapCandidate(for key: KeyEventSnapshot, settings: SettingsStore) -> String? {
        guard let typed = ComposingKeyIntent.documentText(of: key) else { return nil }
        guard ComposingKeyIntent.widthFlipCharacter(key) != nil else { return typed }
        return FullWidthPunctuation.documentPunctuation(typed, isWidthFlip: true, settings: settings) ?? typed
    }

    // MARK: - Effects

    private static func effect(_ effect: Taigi_DesktopShell_Effect) -> ComposingBackendEffect? {
        switch effect.effect {
        case let .setMarkedText(marked):
            .setMarkedText(marked.text, caretUTF16: Int(marked.caretUtf16))
        case .clearMarkedText_p:
            .clearMarkedText
        case let .insertText(insert):
            .insertText(insert.text)
        case let .swapPrecedingSpace(swap):
            .swapPrecedingSpace(replacement: swap.replacement)
        case .armSwap:
            .armSwap
        case let .candidatesChanged(list):
            .candidatesChanged(CandidateListUpdate(
                cells: list.cells.map {
                    CandidateCellContent(text: $0.text, annotation: $0.hasAnnotation ? $0.annotation : nil)
                },
                leadsWithLiteralRoman: list.leadsWithLiteralRoman,
                markedTextLengthUTF16: Int(list.markedTextLengthUtf16),
            ))
        case .candidatesClosed:
            .candidatesClosed
        case let .navigate(navigate):
            navigation(navigate.direction).map(ComposingBackendEffect.navigate)
        case nil:
            nil
        }
    }

    private static func navigation(_ direction: Taigi_DesktopShell_CandidateNavigation) -> CandidateNavigation? {
        switch direction {
        case .left: .left
        case .right: .right
        case .up: .up
        case .down: .down
        case .pageUp: .pageUp
        case .pageDown: .pageDown
        case .nextCandidate: .nextCandidate
        case .previousCandidate: .previousCandidate
        case .unspecified, .UNRECOGNIZED: nil
        }
    }
}
