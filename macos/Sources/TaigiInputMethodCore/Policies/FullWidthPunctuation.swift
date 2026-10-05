// Maps typed half-width punctuation to its full-width form for hanji-first output.

import Foundation

/// The full-width punctuation policy: which typed characters become full-width,
/// and when the mapping applies at all.
///
/// Applied only while the Hanji/romanization swap has Hanji coming first, mirroring the
/// MOE input method's rule (full-width in Hanji mode, half-width in TL mode): romanized output reads
/// as Latin text and keeps Latin punctuation, Hanji output reads as CJK text
/// and gets CJK punctuation. That mode IS the whole gate — there is no
/// setting beside it (USER 2026-08-24) — and it was written as the exact
/// complement of the auto-space gate: auto-space served the
/// roman-first mode, this mapping the hanji-first one.
///
/// Since the Hanji/romanization key (2026-08-25) they can meet. Space commits a romanization
/// while the settings lead with hanji, which earns a trailing space in the very
/// mode this mapping serves, so a following `?` matches both rules. The
/// auto-space swap is read first and wins: the word in front of the caret is
/// romanization, and romanization keeps Latin punctuation whatever the mode
/// would say about a hanji word (the key path reads the swap before the map).
/// The caller reads the mode (`documentPunctuation(_:isWidthFlip:settings:)`),
/// and the mode is a DEFAULT, not a wall: ⌃ on any key of this map types the
/// other width once (`KeyEventSnapshot.widthFlipCharacter`) — the 新注音 /
/// Microsoft IME gesture, made symmetric because the case that hurt was a
/// half-width comma inside hanji-first text (USER 2026-09-20).
enum FullWidthPunctuation {
    /// The MOE manual's symbol shortcut table (符號快捷鍵對照表), minus what this input method must
    /// keep half-width: digits are TL/POJ tone markers, the hyphen is the
    /// syllable separator, letters spell the romanization, and the straight
    /// double quote serves as both opening and closing so a one-to-one map
    /// cannot pick a side (the same reason the engine's auto-space attaching set excludes it).
    private static let map: [Character: String] = [
        ",": "，", ".": "。", "?": "？", "!": "！",
        ";": "；", ":": "：",
        "(": "（", ")": "）",
        "[": "「", "]": "」",
        "{": "『", "}": "』",
        "<": "《", ">": "》",
        "'": "、",
        // The shifted number row (`⇧2`…`⇧8`, `⇧-`, `⇧=`): symbols the MOE table
        // also writes full-width. `!` `(` `)` above complete the row; `~` (⇧ on
        // the backtick key) is its first key (user report 2026-09-28).
        "~": "～", "@": "＠", "#": "＃", "$": "＄", "%": "％", "^": "＾", "&": "＆", "*": "＊",
        "_": "＿", "+": "＋",
    ]

    /// Mapped keys whose ⌃ chord stays the host's instead of flipping the
    /// width: ⌃⇧` (`~`) is VS Code's New Terminal on every desktop.
    private static let hostChordKeys: Set<Character> = ["~"]

    /// Whether ⌃ on the key that typed `text` flips the punctuation width: a
    /// mapped key outside `hostChordKeys` (the desktop core's
    /// `is_width_flip_key`).
    static func isWidthFlipKey(_ text: String) -> Bool {
        mapped(text) != nil && !text.contains(where: hostChordKeys.contains)
    }

    /// The full-width form of one typed character, or nil when the key is not
    /// punctuation this policy maps. Multi-character strings are never mapped:
    /// a key event carries one typed character, and anything longer came from
    /// somewhere this policy has no business rewriting.
    static func mapped(_ text: String) -> String? {
        guard text.count == 1, let character = text.first else { return nil }
        return map[character]
    }

    /// The punctuation the input method writes for `text`, or nil when the
    /// host should write it: the full-width form when the mode types
    /// full-width marks, and under the width-flip chord the OTHER width. A
    /// flipped key is never nil — the host would read the chord as a
    /// shortcut, so even its half-width form is the input method's to write.
    static func documentPunctuation(_ text: String, isFullWidthMode: Bool, isWidthFlip: Bool) -> String? {
        if isFullWidthMode != isWidthFlip {
            return mapped(text)
        }
        return isWidthFlip ? text : nil
    }

    /// The same under the mode `settings` holds NOW — read live, like the
    /// auto-space gate, so a swap applies to the very next key. `isWidthFlip`
    /// is `KeyEventSnapshot.widthFlipCharacter`'s verdict on the key that
    /// typed `text`. The core back end asks it before a swap's client check
    /// (`ComposingBackend.swapCandidate`).
    ///
    /// The EFFECTIVE width (`SettingsStore.isFullWidthPunctuation`), not the
    /// stored swap: a romanization-only display writes romanization, and
    /// romanization takes half-width marks; under Hanji with Romanization the
    /// stored swap still picks the width even though the candidate projection
    /// is forced hanji-first.
    @MainActor
    static func documentPunctuation(_ text: String, isWidthFlip: Bool, settings: SettingsStore) -> String? {
        documentPunctuation(text, isFullWidthMode: settings.isFullWidthPunctuation, isWidthFlip: isWidthFlip)
    }
}
