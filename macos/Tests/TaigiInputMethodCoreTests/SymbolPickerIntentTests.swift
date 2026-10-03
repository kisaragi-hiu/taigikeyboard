// What each key means to an open symbol picker — the pure table.

import AppKit
@testable import TaigiInputMethodCore
import XCTest

final class SymbolPickerIntentTests: XCTestCase {
    private func intent(
        _ characters: String,
        modifiers: NSEvent.ModifierFlags = [],
        navigationKey: NavigationKey? = nil,
        isNamedSpecialKey: Bool = false,
        bindings: ComposingKeyBindings = .default,
    ) -> SymbolPickerIntent {
        SymbolPickerIntent.intent(
            for: KeyEventSnapshot(
                characters: characters,
                modifiers: modifiers,
                isNamedSpecialKey: isNamedSpecialKey || navigationKey != nil,
                navigationKey: navigationKey,
            ),
            bindings: bindings,
        )
    }

    /// One key out, from either level (USER 2026-09-09): Escape closes and is
    /// swallowed. `⌃3` arrives as Escape too and is the host's, so it is not.
    func testEscape_closes_butNotUnderAHostChord() {
        XCTAssertEqual(intent("\u{1B}"), .close)
        XCTAssertEqual(intent("\u{1B}", modifiers: .control), .closeAndPassThrough)
    }

    /// E2b: an Escape with more behind it in one event is not the Escape key
    /// — the picker closes and lets the event through, as the desktop core
    /// answers (`symbol_picker.rs`). Until P11c the first character closed it.
    func testAnEscapeInsideALongerEvent_closesAndFallsThrough() {
        XCTAssertEqual(intent("\u{1B}x"), .closeAndPassThrough)
    }

    func testTheArrowsAndPagingKeys_navigate() {
        XCTAssertEqual(intent("", navigationKey: .leftArrow), .navigate(.left))
        XCTAssertEqual(intent("", navigationKey: .rightArrow), .navigate(.right))
        XCTAssertEqual(intent("", navigationKey: .upArrow), .navigate(.up))
        XCTAssertEqual(intent("", navigationKey: .downArrow), .navigate(.down))
        XCTAssertEqual(intent("", navigationKey: .pageUp), .navigate(.pageUp))
        XCTAssertEqual(intent("", navigationKey: .pageDown), .navigate(.pageDown))
    }

    /// ⇧← extends a selection in the host and is handed back, as the bar
    /// does (`ComposingKeyIntent`).
    func testAShiftedArrow_closesAndFallsThrough() {
        XCTAssertEqual(intent("", modifiers: .shift, navigationKey: .leftArrow), .closeAndPassThrough)
    }

    /// The slot keys follow the tone scheme, exactly as they do on the bar:
    /// the letter row under Standard, the digits under Telex.
    func testTheSlotKeys_followTheToneScheme() {
        XCTAssertEqual(intent("q"), .pickSlot(0))
        XCTAssertEqual(intent(";"), .pickSlot(8))
        XCTAssertEqual(intent("1"), .closeAndPassThrough, "a digit is not a slot key under Standard")

        let telex = ComposingKeyBindings(toneScheme: .telex)
        XCTAssertEqual(intent("1", bindings: telex), .pickSlot(0))
        XCTAssertEqual(intent("9", bindings: telex), .pickSlot(8))
        XCTAssertEqual(intent("q", bindings: telex), .closeAndPassThrough, "a letter is a tone key under Telex")
    }

    /// A digit slot reads the first scalar (the desktop core's rule, P11c):
    /// a digit with a combining scalar behind it picks; `0`, a full-width
    /// digit and a chorded digit do not.
    func testADigitSlot_readsTheFirstScalar() {
        let telex = ComposingKeyBindings(toneScheme: .telex)
        XCTAssertEqual(intent("1\u{20E3}", bindings: telex), .pickSlot(0))
        XCTAssertEqual(intent("0", bindings: telex), .closeAndPassThrough)
        XCTAssertEqual(intent("\u{FF11}", bindings: telex), .closeAndPassThrough)
        XCTAssertEqual(intent("1", modifiers: .command, bindings: telex), .closeAndPassThrough)
    }

    /// The user's own rows are honoured: paging on `[` / `]` and ⇥, and
    /// Return confirms. Space — the Hanji/romanization key on the bar — confirms too, since
    /// a symbol has no other script to commit.
    func testTheBoundRows_pageAndConfirm() {
        XCTAssertEqual(intent("\t", isNamedSpecialKey: true), .navigate(.nextCandidate))
        XCTAssertEqual(intent("\t", modifiers: .shift, isNamedSpecialKey: true), .navigate(.previousCandidate))
        XCTAssertEqual(intent("]"), .navigate(.pageDown))
        XCTAssertEqual(intent("["), .navigate(.pageUp))
        XCTAssertEqual(intent("\r", isNamedSpecialKey: true), .confirm)
        XCTAssertEqual(intent(" "), .confirm)
    }

    /// A rebound paging row pages the picker from its new key, and the old
    /// key no longer does — the picker reads the same contract the bar does.
    func testARecordedPagingChord_isRead() throws {
        let chord = try TestFixtures.chordNoDefaultHolds(key: "\r")
        let bindings = ComposingKeyBindings(chords: [.pageForward: chord])

        XCTAssertEqual(
            intent("\r", modifiers: chord.modifiers, isNamedSpecialKey: true, bindings: bindings),
            .navigate(.pageDown),
        )
        XCTAssertEqual(intent("]", bindings: bindings), .closeAndPassThrough)
    }

    /// E8: a row stored on a precomposed `é` is not read off the decomposed
    /// spelling of the same letter — scalar identity, as the core's picker.
    func testARecordedChord_isReadScalarByScalar() throws {
        let chord = try XCTUnwrap(ComposingKeyChord(rawValue: "c|00E9"))
        let bindings = ComposingKeyBindings(chords: [.pageForward: chord])

        XCTAssertEqual(intent("\u{E9}", modifiers: .control, bindings: bindings), .navigate(.pageDown))
        XCTAssertEqual(intent("e\u{301}", modifiers: .control, bindings: bindings), .closeAndPassThrough)
    }

    /// ⇧Return writes the composition as typed; there is none, so the key is
    /// the host's — and so is any letter, which closes the picker on its way
    /// to starting the composition it would have started anyway.
    func testEverythingElse_closesAndFallsThrough() {
        XCTAssertEqual(intent("\r", modifiers: .shift, isNamedSpecialKey: true), .closeAndPassThrough)
        XCTAssertEqual(intent("a"), .closeAndPassThrough)
        XCTAssertEqual(intent("s", modifiers: .command), .closeAndPassThrough)
        XCTAssertEqual(intent("\u{8}"), .closeAndPassThrough)
    }
}
