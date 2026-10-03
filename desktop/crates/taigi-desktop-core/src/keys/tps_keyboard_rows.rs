//! The on-screen TPS key panel's rows (desktop TPS roadmap D6): the four
//! main-block rows of a US keyboard, each key cap carrying the glyph it
//! types under TPS and its Shift-layer glyph. Built from the layout table
//! (`tps_layout.rs`), so the panel cannot drift from what the keys type. The
//! windows that draw it — `ui/tps_keyboard.rs` (Windows),
//! `TpsKeyboardPanel.swift` (macOS, through `taigi-macos-ffi`),
//! `tps_keyboard.rs` in the Linux settings app — only lay these out.

use super::tps_layout::glyph_for_key;

/// One key cap: the key's own label, what it types bare, and what it types
/// with Shift (`None` where the Shift layer is not a TPS key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TpsKeyCap {
    /// The key as its cap is printed: the capital for a letter, the bare
    /// character for a digit or punctuation key.
    pub label: char,
    pub glyph: &'static str,
    pub shift_glyph: Option<&'static str>,
}

impl TpsKeyCap {
    /// What a click on the cap types: the Shift glyph when the click asks
    /// for the Shift layer (its top half, or a held Shift) and the key has
    /// one, the bare glyph otherwise — as the cap is drawn, Shift glyph top
    /// right.
    pub fn pressed_glyph(&self, is_shift_layer: bool) -> &'static str {
        match self.shift_glyph {
            Some(shift_glyph) if is_shift_layer => shift_glyph,
            _ => self.glyph,
        }
    }
}

/// One row of caps and how far it starts from the left edge, in key widths
/// — the stagger of a physical keyboard, so the panel reads like the keys
/// under the user's hands.
#[derive(Clone, Debug, PartialEq)]
pub struct TpsKeyboardRow {
    pub indent: f32,
    pub caps: Vec<TpsKeyCap>,
}

/// Each row as `(base, shifted)` characters on a US layout, with its indent:
/// the ANSI stagger (Tab 1.5, Caps Lock 1.75, Shift 2.25 key widths) less the
/// backtick key the number row starts with, which types no glyph. Every key
/// here types a TPS glyph bare (`every_bare_letter_digit_and_layout_punctuation_key_types_a_glyph`).
const ROWS: [(f32, &str, &str); 4] = [
    (0.0, "1234567890-=", "!@#$%^&*()_+"),
    (0.5, "qwertyuiop", "QWERTYUIOP"),
    (0.75, "asdfghjkl;'", "ASDFGHJKL:\""),
    (1.25, "zxcvbnm,./", "ZXCVBNM<>?"),
];

/// The panel's four rows, top to bottom.
pub fn tps_keyboard_rows() -> Vec<TpsKeyboardRow> {
    ROWS.iter()
        .map(|(indent, base, shifted)| TpsKeyboardRow {
            indent: *indent,
            caps: base
                .chars()
                .zip(shifted.chars())
                .filter_map(|(base, shifted)| {
                    Some(TpsKeyCap {
                        label: base.to_ascii_uppercase(),
                        glyph: glyph_for_key(base)?,
                        shift_glyph: glyph_for_key(shifted),
                    })
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::tps_layout::assigned_keys;

    fn cap(label: char) -> TpsKeyCap {
        tps_keyboard_rows()
            .into_iter()
            .flat_map(|row| row.caps)
            .find(|cap| cap.label == label)
            .expect("a cap with that label")
    }

    #[test]
    fn every_key_of_the_four_rows_is_a_cap() {
        // trace: ROWS — 12 + 10 + 11 + 10 keys, each typing a glyph bare
        // (roadmap D2), so none is filtered out.
        let lengths: Vec<usize> = tps_keyboard_rows()
            .iter()
            .map(|row| row.caps.len())
            .collect();
        assert_eq!(lengths, [12, 10, 11, 10]);
    }

    #[test]
    fn every_key_the_layout_assigns_is_on_the_panel_once() {
        // The panel is the layout drawn: the typed characters behind its caps
        // — a base key, and its shifted twin where that types a glyph — are
        // exactly the table's keys, each once. A key added to the table with
        // no cap, or a cap reading a key twice, fails here.
        let mut shown: Vec<char> = ROWS
            .iter()
            .flat_map(|(_, base, shifted)| base.chars().zip(shifted.chars()))
            .flat_map(|(base, shifted)| {
                [base, shifted]
                    .into_iter()
                    .filter(|key| glyph_for_key(*key).is_some())
            })
            .collect();
        let mut assigned: Vec<char> = assigned_keys().collect();
        shown.sort_unstable();
        assigned.sort_unstable();
        assert_eq!(shown, assigned);
    }

    #[test]
    fn the_shift_layers_with_no_glyph_are_the_ones_the_layout_leaves_out() {
        // trace: KEYS, row by row — no `@ # $ % & ) _ +` (digits 2 3 4 5 7 0
        // and `-` `=`), no capital for Q W T P / A S F G H / Z X C V B N M,
        // no `"` (on `'`) and no `?` (on `/`); `!` `^` `*` `(` `:` `<` `>`
        // and the other capitals type glyphs.
        let bare: String = tps_keyboard_rows()
            .iter()
            .flat_map(|row| row.caps.iter())
            .filter(|cap| cap.shift_glyph.is_none())
            .map(|cap| cap.label)
            .collect();
        assert_eq!(bare, "234570-=QWTPASFGH'ZXCVBNM/");
    }

    #[test]
    fn a_cap_carries_its_label_and_both_layers() {
        // trace: KEYS — `e` ㄍ / `E` ㆣ; `1` ㄅ / `!` ㆠ; `6` ˊ / `^` ˆ;
        // `q` ㄆ with no Shift glyph; `'` types the hyphen, `"` nothing.
        assert_eq!(
            cap('E'),
            TpsKeyCap {
                label: 'E',
                glyph: "ㄍ",
                shift_glyph: Some("ㆣ")
            }
        );
        assert_eq!(cap('1').shift_glyph, Some("ㆠ"));
        assert_eq!(cap('6').glyph, "\u{02ca}");
        assert_eq!(cap('6').shift_glyph, Some("\u{02c6}"));
        assert_eq!(cap('Q').shift_glyph, None);
        assert_eq!(cap('\'').glyph, "-");
        assert_eq!(cap('\'').shift_glyph, None);
    }

    #[test]
    fn a_press_on_the_shift_layer_types_the_shift_glyph_where_the_key_has_one() {
        // trace: KEYS — `e` ㄍ / `E` ㆣ; `q` ㄆ has no Shift glyph.
        assert_eq!(cap('E').pressed_glyph(false), "ㄍ");
        assert_eq!(cap('E').pressed_glyph(true), "ㆣ");
        assert_eq!(cap('Q').pressed_glyph(true), "ㄆ");
    }

    #[test]
    fn rows_are_staggered_like_a_keyboard() {
        let indents: Vec<f32> = tps_keyboard_rows().iter().map(|row| row.indent).collect();
        assert_eq!(indents, [0.0, 0.5, 0.75, 1.25]);
    }
}
