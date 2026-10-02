// The open parity items, as each back end answers them today.

import AppKit
@testable import TaigiInputMethodCore
import XCTest

/// Where the core back end and the Swift key path still differ, pinned per
/// back end rather than skipped — every one an accepted item of
/// docs/architecture/macos-desktop-core-roadmap.md (§ Parity decisions), each
/// settled in its own `parity:` PR (P11). Runs in both `swift test`
/// processes (`make -C macos test`), so each process holds its own back end
/// to its own column.
///
/// Settled items keep one expectation here, held by both processes:
/// - E2 (P11c) — a key event carrying several characters.
/// - E4 (P11d) — a format character (Cf). Its next-word cases drive the
///   Swift manager, so only the legacy process runs them; the core's are in
///   `tests/composing_manager.rs`.
///
/// Not here: E2b (P11c) — a plain Escape inside a multi-character event. The
/// picker's classification is Swift on both back ends, so its pin is
/// `SymbolPickerIntentTests.testAnEscapeInsideALongerEvent_closesAndFallsThrough`;
/// the composition's Escape tier reads the first character on both.
@MainActor
final class ComposingBackendParityTests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        restoreStandardSettingsAtTeardown()
        InstalledLexicon.installOnce()
    }

    /// E2 (C3), settled P11c: one key event carrying two characters,
    /// mid-composition. Every character has to qualify on both back ends
    /// (`keys/intent.rs`), so `a.` is document text, not a letter.
    func testE2_aKeyEventCarryingSeveralCharacters() throws {
        let session = try composedSession()

        let handled = try session.controller.handle(
            TestFixtures.keyDownEvent(characters: "a."), client: session.client,
        )

        XCTAssertTrue(handled)
        XCTAssertEqual(session.client.insertedTexts, ["taigia."], "committed with the event as text")
    }

    /// E2 through the commit-then-insert policies: `a.` is not attaching
    /// punctuation, so Auto-Space puts the space BEFORE it, as for any other
    /// text (`AutoSpacePolicy.augmentInsert`); and it is not one mapped
    /// key, so the full-width map leaves it alone while a lone `.` maps.
    func testE2_aSeveralCharacterEventThroughAutoSpaceAndFullWidth() throws {
        UserDefaults.standard.set(true, forKey: SettingsStore.Keys.isAutoSpaceEnabled.name)
        let spaced = try composedSession()
        _ = try spaced.controller.handle(TestFixtures.keyDownEvent(characters: "a."), client: spaced.client)
        XCTAssertEqual(spaced.client.insertedTexts, ["taigi a."], "Auto-Space: the space leads non-attaching text")

        UserDefaults.standard.set(false, forKey: SettingsStore.Keys.isAutoSpaceEnabled.name)
        UserDefaults.standard.set(true, forKey: SettingsStore.Keys.isHanjiFirst.name)
        let fullWidth = try composedSession()
        _ = try fullWidth.controller.handle(TestFixtures.keyDownEvent(characters: "a."), client: fullWidth.client)
        XCTAssertEqual(fullWidth.client.insertedTexts, ["taigia."], "full width maps one key, not `a.`")

        // Positive control: the same session's map is on.
        let control = try composedSession()
        _ = try control.controller.handle(TestFixtures.keyDownEvent(characters: "."), client: control.client)
        XCTAssertEqual(control.client.insertedTexts, ["taigi\u{3002}"])
    }

    /// E2 under Telex, idle: `vx` is two letters, not a tone key with nothing
    /// to mark, so it starts a composition on both back ends (until P11c the
    /// Mac read `v` and passed the event to the host).
    func testE2_aSeveralLetterEventUnderTelex_startsAComposition() throws {
        UserDefaults.standard.set(ToneInputScheme.telex.rawValue, forKey: SettingsStore.Keys.toneInputScheme.name)
        let session = try idleSession()

        let handled = try session.controller.handle(
            TestFixtures.keyDownEvent(characters: "vx"), client: session.client,
        )

        XCTAssertTrue(handled)
        XCTAssertEqual(session.client.insertedTexts, [])
        guard case let .setMarkedText(marked, _) = try XCTUnwrap(session.client.writes.last) else {
            return XCTFail("`vx` should have started a composition — got \(session.client.writes)")
        }
        XCTAssertEqual(marked, "vx")
    }

    /// E4 (K7), settled P11d: a format character (Cf) typed
    /// mid-composition, isolated or inside a longer event. Text on both back
    /// ends (`keys/intent.rs`), so the composition commits with it in one
    /// write; until P11d the Mac committed and handed the key to the host.
    func testE4_aFormatCharacterMidComposition() throws {
        for text in ["\u{200B}", "x\u{200D}y", "👩\u{200D}💻"] {
            let session = try composedSession()

            let handled = try session.controller.handle(
                TestFixtures.keyDownEvent(characters: text), client: session.client,
            )

            XCTAssertTrue(handled, "\(text.unicodeScalars)")
            XCTAssertEqual(session.client.insertedTexts, ["taigi" + text], "\(text.unicodeScalars)")
        }
    }

    /// E4 through Auto-Space: a format character is not attaching
    /// punctuation, so the space leads it, as for any other text
    /// (`AutoSpacePolicy.augmentInsert`).
    func testE4_aFormatCharacterThroughAutoSpace() throws {
        UserDefaults.standard.set(true, forKey: SettingsStore.Keys.isAutoSpaceEnabled.name)
        let session = try composedSession()

        _ = try session.controller.handle(TestFixtures.keyDownEvent(characters: "\u{200B}"), client: session.client)

        XCTAssertEqual(session.client.insertedTexts, ["taigi \u{200B}"])
    }

    /// E4 idle: the host types a format character with no composition to
    /// end, on both back ends, as it did before P11d.
    func testE4_anIdleFormatCharacter_isTheHosts() throws {
        let session = try idleSession()

        let handled = try session.controller.handle(
            TestFixtures.keyDownEvent(characters: "\u{200B}"), client: session.client,
        )

        XCTAssertFalse(handled)
        XCTAssertEqual(session.client.writes, [])
    }

    /// E4 idle, at the next-word gate: a format character is now document
    /// text, so the pass-through path reports it — an isolated one and an
    /// emoji ZWJ sequence reach the context, `x‍y` stops on its letter
    /// (`ComposingManager.noteCharacterTypedOutsideComposition`). The engine
    /// reads them as noise that keeps the context. The core's side is
    /// `tests/composing_manager.rs`
    /// `e4_a_format_character_reaches_the_next_word_gate`; this is the
    /// Swift manager's, so the legacy process runs it.
    func testE4_anIdleFormatCharacterReachesTheNextWordGate() throws {
        let rig = try LegacyRig()
        defer { rig.release() }

        for text in ["\u{200B}", "x\u{200D}y", "👩\u{200D}💻"] {
            XCTAssertEqual(rig.press(text)?.handled, false, "\(text.unicodeScalars)")
        }

        // `∅` first: the claim started a new session, which forgets the context.
        XCTAssertEqual(rig.nextWord.reported, ["∅", "\u{200B}", "👩\u{200D}💻"])
    }

    /// E4 mid-composition, at the next-word context: the commit is now the
    /// commit-then-insert one, which the engine does not describe to the
    /// learner, so the context is dropped (`∅`,
    /// `ComposingManager.commitComposition(thenInsert:)`) — as for `.` typed
    /// mid-composition, and as the core's `commit_composition_then_insert`
    /// does. Until P11d the raw commit reported the composition as the
    /// selected word. A control character still takes that raw commit.
    func testE4_aFormatCharacterMidComposition_dropsTheNextWordContext() throws {
        let formatCharacter = try LegacyRig()
        defer { formatCharacter.release() }
        formatCharacter.type("taigi")
        let typed = formatCharacter.nextWord.reported.count

        XCTAssertEqual(formatCharacter.press("\u{200B}")?.handled, true)
        XCTAssertEqual(Array(formatCharacter.nextWord.reported.dropFirst(typed)), ["∅"])

        // Negative control: Cc commits raw, which reports the composition.
        let control = try LegacyRig()
        defer { control.release() }
        control.type("taigi")
        let controlTyped = control.nextWord.reported.count

        XCTAssertEqual(control.press("\u{1}")?.handled, false)
        XCTAssertEqual(Array(control.nextWord.reported.dropFirst(controlTyped)), ["taigi"])
    }

    /// C4: Candidate Display changed while the window is switched off and a
    /// list is still held. Through the controller this cannot happen — the
    /// window going off takes the list down at once
    /// (`applyCandidateWindowSettingChange`) — so it is asked of the back
    /// end itself: the Swift refetch never reads Show Candidate Window and
    /// repaints; the core's (`represent_list`) closes.
    func testC4_aRefetchWithTheCandidateWindowOff() throws {
        let backend = ComposingBackends.shared
        let session = ComposingSessionToken()
        let store = SettingsStore()
        backend.activate(session)
        defer { backend.release(session) }
        var isListOnScreen = false
        func request() -> ComposingRequest {
            Self.request(session: session, settings: store, isListOnScreen: isListOnScreen)
        }
        for character in "taigi".map(String.init) {
            let reply = backend.key(
                KeyEventSnapshot(characters: character, modifiers: [], isNamedSpecialKey: false),
                bindings: store.composingKeyBindings,
                in: request(),
            )
            if reply?.effects.contains(where: {
                if case .candidatesChanged = $0 {
                    true
                } else {
                    false
                }
            }) == true {
                isListOnScreen = true
            }
        }
        XCTAssertTrue(isListOnScreen, "taigi puts a list up")
        store.userDefaults.set(false, forKey: SettingsStore.Keys.isCandidateWindowEnabled.name)

        let represented = backend.represent(refetch: true, in: request())

        if TestFixtures.isCoreBackEnd {
            XCTAssertEqual(represented, .closed, "C4 core: the window is off, so the list goes")
        } else {
            guard case .changed = represented else {
                return XCTFail("C4 Mac: refetched and repainted, got \(String(describing: represented))")
            }
        }
    }

    /// E5 (C2) on picked symbols: whether a symbol written outside a
    /// composition reaches the next-word context. The Mac skips it when a
    /// grapheme is a letter or whitespace (`Character`), the core when a
    /// scalar is alphabetic or whitespace (`composing/manager.rs`). Under the
    /// Mac's own predicate every shipped symbol is forwarded, as under the
    /// core's (`tests/composing_manager.rs`
    /// `e5_every_bundled_symbol_reaches_next_word`): the difference never
    /// reaches the picker. The Swift manager's case, so the legacy process
    /// runs it.
    func testE5_everyShippedSymbolReachesNextWord() throws {
        let symbols = try TestFixtures.shippedSymbolTable().symbols
        let nextWord = RecordingNextWordPort()
        let manager = try TestFixtures.makeComposingManager(
            nextWord: nextWord, startingGeneration: TestFixtures.generationCounter.next(),
        )

        for symbol in symbols {
            manager.noteCharacterTypedOutsideComposition(symbol)
        }

        XCTAssertEqual(nextWord.reported, symbols)
    }

    // MARK: - Session

    /// A request with no slot map and no swap, the highlight on the first
    /// cell while a list is up.
    private static func request(
        session: ComposingSessionToken,
        settings: SettingsStore,
        isListOnScreen: Bool,
    ) -> ComposingRequest {
        ComposingRequest(
            session: session,
            settings: settings,
            panel: ComposingPanelState(
                isListOnScreen: isListOnScreen,
                selectedIndex: { isListOnScreen ? 0 : nil },
                indexForKeySlot: { _ in nil },
                canSwapPrecedingSpace: nil,
            ),
        )
    }

    private struct Session: CandidateBarSession {
        let controller: TaigiInputController
        let client: RecordingTextInputClient
        let presenter: RecordingCandidatePresenter
    }

    /// A legacy back end of its own, its session activated, reporting to a
    /// recording next-word port. Skips in the core process
    /// (`TestFixtures.makeComposingManager`).
    @MainActor
    private struct LegacyRig {
        let backend: LegacyComposingBackend
        let nextWord = RecordingNextWordPort()
        let session = ComposingSessionToken()
        let store = SettingsStore()

        init() throws {
            backend = try TestFixtures.makeLegacyBackend(nextWord: nextWord).0
            backend.activate(session)
        }

        @discardableResult
        func press(_ characters: String) -> ComposingKeyReply? {
            backend.key(
                KeyEventSnapshot(characters: characters, modifiers: [], isNamedSpecialKey: false),
                bindings: store.composingKeyBindings,
                in: ComposingBackendParityTests.request(session: session, settings: store, isListOnScreen: false),
            )
        }

        func type(_ text: String) {
            for character in text {
                press(String(character))
            }
        }

        func release() {
            backend.release(session)
        }
    }

    /// A session over `.standard` that has typed `taigi`.
    private func composedSession() throws -> Session {
        let session = try idleSession()
        try session.type("taigi")
        return session
    }

    /// An activated session over `.standard` with nothing typed.
    private func idleSession() throws -> Session {
        let client = RecordingTextInputClient()
        client.caretRects = [4: CGRect(x: 120, y: 400, width: 1, height: 18)]
        let controller = try TestFixtures.makeInputController()
        let presenter = RecordingCandidatePresenter()
        controller.candidatePresenter = presenter
        controller.settings = SettingsStore()
        controller.activateServer(client)
        return Session(controller: controller, client: client, presenter: presenter)
    }
}
