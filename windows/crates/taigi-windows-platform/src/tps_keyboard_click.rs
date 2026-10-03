//! How a click on the on-screen TPS key panel reaches the key sink (desktop
//! TPS roadmap D6): the panel injects one key with `SendInput` — a window
//! procedure may not ask for an edit session (W3), a key sink may — and
//! that key names the glyph itself, so its delivery needs no state the
//! panel left behind. The key is the unassigned virtual key 0xE8, carrying
//! the glyph's number on the panel as its scan code (`lParam` bits 16–23).
//! A click lost on its way (focus moved to another application between the
//! click and the delivery) reaches that application as a key no layout
//! gives a character, and a later click cannot pick up its glyph.

use taigi_desktop_core::keys::{tps_keyboard_rows, ComposingKeyIntent};
use taigi_desktop_core::settings::InputMode;

/// The virtual key a click is injected as — "Unassigned" in the Win32
/// virtual-key table, so no keyboard layout gives it a character.
pub const TPS_KEYBOARD_CLICK_VIRTUAL_KEY: u16 = 0xE8;

/// Every glyph on the panel, row by row, each cap's bare glyph then its
/// Shift glyph — the numbering the scan code carries.
fn panel_glyphs() -> impl Iterator<Item = &'static str> {
    tps_keyboard_rows()
        .into_iter()
        .flat_map(|row| row.caps)
        .flat_map(|cap| [Some(cap.glyph), cap.shift_glyph])
        .flatten()
}

/// The scan code a click typing `glyph` is injected with: its number on the
/// panel, counted from 1, so 0 names no glyph.
pub fn scan_code_for_glyph(glyph: &str) -> Option<u16> {
    let index = panel_glyphs().position(|panel_glyph| panel_glyph == glyph)?;
    u16::try_from(index + 1).ok()
}

/// The glyph a click's scan code names, or `None` for a scan code no click
/// carries.
fn glyph_for_scan_code(scan_code: u32) -> Option<&'static str> {
    let index = usize::try_from(scan_code.checked_sub(1)?).ok()?;
    panel_glyphs().nth(index)
}

/// What a delivered click types under `input_mode`: the glyph its scan code
/// names, through the same check every click takes
/// (`ComposingKeyIntent::tps_keyboard_press`) — `None` outside TPS or for a
/// scan code that names no glyph.
pub fn click_intent(scan_code: u32, input_mode: InputMode) -> Option<ComposingKeyIntent> {
    ComposingKeyIntent::tps_keyboard_press(glyph_for_scan_code(scan_code)?, input_mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_types_its_glyph_only_under_tps() {
        // trace: scan code 1 = ㄅ (the `1` cap's bare glyph).
        assert_eq!(
            click_intent(1, InputMode::Tps),
            Some(ComposingKeyIntent::TpsKey("ㄅ".into()))
        );
        assert_eq!(click_intent(1, InputMode::Tl), None);
        assert_eq!(click_intent(1, InputMode::Poj), None);
        assert_eq!(click_intent(0, InputMode::Tps), None);
        assert_eq!(click_intent(0xFF, InputMode::Tps), None);
    }

    #[test]
    fn every_glyph_on_the_panel_round_trips_through_its_scan_code() {
        let glyphs: Vec<&str> = panel_glyphs().collect();
        for glyph in &glyphs {
            let scan_code = scan_code_for_glyph(glyph).expect("a glyph on the panel");
            assert_eq!(glyph_for_scan_code(u32::from(scan_code)), Some(*glyph));
        }
        // `lParam` carries eight bits of scan code.
        assert!(glyphs.len() < 0xFF, "{} glyphs", glyphs.len());
    }

    #[test]
    fn the_numbering_starts_at_the_first_cap_and_skips_nothing() {
        // trace: tps_keyboard_rows — the first cap is `1` (ㄅ, Shift ㆠ),
        // the second `2` (ㄉ, no Shift glyph).
        assert_eq!(scan_code_for_glyph("ㄅ"), Some(1));
        assert_eq!(scan_code_for_glyph("ㆠ"), Some(2));
        assert_eq!(scan_code_for_glyph("ㄉ"), Some(3));
        assert_eq!(scan_code_for_glyph("a"), None);
        assert_eq!(scan_code_for_glyph(" "), None);
    }

    #[test]
    fn a_scan_code_no_click_carries_names_no_glyph() {
        assert_eq!(glyph_for_scan_code(0), None);
        assert_eq!(glyph_for_scan_code(0xFF), None);
    }
}
