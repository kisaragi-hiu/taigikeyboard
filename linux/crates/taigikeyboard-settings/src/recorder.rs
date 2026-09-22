//! The shortcut recorder's state and its targets: which row is recording,
//! which registry it writes to, what the last press was refused for. The
//! decision itself is the shared `keys::evaluate_press`; the key events
//! come from a `gtk::EventControllerKey` on the window in the capture phase
//! (`window.rs`), so a recording field sees every key before any widget
//! does — the GTK counterpart of the Windows keyboard hook, with no hook.

use taigi_desktop_core::keys::{
    evaluate_press, ChordRejection, ComposingAction, ComposingKeyChord, RecordedPress,
    RecorderOutcome, RecorderTier, ShortcutAction, ShortcutConflicts,
};
use taigi_desktop_core::settings::SettingsDocument;
use taigi_desktop_core::strings::StringKey;

/// Which row is recording, and so which registry it writes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecorderTarget {
    Global(ShortcutAction),
    Composing(ComposingAction),
}

impl RecorderTarget {
    pub fn label_key(self) -> StringKey {
        match self {
            Self::Global(action) => action.label_key(),
            Self::Composing(action) => action.label_key(),
        }
    }

    fn tier(self) -> RecorderTier {
        match self {
            Self::Global(_) => RecorderTier::Global,
            Self::Composing(_) => RecorderTier::Composing,
        }
    }

    /// Stores `chord` on this row, emptying whatever else held it — last
    /// writer wins across BOTH registries (`ShortcutConflicts`).
    pub fn store(self, document: &mut SettingsDocument, chord: Option<&ComposingKeyChord>) {
        match self {
            Self::Global(action) => {
                action.store_in(document, chord);
                ShortcutConflicts::resolve_after_global_recording(document, action);
            }
            Self::Composing(action) => {
                if let Some(chord) = chord {
                    ShortcutConflicts::resolve_after_composing_recording(document, action, chord);
                }
                document.set_composing_chord(action, chord);
            }
        }
    }
}

/// The window's recording state.
#[derive(Debug, Default)]
pub struct Recorder {
    pub target: Option<RecorderTarget>,
    pub rejection: Option<ChordRejection>,
    /// The key held down, so a repeat (a press with no release between) is
    /// dropped as `evaluate_press` asks.
    held: Option<(u32, u32)>,
}

/// What the window does after a press.
#[derive(Debug, PartialEq, Eq)]
pub enum Recorded {
    /// Nothing to write; the field may need redrawing.
    Nothing,
    /// The chord to store on the row that was recording.
    Store(RecorderTarget, ComposingKeyChord),
}

impl Recorder {
    pub fn start(&mut self, target: RecorderTarget) {
        self.target = Some(target);
        self.rejection = None;
        self.held = None;
    }

    pub fn stop(&mut self) {
        self.target = None;
        self.rejection = None;
        self.held = None;
    }

    pub fn is_recording(&self, target: RecorderTarget) -> bool {
        self.target == Some(target)
    }

    /// One press (keyval, keycode, the recorded snapshot fields), judged by
    /// the shared decision — the same one the Mac and Windows ask.
    pub fn press(&mut self, keyval: u32, keycode: u32, mut press: RecordedPress) -> Recorded {
        let Some(target) = self.target else {
            return Recorded::Nothing;
        };
        press.is_repeat = self.held == Some((keyval, keycode));
        self.held = Some((keyval, keycode));
        match evaluate_press(target.tier(), &press) {
            RecorderOutcome::Recorded(chord) => {
                self.stop();
                Recorded::Store(target, chord)
            }
            RecorderOutcome::Refused(reason) => {
                self.rejection = Some(reason);
                Recorded::Nothing
            }
            // Escape leaves the row as it was.
            RecorderOutcome::Blurred => {
                self.stop();
                Recorded::Nothing
            }
            RecorderOutcome::Ignored => Recorded::Nothing,
        }
    }

    pub fn release(&mut self, keyval: u32, keycode: u32) {
        if self.held == Some((keyval, keycode)) {
            self.held = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taigi_desktop_core::keys::KeyModifiers;

    fn press(key: &str, modifiers: KeyModifiers) -> RecordedPress {
        RecordedPress {
            key: Some(key.to_owned()),
            modifiers,
            key_code: None,
            is_repeat: false,
        }
    }

    #[test]
    fn a_held_key_repeats_until_released_and_a_chord_ends_the_recording() {
        // trace: Ctrl+Alt+K on a global row → Recorded; the same press again
        // with no release → is_repeat → Ignored (Nothing, still recording).
        let mut recorder = Recorder::default();
        let target = RecorderTarget::Global(ShortcutAction::ShowTelexGuide);
        recorder.start(target);
        let chord = KeyModifiers::CONTROL.with(KeyModifiers::ALT);
        assert!(matches!(
            recorder.press(0x6b, 45, press("k", chord)),
            Recorded::Store(t, _) if t == target
        ));
        assert!(recorder.target.is_none());
        recorder.start(target);
        assert!(matches!(
            recorder.press(0x61, 38, press("a", KeyModifiers::NONE)),
            Recorded::Nothing
        ));
        assert!(recorder.rejection.is_some(), "a bare letter is refused");
        assert_eq!(
            recorder.press(0x61, 38, press("a", KeyModifiers::NONE)),
            Recorded::Nothing
        );
        recorder.release(0x61, 38);
        assert!(recorder.is_recording(target));
        assert_eq!(
            recorder.press(0xff1b, 9, press("\u{1B}", KeyModifiers::NONE)),
            Recorded::Nothing
        );
        assert!(!recorder.is_recording(target), "Escape blurs");
    }
}
