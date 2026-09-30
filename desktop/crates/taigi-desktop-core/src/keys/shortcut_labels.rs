//! The fixed rows of the Shortcuts pane: keys the user cannot rebind, named
//! in the recorder rows' own spelling so the pane reads as one list
//! (`ShortcutSettingsView.swift`).

use super::chord::ComposingKeyChord;
use super::intent::{CARET_CHORD_MODIFIERS, WIDTH_FLIP_MODIFIERS};
use super::slot_key_set::CandidateSlotKeySet;
use super::snapshot::KeyModifiers;
use crate::candidates::HorizontalPageLayout;

/// The six keys the fixed navigation tier reads, in the keycap legends
/// (`navigationKeysLabel`).
pub const NAVIGATION_KEYS_LABEL: &str = "←  →  ↑  ↓  PgUp  PgDn";

/// The cancel key's keycap legend (`cancelKeyLabel`).
pub const CANCEL_KEY_LABEL: &str = "Esc";

/// `Ctrl+←  Ctrl+→`, named by the same modifier labels the recorder rows
/// use, from the modifier the classifier reads (`caretChordsLabel`).
pub fn caret_chords_label() -> String {
    ["←", "→"]
        .map(|arrow| {
            ComposingKeyChord::modifier_labels(CARET_CHORD_MODIFIERS)
                .chain([arrow.to_owned()])
                .collect::<Vec<_>>()
                .join("+")
        })
        .join("  ")
}

/// `Ctrl+,  Ctrl+.  Ctrl+;` — three of the keys the width flip reaches
/// (`widthFlipChordsLabel`).
pub fn width_flip_chords_label() -> String {
    [",", ".", ";"]
        .map(|key| {
            ComposingKeyChord {
                key: key.to_owned(),
                modifiers: WIDTH_FLIP_MODIFIERS,
            }
            .display()
        })
        .join("  ")
}

/// `qwdfzxvy;` under Standard, `123456789` under Telex: every key of the
/// live slot set, bare — lowercase because a bare key shows the character
/// it types (`slotKeysLabel`).
pub fn slot_keys_label(slot_keys: CandidateSlotKeySet) -> String {
    ComposingKeyChord {
        key: slot_keys_run(slot_keys),
        modifiers: KeyModifiers::NONE,
    }
    .display()
}

/// `Shift+QWDFZXVY;` under Standard, `Shift+123456789` under Telex: every
/// key of the live slot set behind ONE Shift — the Hanji / romanization
/// commit aimed at a slot (`shiftedSlotKeysLabel`).
pub fn shifted_slot_keys_label(slot_keys: CandidateSlotKeySet) -> String {
    ComposingKeyChord {
        key: slot_keys_run(slot_keys),
        modifiers: KeyModifiers::SHIFT,
    }
    .display()
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

    #[test]
    fn the_fixed_rows_read_in_the_recorder_rows_spelling() {
        // trace: CARET_CHORD_MODIFIERS = WIDTH_FLIP_MODIFIERS = CONTROL,
        // whose label is "Ctrl"; `display` joins with "+" (read by running
        // the functions, 2026-09-30).
        assert_eq!(caret_chords_label(), "Ctrl+←  Ctrl+→");
        assert_eq!(width_flip_chords_label(), "Ctrl+,  Ctrl+.  Ctrl+;");
    }

    #[test]
    fn the_slot_rows_follow_the_live_slot_set() {
        // trace: BARE_KEY_ROW q w d f z x v y ; under Standard, digits under
        // Telex; Shift upper-cases the letters it is shown with.
        assert_eq!(slot_keys_label(CandidateSlotKeySet::BareKeys), "qwdfzxvy;");
        assert_eq!(
            shifted_slot_keys_label(CandidateSlotKeySet::BareKeys),
            "Shift+QWDFZXVY;"
        );
        assert_eq!(slot_keys_label(CandidateSlotKeySet::Digits), "123456789");
        assert_eq!(
            shifted_slot_keys_label(CandidateSlotKeySet::Digits),
            "Shift+123456789"
        );
    }
}
