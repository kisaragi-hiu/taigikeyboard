//! An `NSEvent`'s fields, as Swift sends them (`KeyEvent`), turned into the
//! core's `KeyEventSnapshot` (docs/architecture/macos-desktop-core-roadmap.md
//! D3) — the `taigi-linux-platform` `key_translation` precedent, tested
//! without an event. The constants are AppKit's and Carbon's, checked with a
//! `swift` probe against the macOS 27 SDK.

use taigi_desktop_core::keys::{KeyEventSnapshot, KeyModifiers, NavigationKey, RecordedPress};

use crate::proto::KeyEvent;

/// `NSEvent.ModifierFlags` bits, by role (`KeyModifiers`): ⌘ is the host
/// chord the core calls `win`, ⌥ its `alt`. Caps Lock, the numeric pad,
/// Help and Fn say how a key was reached, not which key it is, and are
/// dropped — as `KeyEventSnapshot.chordingModifiers` drops them.
pub(crate) const SHIFT: u64 = 1 << 17;
pub(crate) const CONTROL: u64 = 1 << 18;
pub(crate) const OPTION: u64 = 1 << 19;
pub(crate) const COMMAND: u64 = 1 << 20;

/// Carbon's ANSI key codes (`kVK_ANSI_*`, positions on the keyboard) for the
/// ten keys the core reads a code for — `1`…`9` and `;` (`chord.rs`
/// `NUMBER_ROW_KEY_CODES` / `SEMICOLON_KEY_CODE`) — each with the code the
/// core spells it with (the Windows virtual key). `6` and `9` sit out of
/// numeric order on Carbon's side.
const KEY_CODES: [(u32, u16); 10] = [
    (0x12, 0x31), // 1
    (0x13, 0x32), // 2
    (0x14, 0x33), // 3
    (0x15, 0x34), // 4
    (0x17, 0x35), // 5
    (0x16, 0x36), // 6
    (0x1A, 0x37), // 7
    (0x1C, 0x38), // 8
    (0x19, 0x39), // 9
    (0x29, 0xBA), // ;
];

/// The snapshot the classifier reads off `event`. `special_key` is
/// `NSEvent.specialKey?.rawValue`: any value makes a named special key
/// (Return and Delete included, as AppKit names them), and six of them are
/// the navigation keys (`NavigationKey.init(NSEvent.SpecialKey)`).
pub(crate) fn snapshot(event: &KeyEvent) -> KeyEventSnapshot {
    KeyEventSnapshot {
        characters: event.characters.clone(),
        characters_ignoring_modifiers: event.characters_ignoring_modifiers.clone(),
        key_code: event.key_code.and_then(core_key_code),
        modifiers: modifiers(event.modifier_flags),
        is_named_special_key: event.special_key.is_some(),
        navigation_key: event.special_key.and_then(navigation_key),
    }
}

/// `kVK_Escape` — the Escape KEY, which the recorder leaves on whatever the
/// layout types for it (`key_rules.rs` `press` reads the key code).
pub(crate) const ESCAPE_KEY_CODE: u32 = 0x35;

/// `NSDeleteFunctionKey`, the forward Delete (⌦).
const FORWARD_DELETE: u32 = 0xF728;

/// The press a shortcut-recording field hands `evaluate_press`: what the
/// key types with no modifier held, the chording modifiers and the core's
/// key code. A repeat never gets here — Swift drops it first.
///
/// A bare forward Delete is read as Delete (`\u{7F}`), so the core blanks
/// the field as the Swift recorder does for `.deleteForward`; with a
/// modifier it stays `F728` and records (⌃⌦, ⇧⌦). Only here: on the key
/// path ⌦ stays what it is — read as `\u{7F}` it would delete backward
/// (roadmap D3).
pub(crate) fn recorded_press(event: &KeyEvent) -> RecordedPress {
    let modifiers = modifiers(event.modifier_flags);
    let key = if modifiers.is_empty() && event.special_key == Some(FORWARD_DELETE) {
        Some("\u{7F}".to_owned())
    } else {
        event
            .characters_ignoring_modifiers
            .clone()
            .or_else(|| event.characters.clone())
    };
    RecordedPress {
        key,
        modifiers,
        key_code: event.key_code.and_then(core_key_code),
        is_repeat: false,
    }
}

pub(crate) fn modifiers(flags: u64) -> KeyModifiers {
    KeyModifiers {
        shift: flags & SHIFT != 0,
        control: flags & CONTROL != 0,
        alt: flags & OPTION != 0,
        win: flags & COMMAND != 0,
    }
}

/// The core's code for a Carbon key code, or `None` for a key the core
/// reads no code for.
fn core_key_code(carbon: u32) -> Option<u16> {
    KEY_CODES
        .iter()
        .find(|(mac, _)| *mac == carbon)
        .map(|(_, core)| *core)
}

/// `NSUpArrowFunctionKey` … `NSPageDownFunctionKey`.
fn navigation_key(special_key: u32) -> Option<NavigationKey> {
    match special_key {
        0xF700 => Some(NavigationKey::UpArrow),
        0xF701 => Some(NavigationKey::DownArrow),
        0xF702 => Some(NavigationKey::LeftArrow),
        0xF703 => Some(NavigationKey::RightArrow),
        0xF72C => Some(NavigationKey::PageUp),
        0xF72D => Some(NavigationKey::PageDown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::key_event;

    fn event(characters: &str, modifier_flags: u64) -> KeyEvent {
        key_event(characters, modifier_flags, None)
    }

    #[test]
    fn modifiers_map_by_role_and_the_rest_are_dropped() {
        // trace: NSEvent.ModifierFlags — capsLock 0x10000, shift 0x20000,
        // control 0x40000, option 0x80000, command 0x100000, numericPad
        // 0x200000, help 0x400000, function 0x800000.
        let cases = [
            (0x20000, KeyModifiers::SHIFT),
            (0x40000, KeyModifiers::CONTROL),
            (0x80000, KeyModifiers::ALT),
            (0x100000, KeyModifiers::WIN),
            (0x10000 | 0x200000 | 0x400000 | 0x800000, KeyModifiers::NONE),
            (
                0x1E0000 | 0x800000,
                KeyModifiers::SHIFT
                    .with(KeyModifiers::CONTROL)
                    .with(KeyModifiers::ALT)
                    .with(KeyModifiers::WIN),
            ),
        ];
        for (flags, expected) in cases {
            assert_eq!(
                snapshot(&event("a", flags)).modifiers,
                expected,
                "{flags:#x}"
            );
        }
    }

    #[test]
    fn the_number_row_and_semicolon_reach_the_core_as_its_codes() {
        // trace: kVK_ANSI_1…9 = 12 13 14 15 17 16 1A 1C 19, kVK_ANSI_Semicolon
        // = 29 (Carbon HIToolbox); core NUMBER_ROW_KEY_CODES = 0x31…0x39,
        // SEMICOLON_KEY_CODE = 0xBA.
        let carbon = [0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1A, 0x1C, 0x19];
        for (index, code) in carbon.into_iter().enumerate() {
            assert_eq!(
                core_key_code(code),
                Some(0x31 + index as u16),
                "digit {}",
                index + 1
            );
        }
        assert_eq!(core_key_code(0x29), Some(0xBA));
        // kVK_ANSI_0, kVK_ANSI_A, kVK_Return, kVK_ANSI_Quote: no code the core reads.
        for other in [0x1D, 0x00, 0x24, 0x27] {
            assert_eq!(core_key_code(other), None, "{other:#x}");
        }
    }

    /// ⇧3 on a US layout: `#` in both character fields, the `3` key's code
    /// still saying which key it was.
    #[test]
    fn a_shifted_digit_keeps_its_key_code() {
        let mut shifted_three = event("#", 0x20000);
        shifted_three.key_code = Some(0x14);
        assert_eq!(
            snapshot(&shifted_three),
            KeyEventSnapshot::chord(Some("#"), "#", KeyModifiers::SHIFT).with_key_code(0x33)
        );
    }

    #[test]
    fn special_keys_are_named_and_six_of_them_navigate() {
        // trace: NSEvent.SpecialKey — up F700, down F701, left F702, right
        // F703, pageUp F72C, pageDown F72D; carriageReturn 0xD, delete 0x7F,
        // deleteForward F728, home F729.
        let navigation = [
            (0xF700, NavigationKey::UpArrow),
            (0xF701, NavigationKey::DownArrow),
            (0xF702, NavigationKey::LeftArrow),
            (0xF703, NavigationKey::RightArrow),
            (0xF72C, NavigationKey::PageUp),
            (0xF72D, NavigationKey::PageDown),
        ];
        for (special_key, key) in navigation {
            let mut arrow = event(&char::from_u32(special_key).unwrap().to_string(), 0);
            arrow.special_key = Some(special_key);
            let snapshot = snapshot(&arrow);
            assert!(snapshot.is_named_special_key);
            assert_eq!(snapshot.navigation_key, Some(key), "{special_key:#x}");
        }
        for (characters, special_key) in [
            ("\r", 0xD),
            ("\u{7F}", 0x7F),
            ("\u{F728}", 0xF728),
            ("\u{F729}", 0xF729),
        ] {
            let mut named = event(characters, 0);
            named.special_key = Some(special_key);
            let snapshot = snapshot(&named);
            assert!(snapshot.is_named_special_key, "{special_key:#x}");
            assert_eq!(snapshot.navigation_key, None, "{special_key:#x}");
            assert_eq!(snapshot.characters.as_deref(), Some(characters));
        }
        let plain = snapshot(&event("a", 0));
        assert!(!plain.is_named_special_key);
        assert_eq!(plain.navigation_key, None);
    }

    /// A field AppKit leaves nil stays absent: a dead key's first press
    /// carries no characters.
    #[test]
    fn absent_fields_stay_absent() {
        let empty = KeyEvent {
            key_code: None,
            characters: None,
            characters_ignoring_modifiers: None,
            modifier_flags: 0,
            special_key: None,
        };
        assert_eq!(snapshot(&empty), KeyEventSnapshot::default());
    }

    /// `charactersIgnoringModifiers` is carried as AppKit gave it — absent
    /// or empty — and the core falls back to `characters` where Swift's
    /// snapshot did (`?? characters`, `unmodified_characters`).
    #[test]
    fn the_unmodified_characters_keep_the_swift_fallback() {
        let mut without = event("a", 0);
        without.characters_ignoring_modifiers = None;
        assert_eq!(snapshot(&without).unmodified_characters(), Some("a"));
        let mut empty = event("a", 0);
        empty.characters_ignoring_modifiers = Some(String::new());
        assert_eq!(snapshot(&empty).unmodified_characters(), Some(""));
    }
}
