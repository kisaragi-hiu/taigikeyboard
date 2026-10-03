//! What a key means while the symbol picker is up. Port of macOS
//! `SymbolPicker.swift`.

use super::bindings::ComposingKeyBindings;
use super::intent::CandidateNavigation;
use super::snapshot::KeyEventSnapshot;
use super::ComposingAction;
use crate::platform::DesktopPlatform;

/// The picker's reading of one key event, decided before any window is
/// asked anything. Its own table rather than a branch of
/// `ComposingKeyIntent`: that classifier is the contract of a COMPOSITION,
/// and the picker runs with none — its keys pick from a list the engine
/// never fetched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolPickerIntent {
    /// Take the picker down and swallow the key. Escape.
    Close,
    /// Move the selection the way the window's layout reads the direction.
    Navigate(CandidateNavigation),
    /// Pick the cell the `slot`-th selection key addresses on the visible
    /// page — the same keys that pick a candidate (`CandidateSlotKeySet`).
    PickSlot(usize),
    /// Pick the highlighted cell.
    Confirm,
    /// Take the picker down and let the key go on to do its job: the picker
    /// is a list to pick from, not a mode, so a letter typed over it starts
    /// the composition it would have started anyway.
    CloseAndPassThrough,
}

impl SymbolPickerIntent {
    /// Classifies `key` for a picker that is on screen.
    ///
    /// Reads the same rules the candidate list does, in the same order —
    /// the fixed navigation keys, then the slot keys, then whatever the user
    /// put on the paging and confirm rows — so a user who moved paging to
    /// Alt+Enter pages the picker with it too. Only the list-specific
    /// outcomes differ: there is no other script to commit, so the Hanji/romanization key
    /// confirms like Enter, and the literal-commit key has no literal to
    /// write, so it falls through.
    pub fn intent(
        key: &KeyEventSnapshot,
        bindings: &ComposingKeyBindings,
        platform: DesktopPlatform,
    ) -> Self {
        if key.is_bare_escape() {
            return Self::Close;
        }
        let modifiers = key.modifiers;
        if !modifiers.shift && !modifiers.has_host_chord() {
            if let Some(navigation) = key.navigation_key {
                return Self::Navigate(CandidateNavigation::from(navigation));
            }
        }
        if let Some(slot) = bindings.slot_key_set().slot_for_event(key) {
            return Self::PickSlot(slot);
        }
        match bindings.action_for(key, platform) {
            Some(action) if action.navigation().is_some() => Self::Navigate(
                action
                    .navigation()
                    .unwrap_or(CandidateNavigation::NextCandidate),
            ),
            Some(ComposingAction::ConfirmHighlighted | ComposingAction::CommitAlternateScript) => {
                Self::Confirm
            }
            _ => Self::CloseAndPassThrough,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{
        ComposingKeyChord, ComposingKeyIntent, KeyModifiers, NavigationKey, ToneInputScheme,
    };
    use crate::platform::test_support::ALL_PLATFORMS;

    /// The picker's answer under the shipped bindings — the same on every
    /// desktop, the Mac included (`taigi-macos-ffi` `key_rules.rs` asks it).
    fn intent(key: &KeyEventSnapshot) -> SymbolPickerIntent {
        let answers = ALL_PLATFORMS.map(|platform| {
            SymbolPickerIntent::intent(key, &ComposingKeyBindings::default(), platform)
        });
        assert!(
            answers.iter().all(|answer| *answer == answers[0]),
            "{answers:?}"
        );
        answers[0]
    }

    fn text(characters: &str) -> KeyEventSnapshot {
        KeyEventSnapshot::text(characters, KeyModifiers::NONE)
    }

    fn named(characters: &str, modifiers: KeyModifiers) -> KeyEventSnapshot {
        KeyEventSnapshot {
            is_named_special_key: true,
            ..KeyEventSnapshot::text(characters, modifiers)
        }
    }

    #[test]
    fn escape_closes_but_not_under_a_host_chord() {
        // trace: SymbolPickerIntentTests.swift `testEscape_closes_butNotUnderAHostChord`.
        assert_eq!(intent(&text("\u{1B}")), SymbolPickerIntent::Close);
        assert_eq!(
            intent(&KeyEventSnapshot::chord(
                Some("\u{1B}"),
                "3",
                KeyModifiers::CONTROL
            )),
            SymbolPickerIntent::CloseAndPassThrough
        );
    }

    /// Roadmap E2b, settled P11c: `is_bare_escape` reads the whole string,
    /// so `"\u{1B}x"` is not the Escape that closes and swallows: the picker
    /// closes and passes the event through. Swift's `isPlainEscape`
    /// reads the whole string too since P11c (before: the first grapheme,
    /// which swallowed it); pinned there by `SymbolPickerIntentTests`
    /// `testAnEscapeInsideALongerEvent_closesAndFallsThrough`.
    #[test]
    fn e2b_an_escape_inside_a_longer_event_closes_and_passes_through() {
        let mac = DesktopPlatform::MacOS;
        let event = text("\u{1B}x");
        // trace: not `is_bare_escape`; no navigation key; `"\u{1B}x"` is no
        // slot key; no binding holds it → CloseAndPassThrough.
        assert!(!event.is_bare_escape());
        assert_eq!(
            SymbolPickerIntent::intent(&event, &ComposingKeyBindings::default(), mac),
            SymbolPickerIntent::CloseAndPassThrough
        );
        // Negative control: the composition's own Escape tier reads the first
        // scalar, as Swift reads the first grapheme — Cancel on both.
        assert_eq!(
            ComposingKeyIntent::intent(&event, true, false, &ComposingKeyBindings::default(), mac),
            ComposingKeyIntent::Cancel
        );
    }

    #[test]
    fn the_arrows_and_paging_keys_navigate_but_a_shifted_arrow_falls_through() {
        for (key, direction) in [
            (NavigationKey::LeftArrow, CandidateNavigation::Left),
            (NavigationKey::RightArrow, CandidateNavigation::Right),
            (NavigationKey::UpArrow, CandidateNavigation::Up),
            (NavigationKey::DownArrow, CandidateNavigation::Down),
            (NavigationKey::PageUp, CandidateNavigation::PageUp),
            (NavigationKey::PageDown, CandidateNavigation::PageDown),
        ] {
            assert_eq!(
                intent(&KeyEventSnapshot::navigation(key, KeyModifiers::NONE)),
                SymbolPickerIntent::Navigate(direction)
            );
        }
        assert_eq!(
            intent(&KeyEventSnapshot::navigation(
                NavigationKey::LeftArrow,
                KeyModifiers::SHIFT
            )),
            SymbolPickerIntent::CloseAndPassThrough
        );
    }

    #[test]
    fn the_slot_keys_follow_the_tone_scheme() {
        assert_eq!(intent(&text("q")), SymbolPickerIntent::PickSlot(0));
        assert_eq!(intent(&text(";")), SymbolPickerIntent::PickSlot(8));
        assert_eq!(
            intent(&text("1")),
            SymbolPickerIntent::CloseAndPassThrough,
            "a digit is not a slot key under Standard"
        );
        // The set itself is `CandidateSlotKeySet`'s and pinned in `intent.rs`;
        // one assertion per scheme shows the picker reads it.
        let telex = ComposingKeyBindings::resolve(&Default::default(), ToneInputScheme::Telex);
        for platform in ALL_PLATFORMS {
            let under_telex =
                |key: &KeyEventSnapshot| SymbolPickerIntent::intent(key, &telex, platform);
            assert_eq!(under_telex(&text("1")), SymbolPickerIntent::PickSlot(0));
            assert_eq!(under_telex(&text("9")), SymbolPickerIntent::PickSlot(8));
            // trace: `direct_selection_slot` reads the first scalar, so a
            // keycap `1` + U+20E3 still names slot 0; `0`, a full-width `１`
            // and a chorded digit name none.
            assert_eq!(
                under_telex(&text("1\u{20E3}")),
                SymbolPickerIntent::PickSlot(0)
            );
            for none in [
                text("0"),
                text("\u{FF11}"),
                KeyEventSnapshot::text("1", KeyModifiers::WIN),
            ] {
                assert_eq!(
                    under_telex(&none),
                    SymbolPickerIntent::CloseAndPassThrough,
                    "{platform:?} {none:?}"
                );
            }
            assert_eq!(
                under_telex(&text("q")),
                SymbolPickerIntent::CloseAndPassThrough,
                "a letter is a tone key under Telex"
            );
        }
    }

    #[test]
    fn the_bound_rows_page_and_confirm() {
        assert_eq!(
            intent(&named("\t", KeyModifiers::NONE)),
            SymbolPickerIntent::Navigate(CandidateNavigation::NextCandidate)
        );
        assert_eq!(
            intent(&named("\t", KeyModifiers::SHIFT)),
            SymbolPickerIntent::Navigate(CandidateNavigation::PreviousCandidate)
        );
        assert_eq!(
            intent(&text("]")),
            SymbolPickerIntent::Navigate(CandidateNavigation::PageDown)
        );
        assert_eq!(
            intent(&text("[")),
            SymbolPickerIntent::Navigate(CandidateNavigation::PageUp)
        );
        assert_eq!(
            intent(&named("\r", KeyModifiers::NONE)),
            SymbolPickerIntent::Confirm
        );
        assert_eq!(intent(&text(" ")), SymbolPickerIntent::Confirm);
    }

    #[test]
    fn a_recorded_paging_chord_is_read() {
        for platform in ALL_PLATFORMS {
            for modifiers in [
                KeyModifiers::CONTROL.with(KeyModifiers::SHIFT),
                KeyModifiers::ALT,
            ] {
                let chord = ComposingKeyChord::make(Some("\r"), modifiers, platform).unwrap();
                let mut document = crate::settings::SettingsDocument::default();
                document.set_composing_chord(ComposingAction::PageForward, Some(&chord), platform);
                let bindings = ComposingKeyBindings::from_document(&document, platform);
                assert_eq!(
                    SymbolPickerIntent::intent(&named("\r", modifiers), &bindings, platform),
                    SymbolPickerIntent::Navigate(CandidateNavigation::PageDown),
                    "{platform:?} {modifiers:?}"
                );
                assert_eq!(
                    SymbolPickerIntent::intent(&text("]"), &bindings, platform),
                    SymbolPickerIntent::CloseAndPassThrough
                );
            }
        }
        // The Mac spells Option `o` in the stored value.
        let option_return =
            ComposingKeyChord::make(Some("\r"), KeyModifiers::ALT, DesktopPlatform::MacOS).unwrap();
        assert_eq!(option_return.raw_value(DesktopPlatform::MacOS), "o|000D");
    }

    #[test]
    fn everything_else_closes_and_falls_through() {
        assert_eq!(
            intent(&named("\r", KeyModifiers::SHIFT)),
            SymbolPickerIntent::CloseAndPassThrough,
            "the literal commit has no literal to write"
        );
        assert_eq!(intent(&text("a")), SymbolPickerIntent::CloseAndPassThrough);
        assert_eq!(
            intent(&KeyEventSnapshot::chord(
                Some("\u{13}"),
                "s",
                KeyModifiers::CONTROL
            )),
            SymbolPickerIntent::CloseAndPassThrough
        );
        assert_eq!(
            intent(&text("\u{8}")),
            SymbolPickerIntent::CloseAndPassThrough
        );
        assert_eq!(
            intent(&KeyEventSnapshot::text("s", KeyModifiers::WIN)),
            SymbolPickerIntent::CloseAndPassThrough
        );
    }
}
