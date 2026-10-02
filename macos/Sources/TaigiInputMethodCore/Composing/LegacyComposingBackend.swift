// The Swift key path behind `ComposingBackend`: the intent switch, the
// candidate commits, auto space and full-width punctuation.

import Foundation

/// The key path as `TaigiInputController` ran it before the seam, recording
/// what it used to do to the client and the window as `ComposingBackendEffect`s.
/// Temporary: deleted with the rest of the Swift key path once the core back
/// end has taken over (roadmap P13).
@MainActor
final class LegacyComposingBackend: ComposingBackend {
    /// The controller's category, where these lines logged before the seam.
    private static let logger = DebugLogger(category: "InputController")

    private let coordinator: ComposingSessionCoordinator

    /// The one Swift-side mirror of the engine's composition, driven only
    /// for the session the coordinator says is focused.
    private let manager: ComposingManager

    /// The candidates the last fetch returned and the cells the window shows
    /// for them (`CandidateSource`). The absolute indices the
    /// `CandidatePresenter` seam answers with name PRESENTED cells, so a
    /// commit resolves them through `source.resolve`; the selection itself
    /// lives in the window, which owns the measured page geometry the
    /// selection moves through.
    ///
    /// One for the process, like the composition it describes: only the
    /// session that owns the engine reaches it (`owningManager`), and `activate`
    /// drops it with the window on every handover — so a list left behind
    /// by `release` is never read.
    private var source = CandidateSource.empty

    init(coordinator: ComposingSessionCoordinator, manager: ComposingManager) {
        self.coordinator = coordinator
        self.manager = manager
    }

    func owns(_ session: ComposingSessionToken) -> Bool {
        coordinator.owns(session)
    }

    func isComposing(_ session: ComposingSessionToken) -> Bool {
        owns(session) && manager.isComposing
    }

    func activate(_ session: ComposingSessionToken) {
        if coordinator.claim(session) {
            manager.startNewSession()
        }
        source = .empty
    }

    func release(_ session: ComposingSessionToken) {
        if coordinator.release(session) {
            manager.startNewSession()
        }
    }

    func key(
        _ key: KeyEventSnapshot,
        bindings: ComposingKeyBindings,
        in request: ComposingRequest,
    ) -> ComposingKeyReply? {
        guard let manager = owningManager(for: request) else { return nil }
        let effects = EffectRecorder()
        let settings = request.settings

        // A window the user switched off since the last key comes down HERE,
        // before the key is read: the setting's observer runs on a later
        // main-actor turn, and a Return classified against a bar still up
        // would pick a candidate the user asked never to see.
        if !settings.isCandidateWindowEnabled, !source.isEmpty {
            closeList(effects)
        }

        let intent = ComposingKeyIntent.intent(
            for: key,
            isComposing: manager.isComposing,
            isShowingCandidates: !source.isEmpty,
            bindings: bindings,
        )
        Self.logger.debug("key intent \(String(describing: intent))")

        switch intent {
        case let .input(text):
            manager.append(text, executing: effects)
            refreshCandidates(from: manager, settings: settings, effects)
        case let .telexKey(key):
            manager.telexKey(key, executing: effects)
            refreshCandidates(from: manager, settings: settings, effects)
        case .deleteBackward:
            manager.deleteBackward(executing: effects)
            refreshCandidates(from: manager, settings: settings, effects)
        case let .moveCaret(direction):
            // No refetch: the text did not change, so the candidates, the
            // highlight and the page still describe it. The bar stays
            // anchored where it was — at the end of the marked region.
            manager.moveCaret(direction, executing: effects)
        case .commit:
            commitAsTyped(from: manager, settings: settings, effects)
        case .cancel:
            manager.cancelComposition(executing: effects)
            closeList(effects)
        case let .commitThenInsert(text):
            // Mapped before the auto-space augmentation so the full-width
            // character rides the same single mutation as the commit. Both
            // rewrites CAN fire here: this path commits the preedit as typed,
            // which is romanization under every mode, while the full-width map
            // still answers to the output mode — so Hanji mode + Auto-Space gets
            // `taigi？ `. The full-width map reading the mode rather than the
            // committed string is a separate approximation, untouched here.
            let isWidthFlip = ComposingKeyIntent.widthFlipCharacter(key) != nil
            let documentText = FullWidthPunctuation.documentPunctuation(
                text, isWidthFlip: isWidthFlip, settings: settings,
            ) ?? text
            let insert = AutoSpacePolicy.augmentInsert(
                documentText,
                afterComposition: manager.displayText,
                isGateActive: isAutoSpaceGateActive(
                    wroteRomanization: AutoSpacePolicy.rawPreeditWritesRomanization(
                        inputMode: settings.inputMode,
                    ),
                    settings: settings,
                ),
            )
            let committedText = manager.commitComposition(thenInsert: insert.text, executing: effects)
            closeList(effects)
            // Armed only when the engine really wrote the mutation — a commit
            // it ignored left the document without the space to swap with.
            if insert.leavesTrailingAutoSpace, committedText != nil {
                effects.record(.armSwap)
            }
        case .commitThenPassThrough:
            manager.commitComposition(executing: effects)
            closeList(effects)
            return ComposingKeyReply(handled: false, effects: effects.recorded)
        case .passThrough:
            // Attaching punctuation typed right after an auto-inserted space
            // swaps with it (`guá ` + `?` → `guá? `) instead of reaching the
            // host — one of the two pass-through keys this input method
            // consumes.
            //
            // Read BEFORE the full-width map, and since the Hanji/romanization key that
            // ordering decides a real case rather than an impossible one: in
            // Hanji mode Space writes a romanization and arms a space, and the
            // `?` that follows matches both rules. The swap wins, and should —
            // the word in front of the caret is romanization, which reads as
            // Latin text and takes Latin punctuation, whatever the mode would
            // say about a hanji word. Pinned by
            // `AutoSpaceControllerTests.testTheSwapFollowsASpaceTheAlternate…`.
            //
            // The width-flip chord is the exception to that ordering: the user
            // named the width, so the swap attaches the glyph they asked for
            // (`guá ` + `⌃,` in romanization mode → `guá， `).
            guard let typed = ComposingKeyIntent.documentText(of: key) else {
                return ComposingKeyReply(handled: false, effects: effects.recorded)
            }
            let isWidthFlip = ComposingKeyIntent.widthFlipCharacter(key) != nil
            let punctuation = FullWidthPunctuation.documentPunctuation(
                typed, isWidthFlip: isWidthFlip, settings: settings,
            )
            if swapAutoSpace(
                inserting: isWidthFlip ? punctuation ?? typed : typed,
                in: request, manager: manager, effects,
            ) {
                return ComposingKeyReply(handled: true, effects: effects.recorded)
            }
            // Punctuation this input method writes itself — full-width under
            // the mode, or either width under the flip chord — the other
            // consumed pass-through key. The host cannot map a key it types
            // itself (and would read the chord as a shortcut), so the
            // character is written here instead, and the engine hears about it
            // the same way it would have below.
            if let punctuation {
                effects.record(.insertText(punctuation))
                manager.noteCharacterTypedOutsideComposition(punctuation)
                return ComposingKeyReply(handled: true, effects: effects.recorded)
            }
            // The host gets the key either way. Text going into the document
            // without passing through a composition is still context, though:
            // a full stop typed here is what ends the sentence the next-word
            // learning would otherwise carry across.
            manager.noteCharacterTypedOutsideComposition(typed)
            return ComposingKeyReply(handled: false, effects: effects.recorded)
        case .commitHighlightedCandidate:
            // The window is authoritative for which absolute index its selection
            // is on. The cell's own script: under Hanji with Romanization that is the Hanji for
            // a Hanji cell and the romanization for a romanization cell.
            commitPresented(
                at: request.panel.selectedIndex(), flip: false,
                from: manager, settings: settings, effects,
            )
        case .commitAlternateScript:
            // The Hanji/romanization key: same candidate the highlight is on, written in the
            // script the cell does NOT stand for. A candidate that has only one
            // answers `.ignored` inside the commit, so the key is consumed and
            // nothing happens — the same answer `⌃7` gets on a page with no
            // seventh slot.
            commitPresented(
                at: request.panel.selectedIndex(), flip: true,
                from: manager, settings: settings, effects,
            )
        case let .selectCandidateSlot(slot, flip):
            // A key aimed at one of the empty slots the last page ends with
            // resolves to no index, and is consumed all the same: `y` is a
            // slot key while the bar is up, and typing it into the composition
            // only when the page happens to be short would make a letter land
            // at random. `flip` is ⇧ on the key: the same cell in its other
            // script, with Space's "nothing to write" answer for a cell that
            // has only one.
            commitPresented(
                at: request.panel.indexForKeySlot(slot), flip: flip,
                from: manager, settings: settings, effects,
            )
        case let .navigate(direction):
            // The window interprets the direction for its layout and repaints
            // itself — nothing comes back, because the window is authoritative
            // for the selection and the commit paths above ask it.
            effects.record(.navigate(direction))
        }
        return ComposingKeyReply(handled: true, effects: effects.recorded)
    }

    func commitComposition(in request: ComposingRequest) -> [ComposingBackendEffect]? {
        guard let manager = owningManager(for: request) else { return nil }
        let effects = EffectRecorder()
        manager.commitComposition(executing: effects)
        return effects.recorded
    }

    /// Commit first, as vChewing does (`InputHandler_HandleStates.swift:1110`):
    /// the picker writes into the document, and a composition still marked
    /// there would have the symbol land inside it.
    func commitForSymbolPicker(in request: ComposingRequest) -> [ComposingBackendEffect]? {
        guard let manager = owningManager(for: request) else { return nil }
        let effects = EffectRecorder()
        if let highlighted = request.panel.selectedIndex() {
            commitPresented(at: highlighted, flip: false, from: manager, settings: request.settings, effects)
        } else {
            commitAsTyped(from: manager, settings: request.settings, effects)
        }
        return effects.recorded
    }

    /// Writes `symbol` at the caret as one string — so a bracket pair lands
    /// as both halves, with the caret after the closing one (IMK has no way
    /// to put it between them, and one rule for both platforms beats a
    /// TSF-only exception) — and not through the full-width map: what the
    /// user picked is what they get, `()` included. The one picker key that
    /// touches the document, so the one that spends the auto-space arm: an
    /// attaching mark swaps with the space a commit left, as a typed one
    /// would, and the engine hears about the character either way.
    func insertSymbol(_ symbol: String, in request: ComposingRequest) -> [ComposingBackendEffect]? {
        guard let manager = owningManager(for: request) else { return nil }
        let effects = EffectRecorder()
        if swapAutoSpace(inserting: symbol, in: request, manager: manager, effects) {
            return effects.recorded
        }
        effects.record(.insertText(symbol))
        manager.noteCharacterTypedOutsideComposition(symbol)
        return effects.recorded
    }

    /// The Hanji/romanization swap (`refetch` false) re-renders the list on
    /// screen: the presented list is rebuilt through the manager, whose
    /// `presentation(for:)` reads the live settings the toggle just wrote.
    ///
    /// A Candidate Display change (`refetch`) fetches it again: unlike the
    /// swap, this setting changes WHICH candidates exist — under Romanization
    /// Only the engine collapses same-roman rows (§44) — so a repaint of the
    /// old fetch would keep the duplicates on screen. The bar goes down only
    /// when the composition is gone or the new list is empty.
    func represent(refetch: Bool, in request: ComposingRequest) -> CandidateListRepresentation? {
        guard let manager = owningManager(for: request) else { return nil }
        guard !source.isEmpty else { return .unchanged }
        if refetch {
            guard case let .found(fetched) = manager.fetchCandidates(), !fetched.isEmpty else {
                source = .empty
                return .closed
            }
            source = CandidateSource(candidates: fetched, manager: manager)
        } else {
            source = CandidateSource(candidates: source.candidates, manager: manager)
        }
        return .changed(listUpdate(from: manager))
    }

    /// The manager when `request`'s session owns it — with the list brought
    /// into line with the window first, so nothing below reads a list the
    /// user cannot see.
    private func owningManager(for request: ComposingRequest) -> ComposingManager? {
        guard coordinator.owns(request.session) else { return nil }
        if !request.panel.isListOnScreen {
            source = .empty
        }
        return manager
    }

    // MARK: - Candidates

    /// Commits the cell the window shows at `index` — in the cell's own
    /// script, or with `flip` the other script of the same candidate (what
    /// Space asks for). A nil index and one past the list both mean "nothing
    /// to commit", and the key is consumed either way: letting a Space through
    /// would drop a stray space into a document whose composition is still
    /// running, and committing would write a candidate the user cannot see.
    private func commitPresented(
        at index: Int?,
        flip: Bool,
        from manager: ComposingManager,
        settings: SettingsStore,
        _ effects: EffectRecorder,
    ) {
        guard let index, let (candidate, script) = source.resolve(cellIndex: index, flip: flip)
        else { return }
        commit(candidate, script: script, from: manager, settings: settings, effects)
    }

    /// Commits one candidate in `script` — resolved by the caller from the
    /// cell — and shows whatever the composition became.
    private func commit(
        _ candidate: ContinuousCandidate,
        script: CandidateScript,
        from manager: ComposingManager,
        settings: SettingsStore,
        _ effects: EffectRecorder,
    ) {
        let outcome = manager.commitCandidate(candidate, script: script, executing: effects)
        Self.logger.debug("candidate commit \(String(describing: outcome))")
        switch outcome {
        case let .finalized(earnsAutoSpace):
            closeList(effects)
            // Final commit only, mirroring iOS (`ActionHandler+Suggestions.swift:129-133`):
            // a nailed segment keeps composing more syllables — and writes
            // nothing to the document under Model B anyway.
            //
            // The engine's verdict on what the pick wrote is carried into the
            // gate rather than re-derived here: spacing is a property of
            // ROMANIZATION (no trailing `-`), and both the Hanji/romanization
            // key and a candidate with no Hanji write a script the output mode
            // alone would name wrong (`AutoSpacePolicy.isGateActive`). So the
            // answer follows the document — a romanization written in Hanji
            // mode is spaced, a hanji written in romanization mode is not — and
            // Auto-Space OFF still means no space anywhere (USER 2026-08-25).
            if isAutoSpaceGateActive(wroteRomanization: earnsAutoSpace, settings: settings) {
                appendAutoSpace(effects)
            }
        case .nailed, .ignored, .unavailable:
            // Anything short of a finished composition is answered by asking the
            // engine what it is holding NOW rather than by reading the outcome:
            // a commit the engine ignored may have been ignored because a
            // generation change had already reset it to Idle
            // (`CandidateOutcomes.swift`), and treating that as "nothing
            // changed" would leave a bar describing a composition that is gone.
            refreshCandidates(from: manager, settings: settings, effects)
        }
    }

    /// Ends the composition as typed — the romanization with its tone marks —
    /// and spaces it if the auto-space gate says so. What ⇧Return does, and
    /// what the symbol picker does to a composition with no highlight to take.
    private func commitAsTyped(
        from manager: ComposingManager,
        settings: SettingsStore,
        _ effects: EffectRecorder,
    ) {
        let committedText = manager.commitComposition(executing: effects)
        closeList(effects)
        guard let committedText,
              isAutoSpaceGateActive(
                  wroteRomanization: AutoSpacePolicy.rawPreeditWritesRomanization(inputMode: settings.inputMode),
                  settings: settings,
              ),
              AutoSpacePolicy.shouldAppendSpace(afterCommitting: committedText)
        else { return }
        appendAutoSpace(effects)
    }

    /// Re-reads the candidates for the composition as it now stands, and shows
    /// them.
    private func refreshCandidates(
        from manager: ComposingManager,
        settings: SettingsStore,
        _ effects: EffectRecorder,
    ) {
        // With the window off nothing is fetched, not merely not shown: a
        // list kept behind no window would turn `isShowingCandidates` on and
        // hand Space and the slot keys to candidates the user cannot see.
        // The composition itself is untouched — the engine still composes
        // it, and Return writes it as typed (`ComposingKeyIntent`).
        guard settings.isCandidateWindowEnabled else {
            closeList(effects)
            return
        }
        switch manager.fetchCandidates() {
        case .unavailable:
            // The QUERY left the engine as it was, but the keystroke before it
            // did not: the character is already in the buffer and already in the
            // marked region. Candidates fetched for the previous buffer would
            // offer spans measured against text that has since changed, and
            // `CommitContinuous` only checks that a span is consumable — not
            // that it came from the composition on screen.
            Self.logger.debug("candidate fetch unavailable — taking the bar down")
            closeList(effects)
        case .notComposing:
            closeList(effects)
        case let .found(fetched):
            source = CandidateSource(candidates: fetched, manager: manager)
            if fetched.isEmpty {
                closeList(effects)
            } else {
                effects.record(.candidatesChanged(listUpdate(from: manager)))
            }
        }
    }

    private func listUpdate(from manager: ComposingManager) -> CandidateListUpdate {
        CandidateListUpdate(
            cells: source.cells,
            leadsWithLiteralRoman: source.leadsWithLiteralRoman,
            markedTextLengthUTF16: manager.displayText.utf16.count,
        )
    }

    private func closeList(_ effects: EffectRecorder) {
        source = .empty
        effects.record(.candidatesClosed)
    }

    // MARK: - Auto-space

    /// The gate every auto-space site reads — Auto-Space live, so a toggle
    /// flipped in the settings window applies to the very next commit, and
    /// `wroteRomanization` from whatever resolved the string this commit wrote.
    ///
    /// The verdict is never derived from the output mode here. A candidate
    /// commit gets it from the engine (`CommitResolution.earns_auto_space`), a
    /// preedit commit from `rawPreeditWritesRomanization`, and the swap from
    /// the armed record of the commit that wrote the space.
    private func isAutoSpaceGateActive(wroteRomanization: Bool, settings: SettingsStore) -> Bool {
        AutoSpacePolicy.isGateActive(
            isAutoSpaceEnabled: settings.isAutoSpaceEnabled,
            wroteRomanization: wroteRomanization,
        )
    }

    /// Writes the trailing auto space after a commit that earned it, and arms
    /// the punctuation swap on it.
    ///
    /// A second document mutation rather than part of the commit's: whether
    /// the space is earned depends on the text the engine decided to write,
    /// which is only known once the commit has run.
    ///
    /// Called from the explicit commit paths only. The lifecycle commits
    /// (`commitComposition` on deactivate, close, or a click outside) leave
    /// the document alone: the user did not finish a word there, and a space
    /// appearing at the old caret after focus moved on reads as corruption.
    private func appendAutoSpace(_ effects: EffectRecorder) {
        effects.record(.insertText(" "))
        effects.record(.armSwap)
    }

    /// Replaces the auto space before the caret with `?` + space — the
    /// smart-punctuation swap (`guá ` + `?` → `guá? `), matching iOS
    /// (`ActionHandler+KeyActions.swift:132-147`). Answers whether the key was
    /// consumed; the controller's `canSwapPrecedingSpace` makes the client
    /// checks, and a client that fails one gets the key passed through
    /// untouched. Re-armed on success, so `?!` chains keep swapping.
    private func swapAutoSpace(
        inserting characters: String,
        in request: ComposingRequest,
        manager: ComposingManager,
        _ effects: EffectRecorder,
    ) -> Bool {
        // Auto-Space is re-read against the setting as it stands NOW,
        // deliberately: switching the feature off invalidates the space it
        // left behind, and the key maps or passes through instead
        // (`AutoSpaceControllerTests.testTheToggleFlippedOffAfterTheCommit…`).
        // The COMMIT's verdict is not re-derived, though — it is the armed
        // record's. What is in front of the caret is romanization or Hanji as
        // a matter of history, and a display mode changed since then does not
        // rewrite it; asking the mode again would refuse to swap a space this
        // controller had just written.
        guard let canSwapPrecedingSpace = request.panel.canSwapPrecedingSpace,
              AutoSpacePunctuation.isAttaching(characters),
              request.settings.isAutoSpaceEnabled,
              canSwapPrecedingSpace()
        else { return false }
        effects.record(.swapPrecedingSpace(replacement: characters + " "))
        // The character still ends the next-word context, exactly as it would
        // have on the pass-through path it was consumed from.
        manager.noteCharacterTypedOutsideComposition(characters)
        effects.record(.armSwap)
        return true
    }
}

/// Collects one request's effects in order — the engine's, through the
/// `ComposingEffectExecutor` the manager writes to, and the key path's own.
/// The engine effects are translated rather than carried as they are: the
/// core back end answers the same `ComposingBackendEffect`s and has no
/// `ComposingTransition` to send.
@MainActor
private final class EffectRecorder: ComposingEffectExecutor {
    private(set) var recorded: [ComposingBackendEffect] = []

    func record(_ effect: ComposingBackendEffect) {
        recorded.append(effect)
    }

    /// The three engine effects that reach a client, as `ClientEffectExecutor`
    /// writes them; the rest it ignores, and so does this.
    func execute(_ effect: ComposingTransition.Effect) {
        switch effect {
        case let .updatePreedit(text, caretUTF16):
            record(.setMarkedText(text, caretUTF16: caretUTF16))
        case .clearPreeditWithoutCommit:
            record(.clearMarkedText)
        case let .commitTextReplacingPreedit(text):
            record(.insertText(text))
        case .clearCandidates, .refreshCandidates, .resetCandidateContext,
             .nextWordUpdateLastSelectedWord, .nextWordWordSelected, .nextWordClearForNewComposing:
            break
        }
    }
}
