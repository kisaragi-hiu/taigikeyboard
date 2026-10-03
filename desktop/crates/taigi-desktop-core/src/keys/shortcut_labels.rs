//! The fixed rows of the Shortcuts pane: keys the user cannot rebind, named
//! in the recorder rows' own spelling so the pane reads as one list
//! (`ShortcutSettingsView.swift`). Each desktop draws them in its own
//! spelling ([`ComposingKeyChord::display`]).

use super::chord::ComposingKeyChord;
use super::intent::{caret_chord_modifiers, WIDTH_FLIP_MODIFIERS};
use super::slot_key_set::CandidateSlotKeySet;
use super::snapshot::KeyModifiers;
use crate::candidates::HorizontalPageLayout;
use crate::platform::DesktopPlatform;

/// The six keys the fixed navigation tier reads, in the keycap legends
/// (`navigationKeysLabel`): `←  →  ↑  ↓  PgUp  PgDn`, the Mac's
/// `←  →  ↑  ↓  ⇞  ⇟`.
pub fn navigation_keys_label(platform: DesktopPlatform) -> &'static str {
    match platform {
        DesktopPlatform::Windows | DesktopPlatform::Linux => "←  →  ↑  ↓  PgUp  PgDn",
        DesktopPlatform::MacOS => "←  →  ↑  ↓  ⇞  ⇟",
    }
}

/// The cancel key's keycap legend (`cancelKeyLabel`): `Esc`, the Mac's `⎋`.
pub fn cancel_key_label(platform: DesktopPlatform) -> &'static str {
    match platform {
        DesktopPlatform::Windows | DesktopPlatform::Linux => "Esc",
        DesktopPlatform::MacOS => "⎋",
    }
}

/// `Ctrl+←  Ctrl+→` (the Mac's `⌥←  ⌥→`), named by the same modifier
/// labels the recorder rows use, from the modifier the classifier reads
/// (`caretChordsLabel`).
pub fn caret_chords_label(platform: DesktopPlatform) -> String {
    ["←", "→"]
        .map(|arrow| {
            ComposingKeyChord {
                key: arrow.to_owned(),
                modifiers: caret_chord_modifiers(platform),
            }
            .display(platform)
        })
        .join("  ")
}

/// `Ctrl+,  Ctrl+.  Ctrl+;` (the Mac's `⌃,  ⌃.  ⌃;`) — three of the keys
/// the width flip reaches (`widthFlipChordsLabel`).
pub fn width_flip_chords_label(platform: DesktopPlatform) -> String {
    [",", ".", ";"]
        .map(|key| {
            ComposingKeyChord {
                key: key.to_owned(),
                modifiers: WIDTH_FLIP_MODIFIERS,
            }
            .display(platform)
        })
        .join("  ")
}

/// `qwdfzxvy;` under Standard, `123456789` under Telex: every key of the
/// live slot set, bare — lowercase because a bare key shows the character
/// it types (`slotKeysLabel`).
pub fn slot_keys_label(slot_keys: CandidateSlotKeySet, platform: DesktopPlatform) -> String {
    ComposingKeyChord {
        key: slot_keys_run(slot_keys),
        modifiers: KeyModifiers::NONE,
    }
    .display(platform)
}

/// `Shift+QWDFZXVY;` under Standard, `Shift+123456789` under Telex (the
/// Mac's `⇧QWDFZXVY;`): every key of the live slot set behind ONE Shift —
/// the Hanji / romanization commit aimed at a slot (`shiftedSlotKeysLabel`).
pub fn shifted_slot_keys_label(
    slot_keys: CandidateSlotKeySet,
    platform: DesktopPlatform,
) -> String {
    ComposingKeyChord {
        key: slot_keys_run(slot_keys),
        modifiers: KeyModifiers::SHIFT,
    }
    .display(platform)
}

/// The nine slot keys of `slot_keys` as one run, in page order.
fn slot_keys_run(slot_keys: CandidateSlotKeySet) -> String {
    (0..HorizontalPageLayout::PAGE_SIZE)
        .map(|slot| slot_keys.label_for_slot(slot))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::test_support::WINDOWS_AND_LINUX;

    #[test]
    fn the_fixed_rows_read_in_the_recorder_rows_spelling() {
        // trace: caret_chord_modifiers = WIDTH_FLIP_MODIFIERS = CONTROL on
        // Windows and Linux, whose label is "Ctrl"; `display` joins with "+"
        // (read by running the functions, 2026-09-30).
        for platform in WINDOWS_AND_LINUX {
            assert_eq!(caret_chords_label(platform), "Ctrl+←  Ctrl+→");
            assert_eq!(width_flip_chords_label(platform), "Ctrl+,  Ctrl+.  Ctrl+;");
            assert_eq!(navigation_keys_label(platform), "←  →  ↑  ↓  PgUp  PgDn");
            assert_eq!(cancel_key_label(platform), "Esc");
        }
    }

    #[test]
    fn the_mac_fixed_rows_read_in_glyphs() {
        // trace: ShortcutSettingsView.swift:152-180 — caret ⌥ (`caretChordModifiers`),
        // width flip ⌃, ShortcutKeyDisplay's ⇞ ⇟ ⎋, no separator.
        let mac = DesktopPlatform::MacOS;
        assert_eq!(caret_chords_label(mac), "⌥←  ⌥→");
        assert_eq!(width_flip_chords_label(mac), "⌃,  ⌃.  ⌃;");
        assert_eq!(navigation_keys_label(mac), "←  →  ↑  ↓  ⇞  ⇟");
        assert_eq!(cancel_key_label(mac), "⎋");
    }

    #[test]
    fn the_slot_rows_follow_the_live_slot_set() {
        // trace: BARE_KEY_ROW q w d f z x v y ; under Standard, digits under
        // Telex; Shift upper-cases the letters it is shown with.
        for platform in WINDOWS_AND_LINUX {
            assert_eq!(
                slot_keys_label(CandidateSlotKeySet::BareKeys, platform),
                "qwdfzxvy;"
            );
            assert_eq!(
                shifted_slot_keys_label(CandidateSlotKeySet::BareKeys, platform),
                "Shift+QWDFZXVY;"
            );
            assert_eq!(
                slot_keys_label(CandidateSlotKeySet::Digits, platform),
                "123456789"
            );
            assert_eq!(
                shifted_slot_keys_label(CandidateSlotKeySet::Digits, platform),
                "Shift+123456789"
            );
        }
        // trace: ShortcutSettingsView.swift:186-199 — one ⇧ ahead of the run.
        let mac = DesktopPlatform::MacOS;
        assert_eq!(
            slot_keys_label(CandidateSlotKeySet::BareKeys, mac),
            "qwdfzxvy;"
        );
        assert_eq!(
            shifted_slot_keys_label(CandidateSlotKeySet::BareKeys, mac),
            "⇧QWDFZXVY;"
        );
        assert_eq!(
            slot_keys_label(CandidateSlotKeySet::Digits, mac),
            "123456789"
        );
        assert_eq!(
            shifted_slot_keys_label(CandidateSlotKeySet::Digits, mac),
            "⇧123456789"
        );
    }
}
