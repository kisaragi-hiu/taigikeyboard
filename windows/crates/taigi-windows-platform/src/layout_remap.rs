//! Dvorak / Colemak on the US base layout the input method always runs on.
//!
//! The TIP is registered under zh-TW, so while it is active the thread's
//! layout is the zh-TW default — KBDUS — whatever layout the user had before
//! (measured on the box 2026-10-07: HKL 0x04040404, physical K A G read as
//! `kag` where Dvorak types `tai`). The key translation therefore reads each
//! key through these tables first: the virtual key of the pressed US
//! position becomes the US virtual key of the character the chosen layout
//! types there. Every Dvorak and Colemak key pair (unshifted / Shift) is a
//! US key pair, so `ToUnicodeEx` on the mapped key gives both characters.
//! Only the main block moves: the number row's digits, the keypad, the
//! navigation keys and the ISO key (`VK_OEM_102`) are never mapped.
//! Colemak's Caps Lock-as-Backspace is not a character and is not taken.

use taigi_desktop_core::keys::SEMICOLON_KEY_CODE as VK_OEM_1;
use taigi_desktop_core::settings::KeyboardLayout;

const VK_OEM_PLUS: u16 = 0xBB; // =
const VK_OEM_COMMA: u16 = 0xBC; // ,
const VK_OEM_MINUS: u16 = 0xBD; // -
const VK_OEM_PERIOD: u16 = 0xBE; // .
const VK_OEM_2: u16 = 0xBF; // /
const VK_OEM_4: u16 = 0xDB; // [
const VK_OEM_6: u16 = 0xDD; // ]
const VK_OEM_7: u16 = 0xDE; // '

/// `(pressed US position, US key of the character typed there)` for every
/// key Dvorak moves — CLDR's US-Dvorak chart, row by row.
const DVORAK: &[(u16, u16)] = &[
    // - =  →  [ ]
    (VK_OEM_MINUS, VK_OEM_4),
    (VK_OEM_PLUS, VK_OEM_6),
    // q w e r t y u i o p [ ]  →  ' , . p y f g c r l / =
    (b'Q' as u16, VK_OEM_7),
    (b'W' as u16, VK_OEM_COMMA),
    (b'E' as u16, VK_OEM_PERIOD),
    (b'R' as u16, b'P' as u16),
    (b'T' as u16, b'Y' as u16),
    (b'Y' as u16, b'F' as u16),
    (b'U' as u16, b'G' as u16),
    (b'I' as u16, b'C' as u16),
    (b'O' as u16, b'R' as u16),
    (b'P' as u16, b'L' as u16),
    (VK_OEM_4, VK_OEM_2),
    (VK_OEM_6, VK_OEM_PLUS),
    // a s d f g h j k l ; '  →  a o e u i d h t n s -
    (b'S' as u16, b'O' as u16),
    (b'D' as u16, b'E' as u16),
    (b'F' as u16, b'U' as u16),
    (b'G' as u16, b'I' as u16),
    (b'H' as u16, b'D' as u16),
    (b'J' as u16, b'H' as u16),
    (b'K' as u16, b'T' as u16),
    (b'L' as u16, b'N' as u16),
    (VK_OEM_1, b'S' as u16),
    (VK_OEM_7, VK_OEM_MINUS),
    // z x c v b n m , . /  →  ; q j k x b m w v z
    (b'Z' as u16, VK_OEM_1),
    (b'X' as u16, b'Q' as u16),
    (b'C' as u16, b'J' as u16),
    (b'V' as u16, b'K' as u16),
    (b'B' as u16, b'X' as u16),
    (b'N' as u16, b'B' as u16),
    (VK_OEM_COMMA, b'W' as u16),
    (VK_OEM_PERIOD, b'V' as u16),
    (VK_OEM_2, b'Z' as u16),
];

/// The same for Colemak — CLDR's US-Colemak chart.
const COLEMAK: &[(u16, u16)] = &[
    // q w e r t y u i o p  →  q w f p g j l u y ;
    (b'E' as u16, b'F' as u16),
    (b'R' as u16, b'P' as u16),
    (b'T' as u16, b'G' as u16),
    (b'Y' as u16, b'J' as u16),
    (b'U' as u16, b'L' as u16),
    (b'I' as u16, b'U' as u16),
    (b'O' as u16, b'Y' as u16),
    (b'P' as u16, VK_OEM_1),
    // a s d f g h j k l ;  →  a r s t d h n e i o
    (b'S' as u16, b'R' as u16),
    (b'D' as u16, b'S' as u16),
    (b'F' as u16, b'T' as u16),
    (b'G' as u16, b'D' as u16),
    (b'J' as u16, b'N' as u16),
    (b'K' as u16, b'E' as u16),
    (b'L' as u16, b'I' as u16),
    (VK_OEM_1, b'O' as u16),
    // z x c v b n m  →  z x c v b k m
    (b'N' as u16, b'K' as u16),
];

fn table(layout: KeyboardLayout) -> &'static [(u16, u16)] {
    match layout {
        KeyboardLayout::Qwerty => &[],
        KeyboardLayout::Dvorak => DVORAK,
        KeyboardLayout::Colemak => COLEMAK,
    }
}

/// The US virtual key of what `layout` types at the US position
/// `virtual_key`; `virtual_key` itself for a key the layout does not move.
pub fn typed_virtual_key(layout: KeyboardLayout, virtual_key: u16) -> u16 {
    table(layout)
        .iter()
        .find(|(position, _)| *position == virtual_key)
        .map_or(virtual_key, |(_, typed)| *typed)
}

/// The inverse: the US position where `layout` types the character of the
/// US key `typed` — where a global chord stored by its character sits.
pub fn position_virtual_key(layout: KeyboardLayout, typed: u16) -> u16 {
    table(layout)
        .iter()
        .find(|(_, key)| *key == typed)
        .map_or(typed, |(position, _)| *position)
}

/// Whether `hkl` is a layout these tables are written over: the zh-TW or
/// en-US default layout (device handle 0x0404 / 0x0409 in the high word),
/// both KBDUS. A real Dvorak, Colemak, US-International or other-language
/// layout has another device handle and is read as it is — the remap would
/// map it twice.
pub fn is_us_base_layout(hkl: usize) -> bool {
    matches!((hkl >> 16) & 0xFFFF, 0x0404 | 0x0409)
}

#[cfg(test)]
mod tests {
    use super::*;
    use taigi_desktop_core::settings::SettingChoice;

    fn typed(layout: KeyboardLayout, keys: &str) -> String {
        keys.bytes()
            .map(|key| typed_virtual_key(layout, u16::from(key)) as u8 as char)
            .collect()
    }

    #[test]
    fn dvorak_and_colemak_type_tai_where_the_box_measured_it() {
        // trace: box 2026-10-07, physical K A G under the Dvorak HKL typed
        // `tai`; Colemak types t a i at F A L (CLDR home row a r s t d h n e i o).
        assert_eq!(typed(KeyboardLayout::Dvorak, "KAG"), "TAI");
        assert_eq!(typed(KeyboardLayout::Colemak, "FAL"), "TAI");
        assert_eq!(typed(KeyboardLayout::Qwerty, "KAG"), "KAG");
    }

    #[test]
    fn every_letter_row_follows_the_chart() {
        // trace: CLDR US-Dvorak / US-Colemak letter rows, pressed on the US
        // positions qwertyuiop / asdfghjkl / zxcvbnm.
        assert_eq!(typed(KeyboardLayout::Dvorak, "RTYUIOP"), "PYFGCRL");
        assert_eq!(typed(KeyboardLayout::Dvorak, "ASDFGHJKL"), "AOEUIDHTN");
        assert_eq!(typed(KeyboardLayout::Dvorak, "XCVBNM"), "QJKXBM");
        assert_eq!(typed(KeyboardLayout::Colemak, "QWERTYUIO"), "QWFPGJLUY");
        assert_eq!(typed(KeyboardLayout::Colemak, "ASDFGHJKL"), "ARSTDHNEI");
        assert_eq!(typed(KeyboardLayout::Colemak, "ZXCVBNM"), "ZXCVBKM");
    }

    #[test]
    fn punctuation_moves_with_the_layout() {
        let dvorak = |key| typed_virtual_key(KeyboardLayout::Dvorak, key);
        // ' , . on the top row; - = become [ ]; [ ] become / =.
        assert_eq!(dvorak(u16::from(b'Q')), VK_OEM_7);
        assert_eq!(dvorak(u16::from(b'W')), VK_OEM_COMMA);
        assert_eq!(dvorak(u16::from(b'E')), VK_OEM_PERIOD);
        assert_eq!(dvorak(VK_OEM_MINUS), VK_OEM_4);
        assert_eq!(dvorak(VK_OEM_PLUS), VK_OEM_6);
        assert_eq!(dvorak(VK_OEM_4), VK_OEM_2);
        assert_eq!(dvorak(VK_OEM_6), VK_OEM_PLUS);
        assert_eq!(dvorak(VK_OEM_7), VK_OEM_MINUS);
        assert_eq!(dvorak(VK_OEM_COMMA), u16::from(b'W'));
    }

    #[test]
    fn the_semicolon_slot_key_sits_where_each_layout_types_it() {
        // trace: Dvorak `;` is the bottom-left letter key (US Z); Colemak `;`
        // is the top-right letter key (US P) — the ninth slot key follows it.
        assert_eq!(
            typed_virtual_key(KeyboardLayout::Dvorak, u16::from(b'Z')),
            VK_OEM_1
        );
        assert_eq!(
            typed_virtual_key(KeyboardLayout::Colemak, u16::from(b'P')),
            VK_OEM_1
        );
        assert_eq!(
            position_virtual_key(KeyboardLayout::Dvorak, VK_OEM_1),
            u16::from(b'Z')
        );
        assert_eq!(
            position_virtual_key(KeyboardLayout::Colemak, VK_OEM_1),
            u16::from(b'P')
        );
    }

    #[test]
    fn each_table_is_a_permutation_and_its_inverse_undoes_it() {
        for layout in [KeyboardLayout::Dvorak, KeyboardLayout::Colemak] {
            let mut positions: Vec<u16> = table(layout).iter().map(|(p, _)| *p).collect();
            let mut typed_keys: Vec<u16> = table(layout).iter().map(|(_, t)| *t).collect();
            positions.sort_unstable();
            typed_keys.sort_unstable();
            positions.dedup();
            assert_eq!(
                positions.len(),
                table(layout).len(),
                "{layout:?}: a position twice"
            );
            assert_eq!(positions, typed_keys, "{layout:?}: not a permutation");
            for &(position, _) in table(layout) {
                let typed = typed_virtual_key(layout, position);
                assert_eq!(position_virtual_key(layout, typed), position);
            }
        }
    }

    #[test]
    fn digits_keypad_and_named_keys_never_move() {
        // 0x30–0x39 number row, 0x60–0x6F keypad (VK_DIVIDE 0x6F shares its
        // scan code with `/`), Return, Space, Backspace, the ISO key.
        let fixed = (0x30..=0x39)
            .chain(0x60..=0x6F)
            .chain([0x0D, 0x20, 0x08, 0xE2, 0xC0, 0xDC]);
        for key in fixed {
            for layout in KeyboardLayout::ALL {
                assert_eq!(
                    typed_virtual_key(*layout, key),
                    key,
                    "{layout:?} moved {key:#x}"
                );
            }
        }
    }

    #[test]
    fn only_the_two_kbdus_defaults_count_as_the_base() {
        assert!(is_us_base_layout(0x0404_0404));
        assert!(is_us_base_layout(0x0409_0409));
        // Measured on the box: the Dvorak HKL. Colemak / US-International are
        // device handles 0xF0D3 / 0xF001 the same way; Japanese is 0x0411.
        assert!(!is_us_base_layout(0xF002_0409));
        assert!(!is_us_base_layout(0xF0D3_0409));
        assert!(!is_us_base_layout(0xF001_0409));
        assert!(!is_us_base_layout(0x0411_0411));
    }
}
