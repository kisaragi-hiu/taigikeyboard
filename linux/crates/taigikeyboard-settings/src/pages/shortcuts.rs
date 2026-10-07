//! The Shortcuts pane: every key the user can put an action on, in three
//! titled groups (`ShortcutSettingsView.swift`, the Windows `shortcuts.rs`)
//! — Candidate Selection, the keys that move through the candidates; Output, the keys that
//! end the composition into the document; Other, the switches and the
//! lists a key raises — plus the reset row. Every recorder row is the same
//! field; which registry it writes to is the tier's. Last writer wins
//! across both registries (`ShortcutConflicts`), and the loser's row
//! visibly empties. The fixed rows show keys nothing records.

use super::PageContext;
use adw::prelude::*;
use taigi_desktop_core::keys::shortcut_labels::{
    cancel_key_label, caret_chords_label, navigation_keys_label, shifted_slot_keys_label,
    slot_keys_label,
};
use taigi_desktop_core::keys::{
    rejection_message_key, ComposingAction, ComposingKeyBindings, ComposingKeyChord,
    RecorderTarget, ShortcutAction,
};
use taigi_desktop_core::settings::SettingsDocument;
use taigi_desktop_core::strings::StringKey;
use taigi_linux_platform::DESKTOP_PLATFORM;

pub fn build<'a>(mut context: PageContext<'a>, page: &adw::PreferencesPage) -> PageContext<'a> {
    let bindings = ComposingKeyBindings::from_document(context.document, DESKTOP_PLATFORM);
    // Group one: through the candidates. The rows inside each group in
    // their roster's own order.
    let selection = adw::PreferencesGroup::builder()
        .title(
            context
                .strings
                .resolve(StringKey::DesktopShortcutSectionCandidateSelection),
        )
        .build();
    for action in ComposingAction::GROUPS[0] {
        recorder_row(&mut context, &selection, RecorderTarget::Composing(*action));
    }
    // Shown, not recordable: the bare slot keys follow the Tone Keys picker
    // (`ToneInputScheme`), the fixed navigation tier, the caret chord.
    let slot_row = fixed_row(
        &mut context,
        &selection,
        StringKey::DesktopShortcutSelectCandidateSlot,
        slot_keys_label(bindings.slot_key_set(), DESKTOP_PLATFORM),
    );
    fixed_row(
        &mut context,
        &selection,
        StringKey::DesktopShortcutNavigateCandidates,
        navigation_keys_label(DESKTOP_PLATFORM).to_owned(),
    );
    fixed_row(
        &mut context,
        &selection,
        StringKey::DesktopShortcutMoveComposingCaret,
        caret_chords_label(DESKTOP_PLATFORM),
    );
    page.add(&selection);

    // Group two: out of the composition and into the document.
    let output = adw::PreferencesGroup::builder()
        .title(
            context
                .strings
                .resolve(StringKey::DesktopShortcutSectionOutput),
        )
        .build();
    for action in ComposingAction::GROUPS[1] {
        recorder_row(&mut context, &output, RecorderTarget::Composing(*action));
    }
    // Not drawn under TPS, where a commit is always the Hanji.
    let shifted_slot_keys = shifted_slot_keys_label(bindings.slot_key_set(), DESKTOP_PLATFORM);
    let shifted_slot_row = fixed_row(
        &mut context,
        &output,
        StringKey::DesktopActionCommitAlternateScript,
        shifted_slot_keys.clone().unwrap_or_default(),
    );
    shifted_slot_row.set_visible(shifted_slot_keys.is_some());
    fixed_row(
        &mut context,
        &output,
        StringKey::DesktopShortcutCancelComposing,
        cancel_key_label(DESKTOP_PLATFORM).to_owned(),
    );
    page.add(&output);
    // The two slot-key rows follow the tone scheme and the input script
    // picked on General.
    context.on_refresh(move |document: &SettingsDocument| {
        let slot_keys =
            ComposingKeyBindings::from_document(document, DESKTOP_PLATFORM).slot_key_set();
        slot_row.set_subtitle(&slot_keys_label(slot_keys, DESKTOP_PLATFORM));
        let shifted_slot_keys = shifted_slot_keys_label(slot_keys, DESKTOP_PLATFORM);
        shifted_slot_row.set_visible(shifted_slot_keys.is_some());
        shifted_slot_row.set_subtitle(&shifted_slot_keys.unwrap_or_default());
    });

    // Group three: the switches, and the lists a key raises — the roster
    // that holds a chord in the global registry.
    let other = adw::PreferencesGroup::builder()
        .title(context.strings.resolve(StringKey::CommonOther))
        .build();
    for action in ShortcutAction::ALL {
        recorder_row(&mut context, &other, RecorderTarget::Global(action));
    }
    page.add(&other);

    // Both registries at once, and no conflict pass afterwards: the shipped
    // defaults hold no chord in common (`ShortcutSettingsView.swift`
    // `restoreDefaults`).
    context.reset_row(page, |document| {
        document.reset_composing_shortcuts();
        document.reset_global_shortcuts();
    });
    context
}

/// One recorder row: the field button, and a × beside a bound one. The
/// field shows the chord, or the prompt — or why the last press was
/// refused — while this row is the one recording.
fn recorder_row(
    context: &mut PageContext<'_>,
    group: &adw::PreferencesGroup,
    target: RecorderTarget,
) {
    let row = adw::ActionRow::builder()
        .title(context.strings.resolve(target.label_key()))
        .build();
    let field = gtk::Button::builder()
        .valign(gtk::Align::Center)
        .width_request(130)
        .build();
    let clear = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text(context.strings.resolve(StringKey::CommonDelete))
        .css_classes(["flat"])
        .build();
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    controls.append(&field);
    controls.append(&clear);
    row.add_suffix(&controls);
    row.set_activatable_widget(Some(&field));
    let shell = context.shell.clone();
    field.connect_clicked(move |button| shell.start_recording(target, Some(button.upcast_ref())));
    let shell = context.shell.clone();
    clear.connect_clicked(move |_| shell.clear_shortcut(target));
    group.add(&row);

    let shell = context.shell.clone();
    let strings = *context.strings;
    let refresh = move |document: &SettingsDocument| {
        let chord = chord_of(target, document);
        let (recording, rejection) = shell.recording();
        let is_recording = recording == Some(target);
        let label = if is_recording {
            strings
                .resolve(
                    rejection.map_or(StringKey::DesktopShortcutRecording, rejection_message_key),
                )
                .to_owned()
        } else {
            chord.as_ref().map_or_else(
                || {
                    strings
                        .resolve(StringKey::DesktopShortcutUnbound)
                        .to_owned()
                },
                |chord| chord.display(DESKTOP_PLATFORM),
            )
        };
        field.set_label(&label);
        // The accent is what a control wears while it takes input.
        if is_recording {
            field.add_css_class("suggested-action");
        } else {
            field.remove_css_class("suggested-action");
        }
        clear.set_visible(chord.is_some() && !is_recording);
    };
    refresh(context.document);
    context.on_refresh(refresh);
}

fn chord_of(target: RecorderTarget, document: &SettingsDocument) -> Option<ComposingKeyChord> {
    match target {
        RecorderTarget::Global(action) => action.chord_in(document, DESKTOP_PLATFORM),
        RecorderTarget::Composing(action) => {
            ComposingKeyBindings::from_document(document, DESKTOP_PLATFORM)
                .chord(action)
                .cloned()
        }
    }
}

/// One read-only row: the label, and the fixed keys as its subtitle — the
/// dim text is what tells it from the rows that record.
fn fixed_row(
    context: &mut PageContext<'_>,
    group: &adw::PreferencesGroup,
    title: StringKey,
    keys: String,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(context.strings.resolve(title))
        .subtitle(keys)
        .build();
    group.add(&row);
    row
}
