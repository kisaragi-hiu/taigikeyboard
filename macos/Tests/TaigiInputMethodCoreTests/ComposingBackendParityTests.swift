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
/// Reachable elsewhere or not at all:
/// - E6 — the chord codec, a row of the chord table
///   (`ChordRulesCrossCheckTests`, `chord_rules.tsv`).
/// - E2b — a plain Escape inside a multi-character event: the Mac's picker
///   classification stays Swift on both back ends (`SymbolPickerIntent`), and
///   the composition's Escape tier reads the first character on both.
@MainActor
final class ComposingBackendParityTests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        restoreStandardSettingsAtTeardown()
        InstalledLexicon.installOnce()
    }

    /// E2 (C3): one key event carrying two characters, mid-composition. The
    /// Mac classifies it by its first character; the core asks every
    /// character to qualify (`keys/intent.rs`).
    func testE2_aKeyEventCarryingSeveralCharacters() throws {
        let session = try composedSession()

        let handled = try session.controller.handle(
            TestFixtures.keyDownEvent(characters: "a."), client: session.client,
        )

        if TestFixtures.isCoreBackEnd {
            XCTAssertTrue(handled)
            XCTAssertEqual(session.client.insertedTexts, ["taigia."], "E2 core: committed with the event as text")
        } else {
            XCTAssertTrue(handled)
            XCTAssertEqual(session.client.insertedTexts, [], "E2 Mac: `a` leads, so the event composes")
        }
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
        let client = RecordingTextInputClient()
        client.caretRects = [4: CGRect(x: 120, y: 400, width: 1, height: 18)]
        let controller = try TestFixtures.makeInputController()
        let presenter = RecordingCandidatePresenter()
        controller.candidatePresenter = presenter
        controller.settings = SettingsStore()
        controller.activateServer(client)
        let session = Session(controller: controller, client: client, presenter: presenter)
        try session.type("taigi")
        return session
    }
}
