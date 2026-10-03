// A key event as the Swift side reads it, and the key predicates it still shares.

import AppKit

/// A key AppKit names that this input method binds to candidate navigation.
///
/// Its own type rather than `NSEvent.SpecialKey` because that one is not
/// `Sendable`, and its own case list rather than raw scalar constants because
/// recognizing `0xF702` as "left arrow" is data extraction, not key policy — the
/// policy stays with the classifier that reads it (`SymbolPickerIntent`).
enum NavigationKey: Sendable, Equatable {
    case leftArrow
    case rightArrow
    case upArrow
    case downArrow
    case pageUp
    case pageDown

    init?(_ specialKey: NSEvent.SpecialKey) {
        switch specialKey {
        case .leftArrow: self = .leftArrow
        case .rightArrow: self = .rightArrow
        case .upArrow: self = .upArrow
        case .downArrow: self = .downArrow
        case .pageUp: self = .pageUp
        case .pageDown: self = .pageDown
        default: return nil
        }
    }
}

/// The parts of an `NSEvent` a composing decision is made from.
///
/// A value rather than the event itself so the classification can be reasoned
/// about — and tested — without an AppKit event object, and so the decision can
/// be handed between isolation domains: `NSEvent` is a reference type that
/// cannot cross one.
struct KeyEventSnapshot: Sendable {
    let characters: String?
    /// What the same key would have typed with no modifiers held. Carried
    /// because Control rewrites the characters of the digits it is chorded with
    /// — Ctrl+2 through Ctrl+8 arrive as NUL, ESC, FS, GS, RS, US and DEL — so
    /// `characters` would read Ctrl+3 as an Escape and cancel the composition
    /// the chord was meant to pick a candidate from.
    let charactersIgnoringModifiers: String?
    /// The virtual key code (`NSEvent.keyCode`) — the key's position as the
    /// system reports it, before any layout turns it into a character.
    /// Carried for the recorder's refusal of a shifted number-row key alone:
    /// `⇧3` types `#` on a US layout and `charactersIgnoringModifiers` keeps
    /// Shift, so the number row's codes are the one thing that still says
    /// which key was pressed (`ComposingKeyChord.make(_:)`). Nil for a
    /// snapshot built without an event.
    let keyCode: UInt16?
    let modifiers: NSEvent.ModifierFlags
    /// Whether AppKit has a name for this key (`NSEvent.SpecialKey`). Which
    /// name is not recorded: the keys this input method binds are recognized by
    /// their characters, and the rest only need to be told apart from text.
    let isNamedSpecialKey: Bool
    /// The navigation key this event is, if it is one of the six.
    let navigationKey: NavigationKey?
    /// Whether this key-down is the keyboard's auto-repeat of a key still
    /// held (`NSEvent.isARepeat`). Read by the symbol-picker chord, which
    /// toggles: a held chord would otherwise open and close the picker on
    /// every repeat. False for a snapshot built without an event.
    let isRepeat: Bool
    /// `NSEvent.specialKey?.rawValue` — what the desktop core reads a named
    /// key from (`KeyEvent.special_key`). Nil for a key AppKit has no name
    /// for, and for a snapshot built without an event.
    let specialKeyRawValue: UInt32?

    init(
        characters: String?,
        modifiers: NSEvent.ModifierFlags,
        isNamedSpecialKey: Bool,
        charactersIgnoringModifiers: String? = nil,
        keyCode: UInt16? = nil,
        navigationKey: NavigationKey? = nil,
        isRepeat: Bool = false,
        specialKeyRawValue: UInt32? = nil,
    ) {
        self.characters = characters
        self.charactersIgnoringModifiers = charactersIgnoringModifiers ?? characters
        self.keyCode = keyCode
        self.modifiers = modifiers
        self.isNamedSpecialKey = isNamedSpecialKey
        self.navigationKey = navigationKey
        self.isRepeat = isRepeat
        self.specialKeyRawValue = specialKeyRawValue
    }

    init(_ event: NSEvent) {
        // Read only off a key event: `NSEvent.keyCode` and `isARepeat` raise
        // on any other type, and the recorder's monitor also sees mouse-ups.
        let isKeyEvent = event.type == .keyDown || event.type == .keyUp
        self.init(
            characters: event.characters,
            modifiers: event.modifierFlags,
            isNamedSpecialKey: event.specialKey != nil,
            charactersIgnoringModifiers: event.charactersIgnoringModifiers,
            keyCode: isKeyEvent ? event.keyCode : nil,
            navigationKey: event.specialKey.flatMap(NavigationKey.init),
            isRepeat: isKeyEvent && event.isARepeat,
            specialKeyRawValue: event.specialKey.flatMap { UInt32(exactly: $0.rawValue) },
        )
    }
}

/// What the Swift side still reads off a key event: the modifier sets and the
/// text predicates the controller, the symbol picker and the Shortcuts pane
/// share. What a key means to a composition is desktop-core's
/// (`desktop/crates/taigi-desktop-core/src/keys/intent.rs`).
enum ComposingKeyIntent {
    /// AppKit encodes function and arrow keys as private-use scalars rather
    /// than control characters, so a scalar check alone would let F5 through as
    /// composition input.
    private static let appKitFunctionKeyRange: ClosedRange<UInt32> = 0xF700 ... 0xF8FF

    /// The chords the host owns. Named once because several rules are written
    /// against it — `documentText`, the symbol picker's plain keys and the
    /// Telex guide's Escape (`TaigiInputController.handle`) — and a list
    /// spelled out at each of them is a list that can drift apart.
    static let hostChords: NSEvent.ModifierFlags = [.command, .control, .option]

    /// The modifier under which ← / → step the composing caret. The Shortcuts
    /// pane draws its read-only row from this value; desktop-core reads the
    /// same chord (`keys/intent.rs`).
    static let caretChordModifiers: NSEvent.ModifierFlags = [.option]

    /// The text `key` puts into the document, or nil when it is a key the host
    /// acts on. Takes the whole event rather than its characters: `⌘.` and a
    /// typed `.` carry the same character, and one of them is a host command
    /// that inserts nothing — the chording modifiers tell them apart. The width-flip chord is document text too, and what it types
    /// is the key under the modifier: `⌃,` arrives with `characters` `,` but
    /// `⌃[` arrives as Escape, and the bracket is what the user asked for.
    static func documentText(of key: KeyEventSnapshot) -> String? {
        if let flipped = widthFlipCharacter(key) {
            return String(flipped)
        }
        guard key.modifiers
            .intersection(.deviceIndependentFlagsMask)
            .isDisjoint(with: hostChords)
        else { return nil }
        guard !key.isNamedSpecialKey else { return nil }
        guard let characters = key.characters, !characters.isEmpty else { return nil }
        return characters.unicodeScalars.allSatisfy(isTextScalar) ? characters : nil
    }

    /// The modifier that types a punctuation key in the other width, once —
    /// the 新注音 / Microsoft IME gesture (`Ctrl+,` → `，`). Fixed, not
    /// recordable, shown read-only on the Shortcuts pane like the caret chord.
    static let widthFlipModifiers: NSEvent.ModifierFlags = [.control]

    /// The punctuation key under a width-flip chord, or nil when `key` is not
    /// one: exactly ⌃ among the four chording modifiers, ⇧ allowed since it
    /// picks the key (`⌃⇧,` is `⌃<`), and the key one `FullWidthPunctuation`
    /// maps. Read off `charactersIgnoringModifiers` because Control rewrites
    /// what some keys type (`⌃[` arrives as Escape). Which width comes out is
    /// the controller's call: the chord means "the other one", and only the
    /// controller knows which one the mode would have typed.
    static func widthFlipCharacter(_ key: KeyEventSnapshot) -> Character? {
        let chording = key.modifiers.intersection(chordingModifiers)
        guard chording.subtracting(.shift) == widthFlipModifiers else { return nil }
        guard !key.isNamedSpecialKey,
              let unmodified = key.charactersIgnoringModifiers,
              FullWidthPunctuation.mapped(unmodified) != nil
        else { return nil }
        return unmodified.first
    }

    /// True for an Escape with no host chord held — the key that closes
    /// whatever card or list this input method has up (the Telex guide, the
    /// symbol picker). `⌃3` arrives as Escape too, and is the host's. The
    /// whole event, not its first character: an Escape with more behind it
    /// is not this key (the desktop core's `is_bare_escape`).
    static func isPlainEscape(_ key: KeyEventSnapshot) -> Bool {
        key.characters == "\u{1B}" && key.modifiers.isDisjoint(with: hostChords)
    }

    /// The four chording modifiers — what a recorded chord is made of. Caps
    /// Lock, the number pad and the function flag say how a key was reached,
    /// not which key it is.
    static let chordingModifiers: NSEvent.ModifierFlags = hostChords.union(.shift)

    /// The numeric tone markers of TL and POJ, which the engine reads as ASCII
    /// digits. A full-width `５` or another script's numeral is a character the
    /// engine cannot parse, so it is document text rather than a tone.
    ///
    /// Visible to `ComposingKeyChord`, which refuses to bind a bare digit:
    /// the digits carry tone under Standard and pick candidates under Telex,
    /// so a chord may not take one away under either.
    static func isToneDigit(_ character: Character) -> Bool {
        character.isASCII && character.isNumber
    }

    /// Not a control character (Cc) and not one of AppKit's function-key
    /// scalars. A format character (Cf) is typed text, so not
    /// `CharacterSet.controlCharacters` (Cc + Cf) — the core's rule (`keys/intent.rs`).
    private static func isTextScalar(_ scalar: Unicode.Scalar) -> Bool {
        scalar.properties.generalCategory != .control && !appKitFunctionKeyRange.contains(scalar.value)
    }
}
