//! The key rules the Swift Shortcuts pane and symbol picker ask
//! (docs/architecture/macos-desktop-core-roadmap.md P14): desktop-core's
//! chord gate, recorder decision, resolved bindings, fixed-row labels and
//! picker reading, under the Mac's grammar. Pure — no token, no session, no
//! runtime: a settings snapshot one of them carries is read for its one
//! answer and never installed, so the key path keeps what the last session
//! request carried, a panic here included.

use taigi_desktop_core::keys::{
    evaluate_press, shortcut_labels, ChordRejection, ComposingAction, ComposingKeyBindings,
    ComposingKeyChord, RecorderOutcome, RecorderTier, SymbolPickerIntent,
};
use taigi_desktop_core::settings::SettingsDocument;

use crate::key_translation::{self, ESCAPE_KEY_CODE};
use crate::proto::{
    chord_reply, desktop_request, desktop_response, press_reply, symbol_picker_reply, Chord,
    ChordReply, ChordRequest, ComposingShortcut, ComposingShortcutsReply, PressReply, PressRequest,
    RecorderAction, SettingsSnapshot, SymbolPickerAction, SymbolPickerKeyRequest,
    SymbolPickerReply,
};
use crate::runtime::{Refusal, DESKTOP_PLATFORM};
use crate::session::navigation;
use crate::settings;

/// The answer to `request` if it is a key rule, under the snapshot it
/// carries; `None` for a session request, which `Shell::serve` runs under
/// the snapshot it puts in force. The one list of the key rules.
pub(crate) fn answer(
    request: &desktop_request::Request,
    snapshot: Option<&SettingsSnapshot>,
) -> Option<Result<desktop_response::Reply, Refusal>> {
    use desktop_request::Request;
    use desktop_response::Reply;
    Some(match request {
        Request::Press(press) => self::press(press).map(Reply::Press),
        Request::Chord(chord) => Ok(Reply::Chord(self::chord(chord))),
        Request::ComposingShortcuts(_) => {
            composing_shortcuts(snapshot).map(Reply::ComposingShortcuts)
        }
        Request::SymbolPickerKey(key) => symbol_picker_key(key, snapshot).map(Reply::SymbolPicker),
        _ => return None,
    })
}

/// One press in a recording field: `evaluate_press` on the composing tier,
/// the Mac's press translation in front of it (`recorded_press`).
///
/// The Mac recorder leaves on the Escape KEY (`kVK_Escape`), not on the ESC
/// character: a bare Escape blurs whatever the layout types for it, and a
/// bare ESC another key types is refused as the reserved key it is, as the
/// gate refuses it — never the way out. The core's blanking keys come first:
/// an Escape key a layout reports as Backspace or Delete blanks the field.
fn press(request: &PressRequest) -> Result<PressReply, Refusal> {
    let event = request.event.as_ref().ok_or(Refusal::Missing("event"))?;
    let press = key_translation::recorded_press(event);
    let is_bare_escape_key = press.modifiers.is_empty() && event.key_code == Some(ESCAPE_KEY_CODE);
    let outcome = match evaluate_press(RecorderTier::Composing, &press, DESKTOP_PLATFORM) {
        RecorderOutcome::Ignored => RecorderOutcome::Ignored,
        _ if is_bare_escape_key => RecorderOutcome::Blurred,
        // The core blurs on the ESC character; typed by another key, it is
        // the reserved key the gate refuses.
        RecorderOutcome::Blurred => RecorderOutcome::Refused(ChordRejection::ReservedKey),
        other => other,
    };
    use press_reply::Outcome;
    let outcome = match outcome {
        RecorderOutcome::Recorded(chord) => Outcome::Recorded(chord_message(&chord)),
        RecorderOutcome::Refused(reason) => Outcome::Refused(rejection(reason) as i32),
        RecorderOutcome::Blurred => Outcome::Action(RecorderAction::Blur as i32),
        // A repeat is Swift's to drop, so `Ignored` is only ever a bare
        // Backspace / Delete / forward Delete, which blanks the field.
        RecorderOutcome::Ignored => Outcome::Action(RecorderAction::Blank as i32),
    };
    Ok(PressReply {
        outcome: Some(outcome),
    })
}

/// The gate a key and modifiers go through (`ComposingKeyChord::make`).
fn chord(request: &ChordRequest) -> ChordReply {
    let modifiers = key_translation::modifiers(request.modifier_flags);
    let answer = match ComposingKeyChord::make(request.key.as_deref(), modifiers, DESKTOP_PLATFORM)
    {
        Ok(chord) => chord_reply::Answer::Chord(chord_message(&chord)),
        Err(reason) => chord_reply::Answer::Rejection(rejection(reason) as i32),
    };
    ChordReply {
        answer: Some(answer),
    }
}

/// The pane's composing rows, resolved from `snapshot`, and its fixed rows.
fn composing_shortcuts(
    snapshot: Option<&SettingsSnapshot>,
) -> Result<ComposingShortcutsReply, Refusal> {
    let bindings = bindings(snapshot)?;
    let slot_keys = bindings.slot_key_set();
    let actions = ComposingAction::ALL
        .into_iter()
        .map(|action| ComposingShortcut {
            name: action.raw().to_owned(),
            settings_key: action.settings_key_name(),
            label_key: action.label_key().as_str().to_owned(),
            group: group(action),
            chord: bindings.chord(action).map(chord_message),
            default_chord: Some(chord_message(&action.default_chord())),
        })
        .collect();
    Ok(ComposingShortcutsReply {
        actions,
        slot_keys: shortcut_labels::slot_keys_label(slot_keys, DESKTOP_PLATFORM),
        // Empty only under TPS, which this seam projects to TL until desktop
        // TPS P4 (`settings.rs` `document_from`).
        shifted_slot_keys: shortcut_labels::shifted_slot_keys_label(slot_keys, DESKTOP_PLATFORM)
            .unwrap_or_default(),
        navigation_keys: shortcut_labels::navigation_keys_label(DESKTOP_PLATFORM).to_owned(),
        caret_chords: shortcut_labels::caret_chords_label(DESKTOP_PLATFORM),
        width_flip_chords: shortcut_labels::width_flip_chords_label(DESKTOP_PLATFORM),
        cancel_key: shortcut_labels::cancel_key_label(DESKTOP_PLATFORM).to_owned(),
    })
}

/// One key while the picker is up, read under `snapshot`'s bindings.
fn symbol_picker_key(
    request: &SymbolPickerKeyRequest,
    snapshot: Option<&SettingsSnapshot>,
) -> Result<SymbolPickerReply, Refusal> {
    let event = request.event.as_ref().ok_or(Refusal::Missing("event"))?;
    let bindings = bindings(snapshot)?;
    let key = key_translation::snapshot(event);
    use symbol_picker_reply::Intent;
    let intent = match SymbolPickerIntent::intent(&key, &bindings, DESKTOP_PLATFORM) {
        SymbolPickerIntent::Close => Intent::Action(SymbolPickerAction::Close as i32),
        SymbolPickerIntent::Navigate(direction) => Intent::Navigate(navigation(direction) as i32),
        // A slot is one of the nine a page holds.
        SymbolPickerIntent::PickSlot(slot) => Intent::PickSlot(slot as u32),
        SymbolPickerIntent::Confirm => Intent::Action(SymbolPickerAction::Confirm as i32),
        SymbolPickerIntent::CloseAndPassThrough => {
            Intent::Action(SymbolPickerAction::CloseAndPassThrough as i32)
        }
    };
    Ok(SymbolPickerReply {
        intent: Some(intent),
    })
}

/// The bindings the snapshot describes. A snapshot is required: the last
/// one the key path installed may be a different store's.
fn bindings(snapshot: Option<&SettingsSnapshot>) -> Result<ComposingKeyBindings, Refusal> {
    let snapshot = snapshot.ok_or(Refusal::Missing("settings"))?;
    let document: SettingsDocument =
        settings::document_from(&snapshot.entries).map_err(Refusal::Settings)?;
    Ok(ComposingKeyBindings::from_document(
        &document,
        DESKTOP_PLATFORM,
    ))
}

fn chord_message(chord: &ComposingKeyChord) -> Chord {
    Chord {
        raw: chord.raw_value(DESKTOP_PLATFORM),
        display: chord.display(DESKTOP_PLATFORM),
    }
}

/// The pane block `action` is drawn in.
fn group(action: ComposingAction) -> u32 {
    ComposingAction::GROUPS
        .iter()
        .position(|group| group.contains(&action))
        .map_or(0, |index| index as u32)
}

fn rejection(reason: ChordRejection) -> crate::proto::ChordRejection {
    use crate::proto::ChordRejection as Wire;
    match reason {
        ChordRejection::TypesRomanization => Wire::TypesRomanization,
        ChordRejection::ReservedKey => Wire::ReservedKey,
        ChordRejection::NoKey => Wire::NoKey,
        ChordRejection::TakenBySystem => Wire::TakenBySystem,
        ChordRejection::NotAGlobalKey => Wire::NotAGlobalKey,
        ChordRejection::BelongsToHost => Wire::BelongsToHost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_translation::{COMMAND, CONTROL, OPTION, SHIFT};
    use crate::proto::{CandidateNavigation as WireNavigation, ChordRejection as Wire, KeyEvent};
    use crate::test_support::{key_event, text};

    use crate::test_support::{CAPS_LOCK, FUNCTION};

    fn press_of(event: KeyEvent) -> press_reply::Outcome {
        press(&PressRequest { event: Some(event) })
            .expect("answered")
            .outcome
            .expect("an outcome")
    }

    fn recorded(raw: &str, display: &str) -> press_reply::Outcome {
        press_reply::Outcome::Recorded(Chord {
            raw: raw.to_owned(),
            display: display.to_owned(),
        })
    }

    fn refused(reason: Wire) -> press_reply::Outcome {
        press_reply::Outcome::Refused(reason as i32)
    }

    fn action(action: RecorderAction) -> press_reply::Outcome {
        press_reply::Outcome::Action(action as i32)
    }

    fn with_key_code(event: KeyEvent, key_code: u32) -> KeyEvent {
        KeyEvent {
            key_code: Some(key_code),
            ..event
        }
    }

    /// P8 → P14: a bare ⌦ blanks the field as `.deleteForward` did in
    /// Swift — Caps Lock and Fn (a laptop's ⌦ is Fn+⌫) do not make it a
    /// chord — and with a chording modifier it records as F728.
    #[test]
    fn a_bare_forward_delete_blanks_and_a_chorded_one_records() {
        for flags in [0, CAPS_LOCK | FUNCTION] {
            assert_eq!(
                press_of(key_event("\u{F728}", flags, Some(0xF728))),
                action(RecorderAction::Blank),
                "{flags:#x}"
            );
        }
        assert_eq!(
            press_of(key_event("\u{F728}", CONTROL | FUNCTION, Some(0xF728))),
            recorded("c|F728", "⌃\u{F728}")
        );
        assert_eq!(
            press_of(key_event("\u{F728}", SHIFT, Some(0xF728))),
            recorded("s|F728", "⇧\u{F728}")
        );
        for (characters, special_key) in [("\u{7F}", 0x7F), ("\u{8}", 0x08)] {
            assert_eq!(
                press_of(key_event(characters, 0, Some(special_key))),
                action(RecorderAction::Blank),
                "{characters:?}"
            );
        }
        assert_eq!(
            press_of(key_event("\u{7F}", SHIFT, Some(0x7F))),
            refused(Wire::ReservedKey),
            "⇧⌫ is the reserved key, not the way to blank"
        );
    }

    /// The Escape KEY blurs whatever the layout types for it; an ESC typed
    /// by another key is refused like the reserved key it is
    /// (`ESCAPE_KEY_CODE`, `kVK_Escape`); ⌃[ (ESC under ⌃) is
    /// a chord on the reserved key.
    #[test]
    fn the_escape_key_blurs_and_an_escape_character_is_refused() {
        assert_eq!(
            press_of(with_key_code(key_event("\u{1B}", 0, None), ESCAPE_KEY_CODE)),
            action(RecorderAction::Blur)
        );
        assert_eq!(
            press_of(with_key_code(key_event("[", 0, None), ESCAPE_KEY_CODE)),
            action(RecorderAction::Blur),
            "a layout that types `[` on Escape still leaves"
        );
        assert_eq!(
            press_of(with_key_code(key_event("\u{1B}", 0, None), 0x00)),
            refused(Wire::ReservedKey)
        );
        // The deletes are read first: an Escape key a layout reports as
        // Delete or forward Delete blanks rather than blurs.
        for (characters, special_key) in [("\u{7F}", 0x7F), ("\u{F728}", 0xF728)] {
            assert_eq!(
                press_of(with_key_code(
                    key_event(characters, 0, Some(special_key)),
                    ESCAPE_KEY_CODE
                )),
                action(RecorderAction::Blank),
                "{characters:?}"
            );
        }
        assert_eq!(
            press_of(with_key_code(key_event("\u{1B}", CONTROL, None), 0x21)),
            refused(Wire::ReservedKey)
        );
    }

    /// The gate through the press: Tab and ⇧Tab (AppKit's back tab) record,
    /// a syllable letter is refused bare, Shift on the `3` and `;` keys is
    /// the digit / slot key whatever the layout types (Carbon codes 0x14 /
    /// 0x29), a `#` reached elsewhere records, no characters is no key.
    #[test]
    fn a_press_goes_through_the_gate() {
        assert_eq!(press_of(key_event("\t", 0, None)), recorded("|0009", "⇥"));
        assert_eq!(
            press_of(key_event("\u{19}", SHIFT, None)),
            recorded("s|0009", "⇧⇥")
        );
        assert_eq!(
            press_of(key_event("a", 0, None)),
            refused(Wire::TypesRomanization)
        );
        for (characters, carbon) in [("#", 0x14), (":", 0x29)] {
            assert_eq!(
                press_of(with_key_code(key_event(characters, SHIFT, None), carbon)),
                refused(Wire::TypesRomanization),
                "{characters}"
            );
        }
        assert_eq!(
            press_of(key_event("#", SHIFT, None)),
            recorded("s|0023", "⇧#")
        );
        assert_eq!(
            press_of(key_event("r", COMMAND | CONTROL | CAPS_LOCK, None)),
            recorded("dc|0072", "⌃⌘R")
        );
        let no_characters = KeyEvent {
            characters: None,
            characters_ignoring_modifiers: None,
            ..key_event("", CONTROL, None)
        };
        assert_eq!(press_of(no_characters), refused(Wire::NoKey));
        assert_eq!(
            press(&PressRequest { event: None }),
            Err(Refusal::Missing("event"))
        );
    }

    #[test]
    fn a_chord_is_made_through_the_gate() {
        let chord_of = |key: Option<&str>, modifier_flags: u64| {
            chord(&ChordRequest {
                key: key.map(str::to_owned),
                modifier_flags,
            })
            .answer
            .expect("an answer")
        };
        assert_eq!(
            chord_of(Some("\r"), SHIFT),
            chord_reply::Answer::Chord(Chord {
                raw: "s|000D".to_owned(),
                display: "⇧↩".to_owned(),
            })
        );
        assert_eq!(
            chord_of(Some("r"), COMMAND | CONTROL | CAPS_LOCK),
            chord_reply::Answer::Chord(Chord {
                raw: "dc|0072".to_owned(),
                display: "⌃⌘R".to_owned(),
            })
        );
        for (key, flags, reason) in [
            (Some("3"), SHIFT, Wire::TypesRomanization),
            (Some("\u{7F}"), 0, Wire::ReservedKey),
            (Some("\u{F702}"), OPTION, Wire::ReservedKey),
            (None, 0, Wire::NoKey),
            (Some(""), CONTROL, Wire::NoKey),
        ] {
            assert_eq!(
                chord_of(key, flags),
                chord_reply::Answer::Rejection(reason as i32),
                "{key:?}"
            );
        }
    }

    fn shortcuts(entries: Vec<crate::proto::SettingEntry>) -> ComposingShortcutsReply {
        composing_shortcuts(Some(&SettingsSnapshot { entries })).expect("answered")
    }

    /// trace: `ComposingAction::ALL` order and `GROUPS`; defaults
    /// ⇥ ⇧⇥ ] [ ↩ ⇧↩ Space (`action.rs` `default_chord`, `display(MacOS)`);
    /// the fixed rows are `shortcut_labels`' Mac arms.
    #[test]
    fn the_pane_reads_every_row_in_roster_order() {
        let reply = shortcuts(vec![]);
        let rows: Vec<(&str, u32, Option<&str>, &str)> = reply
            .actions
            .iter()
            .map(|row| {
                (
                    row.name.as_str(),
                    row.group,
                    row.chord.as_ref().map(|chord| chord.display.as_str()),
                    row.default_chord.as_ref().expect("a default").raw.as_str(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("nextCandidate", 0, Some("⇥"), "|0009"),
                ("previousCandidate", 0, Some("⇧⇥"), "s|0009"),
                ("pageForward", 0, Some("]"), "|005D"),
                ("pageBackward", 0, Some("["), "|005B"),
                ("confirmHighlighted", 1, Some("↩"), "|000D"),
                ("commitLiteral", 1, Some("⇧↩"), "s|000D"),
                ("commitAlternateScript", 1, Some("Space"), "|0020"),
            ]
        );
        let first = &reply.actions[0];
        assert_eq!(first.settings_key, "composingShortcut.nextCandidate");
        assert_eq!(first.label_key, "i18n_desktop_actionNextCandidate");
        assert_eq!(reply.slot_keys, "qwdfzxvy;");
        assert_eq!(reply.shifted_slot_keys, "⇧QWDFZXVY;");
        assert_eq!(reply.navigation_keys, "←  →  ↑  ↓  ⇞  ⇟");
        assert_eq!(reply.caret_chords, "⌥←  ⌥→");
        assert_eq!(reply.width_flip_chords, "⌃,  ⌃.  ⌃;");
        assert_eq!(reply.cancel_key, "⎋");
    }

    /// A cleared row reads empty, a recorded one as the snapshot holds it,
    /// and the slot rows follow the tone scheme.
    #[test]
    fn the_pane_reads_the_snapshot_it_carries() {
        let reply = shortcuts(vec![
            text("composingShortcut.nextCandidate", ""),
            text("composingShortcut.pageForward", "c|005D"),
            text("toneInputScheme", "telex"),
        ]);
        assert_eq!(reply.actions[0].chord, None);
        assert_eq!(
            reply.actions[2].chord,
            Some(Chord {
                raw: "c|005D".to_owned(),
                display: "⌃]".to_owned(),
            })
        );
        assert_eq!(reply.slot_keys, "123456789");
        assert_eq!(reply.shifted_slot_keys, "⇧123456789");
        assert_eq!(composing_shortcuts(None), Err(Refusal::Missing("settings")));
        assert!(matches!(
            composing_shortcuts(Some(&SettingsSnapshot {
                entries: vec![text("notASetting", "")],
            })),
            Err(Refusal::Settings(_))
        ));
    }

    fn picker(
        event: KeyEvent,
        entries: Vec<crate::proto::SettingEntry>,
    ) -> symbol_picker_reply::Intent {
        symbol_picker_key(
            &SymbolPickerKeyRequest { event: Some(event) },
            Some(&SettingsSnapshot { entries }),
        )
        .expect("answered")
        .intent
        .expect("an intent")
    }

    fn picker_action(action: SymbolPickerAction) -> symbol_picker_reply::Intent {
        symbol_picker_reply::Intent::Action(action as i32)
    }

    /// The picker's keys through the Mac's translation (`snapshot`): the
    /// bare Escape, an arrow by its `specialKey`, the slot keys of the
    /// snapshot's scheme, the bound rows, and a row recorded with ⌥.
    #[test]
    fn the_picker_reads_a_key_under_the_snapshot_it_carries() {
        use symbol_picker_reply::Intent;
        assert_eq!(
            picker(key_event("\u{1B}", 0, None), vec![]),
            picker_action(SymbolPickerAction::Close)
        );
        assert_eq!(
            picker(key_event("\u{F701}", 0, Some(0xF701)), vec![]),
            Intent::Navigate(WireNavigation::Down as i32)
        );
        assert_eq!(picker(key_event(";", 0, None), vec![]), Intent::PickSlot(8));
        assert_eq!(
            picker(
                key_event("9", 0, None),
                vec![text("toneInputScheme", "telex")]
            ),
            Intent::PickSlot(8)
        );
        assert_eq!(
            picker(key_event("\r", 0, Some(0x0D)), vec![]),
            picker_action(SymbolPickerAction::Confirm)
        );
        assert_eq!(
            picker(key_event("\r", SHIFT, Some(0x0D)), vec![]),
            picker_action(SymbolPickerAction::CloseAndPassThrough)
        );
        let option_paging = vec![text("composingShortcut.pageForward", "o|000D")];
        assert_eq!(
            picker(key_event("\r", OPTION, Some(0x0D)), option_paging.clone()),
            Intent::Navigate(WireNavigation::PageDown as i32)
        );
        assert_eq!(
            picker(key_event("]", 0, None), option_paging),
            picker_action(SymbolPickerAction::CloseAndPassThrough)
        );
        assert_eq!(
            symbol_picker_key(
                &SymbolPickerKeyRequest {
                    event: Some(key_event("q", 0, None)),
                },
                None
            ),
            Err(Refusal::Missing("settings"))
        );
        assert_eq!(
            symbol_picker_key(
                &SymbolPickerKeyRequest { event: None },
                Some(&SettingsSnapshot::default())
            ),
            Err(Refusal::Missing("event"))
        );
    }
}
