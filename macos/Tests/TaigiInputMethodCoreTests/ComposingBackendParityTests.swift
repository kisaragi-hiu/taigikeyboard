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

    /// E4 (K7): a format character (Cf) typed mid-composition. Not text to
    /// the Mac, which commits and hands the key to the host; text to the
    /// core, which commits and writes it.
    func testE4_aFormatCharacterMidComposition() throws {
        let session = try composedSession()

        let handled = try session.controller.handle(
            TestFixtures.keyDownEvent(characters: "\u{200B}"), client: session.client,
        )

        if TestFixtures.isCoreBackEnd {
            XCTAssertTrue(handled, "E4 core: consumed")
            XCTAssertEqual(session.client.insertedTexts, ["taigi\u{200B}"])
        } else {
            XCTAssertFalse(handled, "E4 Mac: passed to the host")
            XCTAssertEqual(session.client.insertedTexts, ["taigi"])
        }
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
            ComposingRequest(
                session: session,
                settings: store,
                panel: ComposingPanelState(
                    isListOnScreen: isListOnScreen,
                    selectedIndex: { isListOnScreen ? 0 : nil },
                    indexForKeySlot: { _ in nil },
                    canSwapPrecedingSpace: nil,
                ),
            )
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

    private struct Session: CandidateBarSession {
        let controller: TaigiInputController
        let client: RecordingTextInputClient
        let presenter: RecordingCandidatePresenter
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
