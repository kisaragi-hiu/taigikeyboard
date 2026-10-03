// The TPS key panel end to end: the chord that asks for it, the modes it shows under, and who may take it down.

import InputMethodKit
@testable import TaigiInputMethodCore
import XCTest

/// Drives the real controller against the shared panel. What is pinned is
/// the routing — the stored wish, when the panel is up, whose teardown may
/// close it — not how the caps are drawn.
@MainActor
final class TaigiInputControllerTpsKeyboardTests: XCTestCase {
    private var suiteName = ""
    private var userDefaults = UserDefaults.standard

    override func setUpWithError() throws {
        try super.setUpWithError()
        InstalledLexicon.installOnce()
        suiteName = "TaigiInputControllerTpsKeyboardTests.\(UUID().uuidString)"
        userDefaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        // Process-wide, like the guide: a panel left up by one case would be
        // the panel the next case finds.
        TpsKeyboardPanel.shared.hideNow()
    }

    override func tearDown() {
        TpsKeyboardPanel.shared.hideNow()
        userDefaults.removePersistentDomain(forName: suiteName)
        super.tearDown()
    }

    // MARK: - The chord

    func testTheChord_underTps_showsThePanel_andStoresTheWish() throws {
        let session = try makeSession(under: .tps)

        session.controller.performShortcutAction(.showTpsKeyboard)

        XCTAssertTrue(session.controller.settings.isTpsKeyboardShown)
        XCTAssertTrue(TpsKeyboardPanel.shared.isShowing)
        XCTAssertNotNil(TpsKeyboardPanel.shared.owner, "a panel on screen names the session that raised it")
    }

    /// trace: `tps_keyboard_rows` — 12 + 10 + 11 + 10 caps, read from the
    /// core through `KeyRules.tpsKeyboardRows`.
    func testThePanel_drawsTheCoresRows() throws {
        let session = try makeSession(under: .tps)

        session.controller.performShortcutAction(.showTpsKeyboard)

        XCTAssertEqual(TpsKeyboardPanel.shared.capCount, 43)
    }

    func testTheChord_again_hidesThePanel_andClearsTheWish() throws {
        let session = try makeSession(under: .tps)
        session.controller.performShortcutAction(.showTpsKeyboard)

        session.controller.performShortcutAction(.showTpsKeyboard)

        XCTAssertFalse(session.controller.settings.isTpsKeyboardShown)
        XCTAssertFalse(TpsKeyboardPanel.shared.isShowing)
    }

    /// Inert outside TPS, as desktop-core's `is_inert_under` says: no write,
    /// nothing shown.
    func testTheChord_underARomanization_isInert() throws {
        for mode in [InputMode.tl, .poj] {
            let session = try makeSession(under: mode)

            session.controller.performShortcutAction(.showTpsKeyboard)

            XCTAssertNil(userDefaults.object(forKey: SettingsStore.Keys.isTpsKeyboardShown.name), "\(mode)")
            XCTAssertFalse(TpsKeyboardPanel.shared.isShowing, "\(mode)")
        }
    }

    // MARK: - The mode

    /// Leaving TPS takes the panel down and keeps the wish, so coming back
    /// to TPS brings it back.
    func testLeavingTps_hidesThePanel_andComingBack_showsItAgain() throws {
        let session = try makeSession(under: .tps)
        session.controller.performShortcutAction(.showTpsKeyboard)

        session.controller.performShortcutAction(.toggleTps)

        XCTAssertEqual(session.controller.settings.inputMode, .tl)
        XCTAssertFalse(TpsKeyboardPanel.shared.isShowing)
        XCTAssertTrue(session.controller.settings.isTpsKeyboardShown, "the wish is kept for the way back")

        session.controller.performShortcutAction(.toggleTps)

        XCTAssertTrue(TpsKeyboardPanel.shared.isShowing)
    }

    /// The wish stored by an earlier session: the next one to take focus
    /// under TPS puts the panel up.
    func testActivation_underTps_withTheWishStored_showsThePanel() throws {
        userDefaults.set(true, forKey: SettingsStore.Keys.isTpsKeyboardShown.name)

        _ = try makeSession(under: .tps)

        XCTAssertTrue(TpsKeyboardPanel.shared.isShowing)
    }

    func testHidePalettes_hidesThePanel() throws {
        let session = try makeSession(under: .tps)
        session.controller.performShortcutAction(.showTpsKeyboard)

        session.controller.hidePalettes()

        XCTAssertFalse(TpsKeyboardPanel.shared.isShowing)
        XCTAssertTrue(session.controller.settings.isTpsKeyboardShown, "the system's hide is not the user's")
    }

    // MARK: - Ownership

    /// IMK activates the incoming session before it deactivates the outgoing
    /// one: the outgoing session's teardown must not close the panel the
    /// arriving one now shows — the guide's rule.
    func testDeactivate_ofAnotherSession_leavesTheLiveSessionsPanelAlone() throws {
        userDefaults.set(true, forKey: SettingsStore.Keys.isTpsKeyboardShown.name)
        let leaving = try makeSession(under: .tps)
        let arriving = try makeSession(under: .tps)

        leaving.controller.deactivateServer(leaving.client)

        XCTAssertTrue(TpsKeyboardPanel.shared.isShowing, "a session that no longer showed the panel took it down")

        arriving.controller.deactivateServer(arriving.client)

        XCTAssertFalse(TpsKeyboardPanel.shared.isShowing, "the panel goes with the focus of the session showing it")
        XCTAssertTrue(arriving.controller.settings.isTpsKeyboardShown, "focus leaving is not the user's hide")
    }

    // MARK: - Harness

    private struct Session {
        let controller: TaigiInputController
        let client: RecordingTextInputClient
    }

    /// An activated session under `mode`, reading its settings from this
    /// case's own suite.
    private func makeSession(under mode: InputMode) throws -> Session {
        userDefaults.set(mode.rawValue, forKey: SettingsStore.Keys.inputMode.name)
        let client = RecordingTextInputClient()
        let controller = try TestFixtures.makeInputController()
        controller.settings = SettingsStore(userDefaults: userDefaults)
        controller.displayLanguageOverride = TestFixtures.makeDisplayLanguageStore(.hanji, userDefaults: userDefaults)
        controller.candidatePresenter = RecordingCandidatePresenter()
        controller.activateServer(client)
        return Session(controller: controller, client: client)
    }
}
