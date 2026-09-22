//! The shortcut recorder's state and its targets: which row is recording,
//! which registry it writes to, what the last press was refused for. The
//! decision itself is the shared `keys::evaluate_press`; the key events
//! come from a `gtk::EventControllerKey` on the window in the capture phase
//! (`window.rs`), so a recording field sees every key before any widget
//! does — the GTK counterpart of the Windows keyboard hook, with no hook.

use std::collections::HashSet;
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
    /// The physical keys held down (keycodes — the keysym of a repeat can
    /// change when a modifier joins it), so a repeat is dropped as
    /// `evaluate_press` asks, and a key the recorder consumed stays
    /// swallowed until its release even after the recording ended: a held
    /// Tab must not walk the form, a held Enter must not press the field
    /// again. GTK repeats presses without a release between them on both
    /// X11 (detectable autorepeat) and Wayland.
    held: HashSet<u32>,
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
    }

    /// Ends the recording; the keys still held stay owed their release.
    pub fn stop(&mut self) -> bool {
        let was_recording = self.target.is_some();
        self.target = None;
        self.rejection = None;
        was_recording
    }

    pub fn is_recording(&self, target: RecorderTarget) -> bool {
        self.target == Some(target)
    }

    /// Whether a press of `keycode` is the recorder's: every press while a
    /// row records, and the repeats of a key it consumed until released.
    pub fn swallows(&self, keycode: u32) -> bool {
        self.target.is_some() || self.held.contains(&keycode)
    }

    /// One press (its keycode, the recorded snapshot fields), judged by the
    /// shared decision — the same one the Mac and Windows ask.
    pub fn press(&mut self, keycode: u32, mut press: RecordedPress) -> Recorded {
        press.is_repeat = !self.held.insert(keycode);
        let Some(target) = self.target else {
            return Recorded::Nothing;
        };
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

    pub fn release(&mut self, keycode: u32) {
        self.held.remove(&keycode);
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
    fn a_chord_ends_the_recording_and_its_key_stays_swallowed_until_released() {
        // trace: Ctrl+Alt+K (keycode 45) on a global row → Recorded, stop;
        // the held K's repeats are still the recorder's until the release.
        let mut recorder = Recorder::default();
        let target = RecorderTarget::Global(ShortcutAction::ShowTelexGuide);
        recorder.start(target);
        let chord = KeyModifiers::CONTROL.with(KeyModifiers::ALT);
        assert!(matches!(
            recorder.press(45, press("k", chord)),
            Recorded::Store(t, _) if t == target
        ));
        assert!(recorder.target.is_none());
        assert!(
            recorder.swallows(45),
            "the held key is still owed a release"
        );
        assert!(!recorder.swallows(46));
        assert_eq!(recorder.press(45, press("k", chord)), Recorded::Nothing);
        recorder.release(45);
        assert!(!recorder.swallows(45));
    }

    #[test]
    fn a_repeat_is_the_keycode_not_the_keysym_and_escape_blurs() {
        // trace: a held `a` (keycode 38) is refused; Shift joining it makes
        // the repeat arrive as `A` — same keycode, still a repeat, never a
        // fresh Shift+A recording. Escape ends the recording, keeps the row.
        let mut recorder = Recorder::default();
        let target = RecorderTarget::Global(ShortcutAction::ShowTelexGuide);
        recorder.start(target);
        assert_eq!(
            recorder.press(38, press("a", KeyModifiers::NONE)),
            Recorded::Nothing
        );
        assert!(recorder.rejection.is_some(), "a bare letter is refused");
        recorder.rejection = None;
        assert_eq!(
            recorder.press(38, press("a", KeyModifiers::SHIFT)),
            Recorded::Nothing
        );
        assert!(
            recorder.rejection.is_none(),
            "a repeat is ignored, not judged"
        );
        recorder.release(38);
        assert!(recorder.is_recording(target));
        assert_eq!(
            recorder.press(9, press("\u{1B}", KeyModifiers::NONE)),
            Recorded::Nothing
        );
        assert!(!recorder.is_recording(target), "Escape blurs");
    }
}
