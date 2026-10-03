// The keys that pick a candidate out of the nine slots: the bare letters
// under Standard, the bare digits under Telex.

import AppKit
@testable import TaigiInputMethodCore
import XCTest

/// What these pin: the shipped set, the labels, and that no chord can sit on
/// a slot key. Which key picks which slot, and when, is desktop-core's
/// (`keys/intent.rs`, `keys/slot_key_set.rs`). The snapshots are built by
/// hand where a layout is being described: `⇧3` types `#` on a US layout.
@MainActor
final class CandidateSlotKeyTests: XCTestCase {
    private func snapshot(
        _ characters: String,
        modifiers: NSEvent.ModifierFlags = [],
        charactersIgnoringModifiers: String? = nil,
        keyCode: UInt16? = nil,
    ) -> KeyEventSnapshot {
        KeyEventSnapshot(
            characters: characters,
            modifiers: modifiers,
            isNamedSpecialKey: false,
            charactersIgnoringModifiers: charactersIgnoringModifiers,
            keyCode: keyCode,
        )
    }

    /// `⇧3` as a US layout reports it: `#` in both character fields, and the
    /// digit only in the key code.
    private func shiftedDigitUS(_ digit: Int) -> KeyEventSnapshot {
        let symbols = ["!", "@", "#", "$", "%", "^", "&", "*", "("]
        return snapshot(
            symbols[digit - 1], modifiers: .shift,
            keyCode: ComposingKeyChord.numberRowKeyCodes[digit - 1],
        )
    }

    /// `⇧;` as a US layout reports it: `:` in both character fields, and the
    /// `;` only in the key code.
    private func shiftedSemicolonUS() -> KeyEventSnapshot {
        snapshot(":", modifiers: .shift, keyCode: ComposingKeyChord.semicolonKeyCode)
    }

    /// The set is the shipped one: a fresh `ComposingKeyBindings` reads it,
    /// and so does a store with nothing recorded.
    func testTheBareKeys_areTheShippedSet() throws {
        XCTAssertEqual(ComposingKeyBindings.default.slotKeySet, .bareKeys)
        XCTAssertEqual(try makeScratchSettingsStore().composingKeyBindings.slotKeySet, .bareKeys)
    }

    // MARK: - What the recorder would store

    /// Every slot key of every set is a typing key to the gate, so no chord
    /// can ever sit on one — under either scheme, whichever is live when it
    /// is recorded. This is what lets the bindings skip a pass against the
    /// slot tier.
    func testEverySlotKey_ofEverySet_isRefusedAsAChord() {
        for keySet in CandidateSlotKeySet.allCases {
            for slot in 0 ..< HorizontalPageLayout.pageSize {
                let key = keySet.label(forSlot: slot)
                XCTAssertEqual(
                    ComposingKeyChord.make(key: key, modifiers: []),
                    .failure(.typesRomanization), "\(keySet) slot \(slot) (`\(key)`)",
                )
            }
        }
    }

    /// A shifted number-row key is refused as the digit it is, read off the
    /// key code since a US layout types `#` for it — the same answer the
    /// Carbon bridge gives when handed the unmodified `3` with Shift, so a
    /// press no composing row can hold is one no global row bridges to.
    func testAShiftedDigit_isRefusedAsTheDigitItIs_onBothPaths() throws {
        XCTAssertEqual(ComposingKeyChord.make(shiftedDigitUS(3)), .failure(.typesRomanization))
        XCTAssertEqual(ComposingKeyChord.make(key: "3", modifiers: .shift), .failure(.typesRomanization))
        // A symbol reached without the number row — a keypad or another
        // layout's own `#` key — is still the character it types.
        XCTAssertEqual(try ComposingKeyChord.make(snapshot("#", modifiers: .shift)).get().key, "#")
    }

    /// `⇧;` is refused the same way: it is the ninth slot key's Hanji/romanization chord,
    /// and a US layout types `:` for it. A `:` reached without that key
    /// still records.
    func testAShiftedSemicolon_isRefusedAsTheSlotKeyItIs() throws {
        XCTAssertEqual(ComposingKeyChord.make(shiftedSemicolonUS()), .failure(.typesRomanization))
        XCTAssertEqual(try ComposingKeyChord.make(snapshot(":", modifiers: .shift)).get().key, ":")
    }

    /// Every other shifted key keeps the character it types, which is what
    /// its stored chords already hold.
    func testOtherShiftedKeys_stillRecordTheCharacterTheyType() throws {
        let chord = try ComposingKeyChord.make(
            snapshot("{", modifiers: .shift, charactersIgnoringModifiers: "{"),
        ).get()

        XCTAssertEqual(chord.key, "{")
        XCTAssertEqual(chord.modifiers, .shift)
    }

    // MARK: - The labels

    func testTheLabels_nameTheKeysThemselves() {
        XCTAssertEqual((0 ..< 9).map { CandidateSlotKeySet.bareKeys.label(forSlot: $0) }, CandidateSlotKeySet.bareKeyRow)
        XCTAssertEqual(
            (0 ..< 9).map { CandidateSlotKeySet.digits.label(forSlot: $0) },
            ["1", "2", "3", "4", "5", "6", "7", "8", "9"],
        )
    }
}
