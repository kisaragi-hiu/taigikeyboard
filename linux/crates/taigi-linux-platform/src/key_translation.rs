//! A Linux key event as the desktop core's [`KeyEventSnapshot`] (roadmap L5).
//!
//! Both frameworks hand an engine `(keysym, keycode, state)`: the X11 keysym
//! the layout produced for the press (already shifted / Caps-Locked — `A`
//! for Shift+a), the hardware keycode, and the modifier mask. The keycode
//! differs on the wire: Fcitx5 carries the X keycode (evdev + 8), IBus the
//! evdev code itself (ibus `client/gtk2/ibusimcontext.c` sends
//! `keycode - 8`; Fcitx5's own IBus frontend adds the 8 back,
//! `ibusfrontend.cpp:397`). [`RawKeyEvent`] holds the X keycode;
//! [`RawKeyEvent::from_ibus`] converts at the IBus boundary. The Windows
//! counterpart (`taigi-windows-platform::key_translation::snapshot`) has to
//! ask the layout twice to get the characters with and without the chording
//! modifiers; here the keysym is the answer to both questions, because X
//! keeps a Control chord's keysym as the letter.
//!
//! Only presses reach the classifier: a release (`IBUS_RELEASE_MASK`) and a
//! bare modifier press answer `None`, which the engine turns into "not
//! handled" so the client processes the key itself.

use taigi_desktop_core::keys::{KeyEventSnapshot, KeyModifiers, NavigationKey};
use xkeysym::{key, Keysym};

/// The modifier bits IBus packs into `state` (ibus `src/ibustypes.h:70-97`).
pub mod state {
    pub const SHIFT: u32 = 1 << 0;
    pub const LOCK: u32 = 1 << 1;
    pub const CONTROL: u32 = 1 << 2;
    /// Mod1 — Alt on every common layout.
    pub const MOD1: u32 = 1 << 3;
    /// Mod4 — the Super / Windows key on every common layout.
    pub const MOD4: u32 = 1 << 6;
    pub const SUPER: u32 = 1 << 26;
    pub const RELEASE: u32 = 1 << 30;
}

/// X keycode = evdev code + 8, on every X server and Wayland compositor.
const EVDEV_TO_X_KEYCODE: u32 = 8;

/// A key as the framework delivers it, with the keycode in X terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKeyEvent {
    pub keyval: u32,
    /// The X keycode (Fcitx5 `Key::code()`), never the evdev code.
    pub keycode: u32,
    pub state: u32,
}

impl RawKeyEvent {
    /// The three numbers of IBus `ProcessKeyEvent`, whose keycode is the
    /// evdev code. A client that sends no keycode (0) keeps none.
    pub fn from_ibus(keyval: u32, evdev_keycode: u32, state: u32) -> Self {
        let keycode = if evdev_keycode == 0 {
            0
        } else {
            evdev_keycode + EVDEV_TO_X_KEYCODE
        };
        Self {
            keyval,
            keycode,
            state,
        }
    }
}

/// The modifier mask, decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyState {
    pub modifiers: KeyModifiers,
    pub is_release: bool,
}

impl KeyState {
    pub fn decode(state: u32) -> Self {
        Self {
            modifiers: KeyModifiers {
                shift: state & state::SHIFT != 0,
                control: state & state::CONTROL != 0,
                alt: state & state::MOD1 != 0,
                win: state & (state::MOD4 | state::SUPER) != 0,
            },
            is_release: state & state::RELEASE != 0,
        }
    }
}

/// The snapshot for a press, or `None` for a release, a bare modifier or a
/// keysym that is neither a character nor a key this input method names.
pub fn snapshot(event: RawKeyEvent) -> Option<KeyEventSnapshot> {
    let state = KeyState::decode(event.state);
    if state.is_release {
        return None;
    }
    let keysym = Keysym::new(event.keyval);
    if is_modifier_keysym(keysym) {
        return None;
    }
    let fixed = fixed_control_character(keysym);
    let characters = fixed
        .map(str::to_owned)
        .or_else(|| keysym.key_char().map(|c| c.to_string()));
    Some(KeyEventSnapshot {
        characters: characters.clone(),
        characters_ignoring_modifiers: characters,
        key_code: virtual_key_code(event.keycode),
        modifiers: state.modifiers,
        is_named_special_key: is_named_special_keysym(keysym),
        navigation_key: navigation_key(keysym),
    })
}

/// Shift, Control, Alt / Meta, Super / Hyper, the locks — a press that types
/// nothing and classifies nothing (`is_modifier_key` on Windows).
fn is_modifier_keysym(keysym: Keysym) -> bool {
    matches!(
        keysym.raw(),
        key::Shift_L
            | key::Shift_R
            | key::Control_L
            | key::Control_R
            | key::Caps_Lock
            | key::Shift_Lock
            | key::Meta_L
            | key::Meta_R
            | key::Alt_L
            | key::Alt_R
            | key::Super_L
            | key::Super_R
            | key::Hyper_L
            | key::Hyper_R
            | key::Num_Lock
            | key::Scroll_Lock
            | key::ISO_Level3_Shift
            | key::ISO_Level5_Shift
            | key::Mode_switch
    )
}

/// The control characters the Windows key sink spells these keys as
/// (`fixed_control_character`), so the shared intent table reads Return,
/// Tab, Escape, Backspace and forward Delete the same way on both desktops.
fn fixed_control_character(keysym: Keysym) -> Option<&'static str> {
    match keysym.raw() {
        key::Return | key::KP_Enter => Some("\r"),
        key::Tab | key::ISO_Left_Tab => Some("\t"),
        key::Escape => Some("\u{1B}"),
        key::BackSpace => Some("\u{8}"),
        key::Delete | key::KP_Delete => Some("\u{7F}"),
        _ => None,
    }
}

fn navigation_key(keysym: Keysym) -> Option<NavigationKey> {
    match keysym.raw() {
        key::Left | key::KP_Left => Some(NavigationKey::LeftArrow),
        key::Right | key::KP_Right => Some(NavigationKey::RightArrow),
        key::Up | key::KP_Up => Some(NavigationKey::UpArrow),
        key::Down | key::KP_Down => Some(NavigationKey::DownArrow),
        key::Page_Up | key::KP_Page_Up => Some(NavigationKey::PageUp),
        key::Page_Down | key::KP_Page_Down => Some(NavigationKey::PageDown),
        _ => None,
    }
}

/// Keys the platform names rather than types: navigation, Home / End /
/// Insert / forward Delete, the function keys (`is_named_special_key` on
/// Windows).
fn is_named_special_keysym(keysym: Keysym) -> bool {
    navigation_key(keysym).is_some()
        || matches!(
            keysym.raw(),
            key::Home
                | key::End
                | key::Insert
                | key::Delete
                | key::KP_Home
                | key::KP_End
                | key::KP_Insert
                | key::KP_Delete
                | key::Begin
                | key::Menu
                | key::Pause
                | key::Print
                | key::Sys_Req
                | key::Break
        )
        || (key::F1..=key::F35).contains(&keysym.raw())
}

/// The X keycode of the `1`…`9`, `0` row and the `;` key, as the Windows
/// virtual-key codes the shared chord and slot-key tables are written in
/// (`chord.rs` `NUMBER_ROW_KEY_CODES` / `SEMICOLON_KEY_CODE`): the tables
/// tell Shift+3 apart from a typed `#` by the KEY, which on Linux is the
/// hardware keycode — the X keycode, evdev + 8 (`KEY_1` = 2 → 10); other
/// keys carry no code, as nothing reads one.
fn virtual_key_code(keycode: u32) -> Option<u16> {
    match keycode {
        10..=18 => Some(0x31 + (keycode - 10) as u16),
        19 => Some(0x30),
        47 => Some(0xBA),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(keyval: u32, keycode: u32, state: u32) -> Option<KeyEventSnapshot> {
        snapshot(RawKeyEvent {
            keyval,
            keycode,
            state,
        })
    }

    #[test]
    fn a_letter_types_itself_with_no_modifier() {
        // trace: keysym `a` (0x61) → key_char 'a'; keycode 38 carries no
        // virtual-key code.
        let snapshot = press(key::a, 38, 0).unwrap();
        assert_eq!(snapshot.characters.as_deref(), Some("a"));
        assert_eq!(snapshot.characters_ignoring_modifiers.as_deref(), Some("a"));
        assert_eq!(snapshot.modifiers, KeyModifiers::NONE);
        assert!(!snapshot.is_named_special_key);
        assert_eq!(snapshot.key_code, None);
    }

    #[test]
    fn shift_and_caps_lock_arrive_as_the_uppercase_keysym() {
        // trace: X gives `A` for Shift+a and for Caps Lock + a; the mask
        // says which. Lock is not a modifier the core tracks.
        let shifted = press(key::A, 38, state::SHIFT).unwrap();
        assert_eq!(shifted.characters.as_deref(), Some("A"));
        assert_eq!(shifted.modifiers, KeyModifiers::SHIFT);
        let locked = press(key::A, 38, state::LOCK).unwrap();
        assert_eq!(locked.characters.as_deref(), Some("A"));
        assert_eq!(locked.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn control_alt_super_map_to_the_core_modifiers() {
        let snapshot = press(key::s, 39, state::CONTROL | state::MOD1).unwrap();
        assert_eq!(
            snapshot.modifiers,
            KeyModifiers {
                shift: false,
                control: true,
                alt: true,
                win: false
            }
        );
        // The chord's keysym stays the letter — both character fields read `s`.
        assert_eq!(snapshot.characters.as_deref(), Some("s"));
        assert_eq!(snapshot.characters_ignoring_modifiers.as_deref(), Some("s"));
        assert!(press(key::s, 39, state::SUPER).unwrap().modifiers.win);
        assert!(press(key::s, 39, state::MOD4).unwrap().modifiers.win);
    }

    #[test]
    fn releases_and_bare_modifiers_are_not_events() {
        assert_eq!(press(key::a, 38, state::RELEASE), None);
        assert_eq!(press(key::Shift_L, 50, 0), None);
        assert_eq!(press(key::Control_L, 37, 0), None);
        assert_eq!(press(key::Caps_Lock, 66, 0), None);
    }

    #[test]
    fn the_fixed_control_characters_match_the_windows_spelling() {
        // trace: `taigi-windows-platform::key_translation::fixed_control_character`.
        assert_eq!(
            press(key::Return, 36, 0).unwrap().characters.as_deref(),
            Some("\r")
        );
        assert_eq!(
            press(key::KP_Enter, 104, 0).unwrap().characters.as_deref(),
            Some("\r")
        );
        assert_eq!(
            press(key::Tab, 23, 0).unwrap().characters.as_deref(),
            Some("\t")
        );
        let escape = press(key::Escape, 9, 0).unwrap();
        assert_eq!(escape.characters.as_deref(), Some("\u{1B}"));
        assert!(escape.is_bare_escape());
        assert_eq!(
            press(key::BackSpace, 22, 0).unwrap().characters.as_deref(),
            Some("\u{8}")
        );
        let delete = press(key::Delete, 119, 0).unwrap();
        assert_eq!(delete.characters.as_deref(), Some("\u{7F}"));
        assert!(delete.is_named_special_key);
    }

    #[test]
    fn navigation_keys_are_named_and_carry_no_characters() {
        let left = press(key::Left, 113, 0).unwrap();
        assert_eq!(left.navigation_key, Some(NavigationKey::LeftArrow));
        assert!(left.is_named_special_key);
        assert_eq!(left.characters, None);
        assert_eq!(
            press(key::Page_Down, 117, 0).unwrap().navigation_key,
            Some(NavigationKey::PageDown)
        );
        assert_eq!(
            press(key::KP_Up, 80, 0).unwrap().navigation_key,
            Some(NavigationKey::UpArrow)
        );
        let home = press(key::Home, 110, 0).unwrap();
        assert!(home.is_named_special_key);
        assert_eq!(home.navigation_key, None);
        assert!(press(key::F5, 71, 0).unwrap().is_named_special_key);
    }

    #[test]
    fn the_number_row_and_semicolon_carry_the_windows_virtual_key_codes() {
        // trace: X keycode 12 = evdev KEY_3 (4) + 8 → VK `3` = 0x33, so the
        // slot-key table can read Shift+3 as slot 2 under Digits.
        let hash = press(key::numbersign, 12, state::SHIFT).unwrap();
        assert_eq!(hash.characters.as_deref(), Some("#"));
        assert_eq!(hash.key_code, Some(0x33));
        assert_eq!(press(key::_0, 19, 0).unwrap().key_code, Some(0x30));
        assert_eq!(
            press(key::colon, 47, state::SHIFT).unwrap().key_code,
            Some(0xBA)
        );
        assert_eq!(press(key::q, 24, 0).unwrap().key_code, None);
    }

    #[test]
    fn an_ibus_keycode_is_the_evdev_code_and_gains_eight() {
        // trace: IBus Shift+3 arrives as evdev KEY_3 = 4 → X keycode 12 →
        // VK `3`; evdev KEY_V = 47 must not read as the `;` key (X 47).
        let hash = snapshot(RawKeyEvent::from_ibus(key::numbersign, 4, state::SHIFT)).unwrap();
        assert_eq!(hash.key_code, Some(0x33));
        let v = snapshot(RawKeyEvent::from_ibus(key::V, 47, state::SHIFT)).unwrap();
        assert_eq!(v.key_code, None);
        let colon = snapshot(RawKeyEvent::from_ibus(key::colon, 39, state::SHIFT)).unwrap();
        assert_eq!(colon.key_code, Some(0xBA));
        assert_eq!(RawKeyEvent::from_ibus(key::a, 0, 0).keycode, 0);
    }

    #[test]
    fn space_and_punctuation_type_their_character() {
        assert_eq!(
            press(key::space, 65, 0).unwrap().characters.as_deref(),
            Some(" ")
        );
        assert_eq!(
            press(key::comma, 59, 0).unwrap().characters.as_deref(),
            Some(",")
        );
        assert_eq!(
            press(key::grave, 49, 0).unwrap().characters.as_deref(),
            Some("`")
        );
    }
}
