// Executable spec for the key predicates the Swift side still reads.

import AppKit
@testable import TaigiInputMethodCore
import XCTest

/// What a key event is to the controller, the symbol picker and the Shortcuts
/// pane: a plain Escape, a navigation key, document text, the width-flip
/// chord. What a key means to a composition is desktop-core's
/// (`keys/intent.rs`).
final class ComposingKeyIntentTests: XCTestCase {
    func testAPlainEscape_isTheEscapeAlone() {
        XCTAssertTrue(ComposingKeyIntent.isPlainEscape(textSnapshot("\u{1B}")))
        XCTAssertFalse(ComposingKeyIntent.isPlainEscape(textSnapshot("\u{1B}", modifiers: .control)), "⌃3 is the host's")
    }

    /// E2b: an Escape with more behind it is not the plain Escape that
    /// closes the picker or the Telex guide (the core's `is_bare_escape`).
    func testAnEscapeInsideALongerEvent_isNotAPlainEscape() {
        let key = KeyEventSnapshot(characters: "\u{1B}x", modifiers: [], isNamedSpecialKey: false)
        XCTAssertFalse(ComposingKeyIntent.isPlainEscape(key))
    }

    /// The mapping from AppKit's own key names, which the snapshot is what
    /// isolates: the symbol picker reads `NavigationKey`, so without this the
    /// six keys could all be extracted as nil.
    func testArrowEvents_areRecognizedAsNavigationKeys() throws {
        let cases: [(String, NavigationKey)] = try [
            (String(XCTUnwrap(UnicodeScalar(NSLeftArrowFunctionKey))), .leftArrow),
            (String(XCTUnwrap(UnicodeScalar(NSRightArrowFunctionKey))), .rightArrow),
            (String(XCTUnwrap(UnicodeScalar(NSUpArrowFunctionKey))), .upArrow),
            (String(XCTUnwrap(UnicodeScalar(NSDownArrowFunctionKey))), .downArrow),
            (String(XCTUnwrap(UnicodeScalar(NSPageUpFunctionKey))), .pageUp),
            (String(XCTUnwrap(UnicodeScalar(NSPageDownFunctionKey))), .pageDown),
        ]

        for (characters, expected) in cases {
            let event = try TestFixtures.keyDownEvent(characters: characters)
            XCTAssertEqual(KeyEventSnapshot(event).navigationKey, expected)
        }
    }

    /// AppKit encodes the arrows in the same private-use range as the function
    /// keys, and only the ones bound above are navigation.
    func testFunctionKeyEvents_areNotNavigationKeys() throws {
        let event = try TestFixtures.keyDownEvent(
            characters: String(XCTUnwrap(UnicodeScalar(NSF5FunctionKey))),
        )

        XCTAssertNil(KeyEventSnapshot(event).navigationKey)
    }

    // MARK: - Document text

    /// The pass-through path reports document text to the engine so a full stop
    /// can end a learning context. What it must NOT report is a key the host
    /// acts on — Escape and Return are pass-through too, and neither is a
    /// character anyone typed into a document.
    func testIsDocumentText_acceptsPrintableCharactersAndRejectsKeysTheHostActsOn() {
        for text in ["。", "、", "!", "?", " ", "台", "x"] {
            XCTAssertTrue(
                isDocumentText(textSnapshot(text)),
                "'\(text)' is text the host puts into its document",
            )
        }
        for text in ["\u{1B}", "\r", "\u{8}", "\u{7F}"] {
            XCTAssertFalse(
                isDocumentText(textSnapshot(text)),
                "a control character is a command, not document text",
            )
        }
    }

    /// `⌘.` and a typed `.` carry the same character. One of them inserts
    /// nothing, and reporting it as document text would end a learning context
    /// on a keystroke that never reached the document.
    func testIsDocumentText_rejectsAHostChordCarryingAPrintableCharacter() {
        // `x` under ⌃ rather than `.`: ⌃. is the width flip, document text by
        // design (`testWidthFlipChord…`).
        for modifier in [NSEvent.ModifierFlags.command, .control, .option] {
            XCTAssertFalse(
                isDocumentText(textSnapshot("x", modifiers: modifier)),
                "a chord is a host command however printable its character is",
            )
        }
        for modifier in [NSEvent.ModifierFlags.command, .option] {
            XCTAssertFalse(
                isDocumentText(textSnapshot(".", modifiers: modifier)),
                "a chord is a host command however printable its character is",
            )
        }
        XCTAssertTrue(
            isDocumentText(textSnapshot(".", modifiers: .shift)),
            "Shift is how the character was typed, not a command",
        )
    }

    func testIsDocumentText_rejectsArrowsAndFunctionKeys() throws {
        let leftArrow = try String(XCTUnwrap(UnicodeScalar(NSLeftArrowFunctionKey)))
        let functionKey = try String(XCTUnwrap(UnicodeScalar(NSF5FunctionKey)))

        XCTAssertFalse(isDocumentText(textSnapshot(leftArrow)))
        XCTAssertFalse(isDocumentText(textSnapshot(functionKey)))
        XCTAssertFalse(
            isDocumentText(textSnapshot("\u{2028}", isNamedSpecialKey: true)),
            "a line separator is a named key AppKit gives us, not typed text",
        )
    }

    func testIsDocumentText_rejectsNothingAtAll() {
        XCTAssertFalse(isDocumentText(textSnapshot(nil)))
        XCTAssertFalse(isDocumentText(textSnapshot("")))
    }

    /// A format character (Cf) is text the user typed, isolated or inside a
    /// longer event — the desktop core's rule (roadmap E4, settled P11d).
    /// Only a control character (Cc) is a command.
    func testFormatCharacters_areDocumentText_andControlCharactersAreNot() {
        // trace: Cf is not Cc and not in F700…F8FF → every scalar is text.
        for text in [
            "\u{200B}", "\u{200C}", "\u{AD}", "\u{FEFF}", "\u{2066}", "x\u{200D}y", "👩\u{200D}💻",
            "🏴\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}", // emoji tag sequence (plane 14 Cf)
        ] {
            let key = textSnapshot(text)
            XCTAssertEqual(ComposingKeyIntent.documentText(of: key), text, "\(text.unicodeScalars)")
        }
        // Negative control: C0, C1 (NEL) and DEL are not text.
        for text in ["\u{1}", "\u{85}"] {
            let key = textSnapshot(text)
            XCTAssertFalse(isDocumentText(key), "\(text.unicodeScalars)")
        }
        XCTAssertFalse(isDocumentText(textSnapshot("\u{7F}")))
    }

    // MARK: - Width flip

    /// ⌃ on a punctuation key types that key in the other width, once. The
    /// predicate answers the key as typed — the width is picked later — and
    /// the key is read under the modifier: `⌃,` arrives as
    /// `,`, `⌃[` as Escape, `⌃⇧,` as `<`.
    func testWidthFlipChord_typesTheMappedKeyInBothStates() throws {
        // trace: AppKit's `charactersIgnoringModifiers` keeps Shift, so ⌃⇧,
        // reads `<`; Control rewrites `[` to `\u{1B}` in `characters` only.
        let comma = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: ",", modifiers: .control, charactersIgnoringModifiers: ",",
        ))
        let bracket = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: "\u{1B}", modifiers: .control, charactersIgnoringModifiers: "[",
        ))
        let angle = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: "<", modifiers: [.control, .shift], charactersIgnoringModifiers: "<",
        ))
        for (key, expected) in [(comma, ","), (bracket, "["), (angle, "<")] {
            XCTAssertEqual(ComposingKeyIntent.widthFlipCharacter(key), expected.first)
            XCTAssertEqual(ComposingKeyIntent.documentText(of: key), expected)
        }
        XCTAssertFalse(ComposingKeyIntent.isPlainEscape(bracket), "⌃[ is the flip, not a cancel")
    }

    /// Exactly ⌃ on a mapped key: another chording modifier beside it, a key
    /// the policy does not map, or a bare key is no flip.
    func testWidthFlipChord_needsExactlyControlOnAMappedKey() throws {
        let withCommand = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: ",", modifiers: [.control, .command], charactersIgnoringModifiers: ",",
        ))
        let withOption = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: ",", modifiers: [.control, .option], charactersIgnoringModifiers: ",",
        ))
        let letter = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: "\u{13}", modifiers: .control, charactersIgnoringModifiers: "s",
        ))
        let hyphen = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: "-", modifiers: .control, charactersIgnoringModifiers: "-",
        ))
        let quote = try KeyEventSnapshot(TestFixtures.keyDownEvent(
            characters: "\"", modifiers: .control, charactersIgnoringModifiers: "\"",
        ))
        for key in [withCommand, withOption, letter, hyphen, quote] {
            XCTAssertNil(ComposingKeyIntent.widthFlipCharacter(key), "\(key)")
        }
        XCTAssertNil(ComposingKeyIntent.widthFlipCharacter(textSnapshot(",")))
    }

    /// Text the host will put into its document, rather than a key it will act on.
    private func isDocumentText(_ key: KeyEventSnapshot) -> Bool {
        ComposingKeyIntent.documentText(of: key) != nil
    }

    private func textSnapshot(
        _ characters: String?,
        modifiers: NSEvent.ModifierFlags = [],
        isNamedSpecialKey: Bool = false,
    ) -> KeyEventSnapshot {
        KeyEventSnapshot(
            characters: characters,
            modifiers: modifiers,
            isNamedSpecialKey: isNamedSpecialKey,
        )
    }
}
