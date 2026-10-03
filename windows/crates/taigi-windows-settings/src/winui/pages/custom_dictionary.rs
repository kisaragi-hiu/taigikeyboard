//! Custom Dictionary: the words the user added themselves. Port of
//! `CustomDictionaryPage.swift`: rows fetched one PAGE at a time (10, so a
//! page never needs a scroller of its own), a filter that reloads once it
//! settles, a `ListView` whose selection drives the add / edit / delete
//! trio, the pager under it, CSV import/export, delete all, and the one
//! destructive verb for the learning records.
//!
//! The listing rules and every job body are
//! `taigi_desktop_core::settings::custom_dictionary`'s, shared with the
//! Linux pane; this file draws them and runs them. Every engine request —
//! the loads included — runs off the UI thread: the newest load wins by
//! generation, and the writes share the page's one work slot (refused, not
//! queued — a queued delete would name a row the list may no longer show).
//!
//! Named divergences: the Mac's double-click-to-edit and right-click menu
//! become an explicit ✎ button over the selection (Reactor's `ListView`
//! exposes neither a double-click nor a per-item flyout, and a button over
//! the selection is reachable from the keyboard, which neither was); and
//! the empty list says so in WORDS rather than the Mac's `tray` symbol —
//! Segoe Fluent Icons has no empty-container glyph, and a Windows 11 empty
//! state is a line of text.

use crate::winui::cards;
use crate::winui::list_pager::{self, icon_button};
use crate::winui::list_selection::{selectable_list, SettledRows};
use crate::winui::window::{Message as WindowMessage, SettingsWindow};
use std::path::Path;
use taigi_desktop_core::engine::user_data::{CustomDictionaryEntry, CustomDictionaryPage};
use taigi_desktop_core::settings::custom_dictionary::{
    clear_learning_records_job, delete_all_job, delete_entry_job, export_file_name, export_job,
    fetch, import_job, save_entry_job, Confirm, Listing,
};
use taigi_desktop_core::settings::keys;
use taigi_desktop_core::settings::listing::{
    JobOutcome, JobState, LoadLanded, FILTER_SETTLE, LOAD_DID_NOT_FINISH, OVERLAY_DELAY,
};
use taigi_desktop_core::settings::presentation::PageMessage;
use taigi_desktop_core::strings::{StringKey, StringResolver};
use windows_reactor::*;

/// A definite height, not a floor (`Metrics.tableHeight`): the page is
/// sized from the page size, so a short page keeps the controls under the
/// table where they were.
const TABLE_HEIGHT: f64 = 300.0;
const DIALOG_FIELD_WIDTH: f64 = 320.0;
/// Between the two columns, and between the small controls under the list.
const CONTROL_GAP: f64 = 8.0;
const TABLE_COLUMN_GAP: f64 = 12.0;
/// The header sits over the list's own item inset.
const TABLE_HEADER_INSET: f64 = 12.0;
const TABLE_HEADER_GAP: f64 = 8.0;
const OVERLAY_RING_SIZE: f64 = 20.0;
/// WinUI's secondary text, as opacity, so it follows the theme.
const SECONDARY_OPACITY: f64 = 0.65;

/// Which field of the entry dialog changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryField {
    Roman,
    Hanji,
}

#[derive(Clone)]
pub enum Message {
    FilterChanged(String),
    /// The filter stopped changing: reload from the first page.
    FilterSettled(u64),
    /// A page the user stepped to, already clamped by the caller.
    ShowPage(usize),
    Loaded(u64, Box<Result<CustomDictionaryPage, String>>),
    Select(Option<usize>),
    Add,
    Edit,
    Delete,
    EntryFieldChanged(EntryField, String),
    EntryDialogClosed(ContentDialogResult),
    Export,
    Import,
    /// A destructive command asking for its confirmation (`Confirm` says why
    /// it asks).
    Ask(Confirm),
    ConfirmClosed(ContentDialogResult),
    /// Confirmed — only [`Message::ConfirmClosed`] sends these.
    DeleteAll,
    ClearLearningRecords,
    JobFinished(u64, Box<JobOutcome>),
    /// The job at this generation has run long enough to say so.
    ShowBusy(u64),
    /// What the list on screen holds, as `list_selection` reports it.
    RowsApplied(Option<Vec<String>>),
}

/// The row being added or edited in the dialog.
struct EditingRow {
    original: CustomDictionaryEntry,
    roman: String,
    hanji: String,
}

#[derive(Default)]
pub struct CustomDictionaryModel {
    listing: Listing,
    is_first_load_requested: bool,
    /// Which rows the list on screen holds, so the selection index reaches
    /// XAML a render after the rows it counts do (`list_selection`).
    settled: SettledRows,
    editing: Option<EditingRow>,
    /// The destructive command waiting on its dialog, if any.
    confirming: Option<Confirm>,
    /// The page's one work slot: `Some` while a write runs. A file
    /// dialog does not need it — it is modal and runs on the UI thread,
    /// so no second command can arrive while it is up (the egui page had
    /// to reserve the slot across it because its dialog did not block the
    /// frame loop). `job` is the job it waits on and whether the overlay
    /// shows.
    job: JobState,
    next_job_generation: u64,
    /// What the running job is called, for the overlay.
    job_label: Option<StringKey>,
}

impl CustomDictionaryModel {
    fn is_working(&self) -> bool {
        self.job.is_running()
    }
}

/// Starts the first load, once, when the page is first shown.
pub fn ensure_loaded(
    model: &mut CustomDictionaryModel,
    context: &ComponentContext<SettingsWindow>,
) {
    if model.is_first_load_requested {
        return;
    }
    model.is_first_load_requested = true;
    load(model, context);
}

/// `alert` is the window's one notice: what a finished job has to say.
pub fn update(
    model: &mut CustomDictionaryModel,
    message: Message,
    alert: &mut Option<PageMessage>,
    context: &ComponentContext<SettingsWindow>,
) {
    match message {
        Message::FilterChanged(filter) => {
            let Some(generation) = model.listing.set_filter(filter) else {
                return;
            };
            // A wait the runtime will not start applies the filter AT ONCE
            // instead — a filter that never arrives would leave the list
            // showing rows the box no longer describes, and "applied
            // without waiting" is not a failure worth a banner.
            _ = context.spawn_background_with_rejection(
                move |_| {
                    std::thread::sleep(FILTER_SETTLE);
                    WindowMessage::CustomDictionary(Message::FilterSettled(generation))
                },
                WindowMessage::CustomDictionary(Message::FilterSettled(generation)),
            );
        }
        Message::FilterSettled(generation) => {
            if model.listing.settle(generation) {
                load(model, context);
            }
        }
        Message::ShowPage(page) => {
            model.listing.page = page;
            load(model, context);
        }
        Message::Loaded(generation, outcome) => {
            if let LoadLanded::Failed(notice) = model.listing.land(generation, *outcome) {
                *alert = Some(notice);
            }
        }
        Message::Select(index) => {
            // The index names a row of the list ON SCREEN, which a reload may
            // already have renumbered here — so it is resolved through the
            // rows XAML holds (`list_selection`). A cleared selection is not
            // acted on: single-select gives the user no way to clear one, so
            // the `-1` is the handover's own, and the row the user chose stays
            // chosen. A row a reload really dropped is cleared where that
            // reload lands (`Message::Loaded`).
            let Some(id) = index.and_then(|index| model.settled.key_at(index)) else {
                return;
            };
            model.listing.selected_id = Some(id.to_owned());
        }
        Message::Add => {
            model.editing = Some(EditingRow {
                original: CustomDictionaryEntry::default(),
                roman: String::new(),
                hanji: String::new(),
            });
        }
        Message::Edit => {
            if let Some(row) = model.listing.selected_row() {
                model.editing = Some(EditingRow {
                    roman: row.roman.clone(),
                    hanji: row.hanji.clone(),
                    original: row.clone(),
                });
            }
        }
        Message::Delete => {
            let Some(id) = model.listing.selected_id.clone() else {
                return;
            };
            begin_job(
                model,
                context,
                StringKey::DesktopProgressWorking,
                move || delete_entry_job(&id),
            );
        }
        Message::EntryFieldChanged(field, text) => {
            if let Some(editing) = model.editing.as_mut() {
                match field {
                    EntryField::Roman => editing.roman = text,
                    EntryField::Hanji => editing.hanji = text,
                }
            }
        }
        Message::EntryDialogClosed(result) => {
            let Some(editing) = model.editing.take() else {
                return;
            };
            if result != ContentDialogResult::Primary {
                return;
            }
            // A romanization is what the entry is found by; without one
            // there is nothing to store it under. The dialog's primary
            // button is disabled in that case, so this is the belt.
            if editing.roman.trim().is_empty() {
                return;
            }
            let EditingRow {
                original,
                roman,
                hanji,
            } = editing;
            begin_job(
                model,
                context,
                StringKey::DesktopProgressWorking,
                move || save_entry_job(&original.id, &roman, &hanji),
            );
        }
        Message::Export => export(model, context),
        Message::Import => import(model, context),
        Message::Ask(confirm) => model.confirming = Some(confirm),
        Message::ConfirmClosed(result) => {
            let confirmed = model.confirming.take().filter(|_| {
                // The primary button is the destructive one; Escape, the
                // close button and a dismissal all leave the store alone.
                result == ContentDialogResult::Primary
            });
            if let Some(confirm) = confirmed {
                let command = match confirm {
                    Confirm::DeleteAll => Message::DeleteAll,
                    Confirm::ClearLearningRecords => Message::ClearLearningRecords,
                };
                update(model, command, alert, context);
            }
        }
        Message::DeleteAll => begin_job(
            model,
            context,
            StringKey::DesktopProgressWorking,
            delete_all_job,
        ),
        Message::ClearLearningRecords => begin_job(
            model,
            context,
            StringKey::DesktopProgressWorking,
            clear_learning_records_job,
        ),
        Message::JobFinished(generation, outcome) => {
            if !model.job.finish(generation) {
                return;
            }
            model.job_label = None;
            let JobOutcome {
                message,
                is_reload_wanted,
            } = *outcome;
            if message.is_some() {
                *alert = message;
            }
            if is_reload_wanted {
                load(model, context);
            }
        }
        Message::ShowBusy(generation) => {
            model.job.show_busy(generation);
        }
        Message::RowsApplied(rows) => model.settled.report(rows),
    }
}

/// Starts a load of the page on screen. A load has no overlay: the rows
/// already on screen stay put while it runs.
fn load(model: &mut CustomDictionaryModel, context: &ComponentContext<SettingsWindow>) {
    let request = model.listing.begin_load();
    let generation = request.generation;
    _ = context.spawn_background_with_rejection(
        move |_| {
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fetch(&request)))
                    .unwrap_or_else(|_| Err(LOAD_DID_NOT_FINISH.to_owned()));
            WindowMessage::CustomDictionary(Message::Loaded(generation, Box::new(outcome)))
        },
        // A thread the runtime would not start is a failure the user sees,
        // never a load that quietly never lands.
        WindowMessage::CustomDictionary(Message::Loaded(
            generation,
            Box::new(Err("the load could not be started".to_owned())),
        )),
    );
}

/// Takes the page's one work slot for `job`, or does nothing because
/// something else holds it (the macOS model: refused, not queued).
fn begin_job(
    model: &mut CustomDictionaryModel,
    context: &ComponentContext<SettingsWindow>,
    label: StringKey,
    job: impl FnOnce() -> JobOutcome + Send + 'static,
) {
    if model.is_working() {
        return;
    }
    model.next_job_generation = model.next_job_generation.wrapping_add(1);
    let generation = model.next_job_generation;
    model.job.start(generation);
    model.job_label = Some(label);
    _ = context.spawn_background_with_rejection(
        move |_| {
            // A panicking store call must not leave the slot held for the
            // life of the window.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                .unwrap_or_else(|_| JobOutcome::did_not_finish::<CustomDictionaryEntry>());
            WindowMessage::CustomDictionary(Message::JobFinished(generation, Box::new(outcome)))
        },
        WindowMessage::CustomDictionary(Message::JobFinished(
            generation,
            Box::new(JobOutcome {
                message: Some(PageMessage::failure(
                    StringKey::DesktopCustomDictWriteFailed,
                    "the operation could not be started",
                )),
                is_reload_wanted: false,
            }),
        )),
    );
    // The overlay waits, so a millisecond-long write does not flash it.
    _ = context.spawn_background(move |_| {
        std::thread::sleep(OVERLAY_DELAY);
        WindowMessage::CustomDictionary(Message::ShowBusy(generation))
    });
}

fn export(model: &mut CustomDictionaryModel, context: &ComponentContext<SettingsWindow>) {
    if model.is_working() {
        return;
    }
    let suggested = export_file_name(&taigi_windows_platform::local_date());
    let Some(path) = crate::winui::file_dialog::save(&suggested) else {
        return;
    };
    begin_job(
        model,
        context,
        StringKey::DesktopProgressWorking,
        move || export_job(&path, write_atomically),
    );
}

fn import(model: &mut CustomDictionaryModel, context: &ComponentContext<SettingsWindow>) {
    if model.is_working() {
        return;
    }
    let Some(path) = crate::winui::file_dialog::open() else {
        return;
    };
    begin_job(
        model,
        context,
        StringKey::DesktopProgressWorking,
        move || import_job(&path),
    );
}

/// `Data.write(to:options:.atomic)`: the file appears whole or not at all,
/// and a failed export never damages the one it would have replaced.
fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("csv.{}.tmp", std::process::id()));
    std::fs::write(&temporary, contents).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, path).map_err(|error| {
        // The rename's error is the one worth reporting; a temporary file
        // that will not go is not what the user asked about.
        std::fs::remove_file(&temporary).ok();
        error.to_string()
    })
}

/// Segoe Fluent Icons: Add, Edit, Remove, and the two chevrons.
const ADD_GLYPH: &str = "\u{E710}";
const EDIT_GLYPH: &str = "\u{E70F}";
const REMOVE_GLYPH: &str = "\u{E738}";

pub fn view(
    window: &SettingsWindow,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
) -> View {
    let model = window.custom_dictionary();
    let enabled_row = cards::switch_row(
        strings.resolve(StringKey::DictionaryCustomDictEnabled),
        window.document().bool(&keys::IS_CUSTOM_DICT_ENABLED),
        true,
        context.callback(|is_on| WindowMessage::SetSwitch(keys::IS_CUSTOM_DICT_ENABLED, is_on)),
    );
    // No user-data directory: the stores never opened, and the banner at
    // the top of the window says so — nothing to list, nothing to write.
    if window.is_read_only() {
        return enabled_row;
    }
    // Mutual exclusion is the slot's; the greyed look waits the same
    // 400 ms as the overlay so a millisecond-long write does not flash it.
    let is_enabled = !model.job.is_busy_shown();
    View::fragment((
        enabled_row,
        cards::section_title_with_count(
            strings.resolve(StringKey::DesktopEntriesSection),
            &model.listing.count_label(),
        ),
        TextBox::new()
            .text(model.listing.filter().to_owned())
            .is_enabled(is_enabled)
            .placeholder_text(strings.resolve(StringKey::DictionarySearchPlaceholder))
            .on_text_changed(
                context
                    .callback(|text| WindowMessage::CustomDictionary(Message::FilterChanged(text))),
            ),
        entry_table(model, strings, context, is_enabled),
        cards::section_gap(),
        csv_row(strings, context, is_enabled),
        cards::action_row(
            strings.resolve(StringKey::DictionaryDeleteAll),
            strings.resolve(StringKey::CommonDelete),
            true,
            is_enabled,
            context
                .callback(|()| WindowMessage::CustomDictionary(Message::Ask(Confirm::DeleteAll))),
        ),
        cards::section_gap(),
        cards::action_row(
            strings.resolve(StringKey::DictionaryClearLearningRecords),
            strings.resolve(StringKey::CommonDelete),
            true,
            is_enabled,
            context.callback(|()| {
                WindowMessage::CustomDictionary(Message::Ask(Confirm::ClearLearningRecords))
            }),
        ),
        busy_overlay(model, strings),
        dialog(model, strings, context),
    ))
}

/// The entries in one card: the list, then the add / edit / delete trio
/// and the pager under it — as a list sits in one `ListView` surface on
/// Windows.
fn entry_table(
    model: &CustomDictionaryModel,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
    is_enabled: bool,
) -> View {
    let items = model
        .listing
        .rows
        .iter()
        .map(|row| {
            (
                row.id.clone(),
                ListViewItem::new().tag(row.id.clone()).content(
                    Grid::new()
                        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
                        .column_spacing(TABLE_COLUMN_GAP)
                        .children((
                            TextBlock::new()
                                .text(row.roman.clone())
                                .opacity(SECONDARY_OPACITY)
                                .grid_column(0),
                            TextBlock::new().text(row.hanji.clone()).grid_column(1),
                        )),
                ),
            )
        })
        .collect::<Vec<_>>();
    let has_selection = model.listing.selected_row().is_some();
    // The list itself stays live while a job runs: selecting a row writes
    // nothing, and the verbs over it are what a job turns off. The selection
    // index reaches XAML a render after the rows it counts (`list_selection`);
    // the verbs above stay on the stored selection.
    let list = selectable_list(
        "customDictionary.rows",
        &model.settled,
        items,
        model.listing.selected_index(),
        ListView::new()
            .selection_mode(ListViewSelectionMode::Single)
            .on_selection_changed(
                context.callback(|index| WindowMessage::CustomDictionary(Message::Select(index))),
            )
            .height(TABLE_HEIGHT),
        context,
        |rows| WindowMessage::CustomDictionary(Message::RowsApplied(rows)),
    );
    // OVER the list, not in place of it (`CustomDictionaryPage.swift`'s `.overlay`):
    // the columns and the controls under them stay put while a filter is
    // narrowed to nothing and widened again. The sentence is only ever there
    // when the list has no rows to press, and a `TextBlock` takes no focus —
    // a background-less `Border` is itself not hit-testable, but its child
    // still is, so this does not rely on the overlay being transparent.
    let list = Grid::new().children((list, Border::new().content(empty_state(model, strings))));
    // `cards::frame` stacks what it is given, so these three sit in its
    // panel with no spacing of its own — the header and the controls carry
    // their own `TABLE_HEADER_GAP` margins.
    cards::frame(View::fragment((
        // The column names, above the list rather than inside it: a
        // `ListView` has no header of its own.
        Grid::new()
            .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
            .column_spacing(TABLE_COLUMN_GAP)
            .margin(Thickness::new(
                TABLE_HEADER_INSET,
                0.0,
                0.0,
                TABLE_HEADER_GAP,
            ))
            .children((
                TextBlock::new()
                    .text(strings.resolve(StringKey::DictionaryRomanLabel))
                    .font_weight(FontWeight::SEMI_BOLD)
                    .grid_column(0),
                TextBlock::new()
                    .text(strings.resolve(StringKey::DictionaryHanziLabel))
                    .font_weight(FontWeight::SEMI_BOLD)
                    .grid_column(1),
            )),
        list,
        table_controls(model, strings, context, is_enabled, has_selection),
    )))
}

/// The add / edit / delete trio and the pager (`entryTableControls`).
fn table_controls(
    model: &CustomDictionaryModel,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
    is_enabled: bool,
    has_selection: bool,
) -> View {
    list_pager::bar(
        (
            icon_button(
                ADD_GLYPH,
                strings.resolve(StringKey::DictionaryAddEntry),
                is_enabled,
                context.callback(|()| WindowMessage::CustomDictionary(Message::Add)),
            ),
            icon_button(
                EDIT_GLYPH,
                strings.resolve(StringKey::DictionaryEditEntry),
                is_enabled && has_selection,
                context.callback(|()| WindowMessage::CustomDictionary(Message::Edit)),
            ),
            icon_button(
                REMOVE_GLYPH,
                strings.resolve(StringKey::CommonDelete),
                is_enabled && has_selection,
                context.callback(|()| WindowMessage::CustomDictionary(Message::Delete)),
            ),
        ),
        model.listing.page,
        model.listing.page_count(),
        is_enabled,
        strings,
        context,
        |page| WindowMessage::CustomDictionary(Message::ShowPage(page)),
    )
}

/// Import and export, side by side under the table.
fn csv_row(
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
    is_enabled: bool,
) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(CONTROL_GAP)
        .horizontal_alignment(HorizontalAlignment::Right)
        .margin(Thickness::new(0.0, 0.0, 0.0, CONTROL_GAP))
        .children((
            Button::new()
                .is_enabled(is_enabled)
                .on_click(context.callback(|()| WindowMessage::CustomDictionary(Message::Import)))
                .content(strings.resolve(StringKey::DictionaryImportCSV)),
            Button::new()
                .is_enabled(is_enabled)
                .on_click(context.callback(|()| WindowMessage::CustomDictionary(Message::Export)))
                .content(strings.resolve(StringKey::DictionaryExportCSV)),
        ))
}

/// The job's name over a ring, once it has run long enough to say so.
fn busy_overlay(model: &CustomDictionaryModel, strings: &StringResolver) -> View {
    let Some(label) = model.job_label.filter(|_| model.job.is_busy_shown()) else {
        return View::empty();
    };
    cards::frame(
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(CONTROL_GAP)
            .horizontal_alignment(HorizontalAlignment::Center)
            .children((
                ProgressRing::new()
                    .is_active(true)
                    .width(OVERLAY_RING_SIZE)
                    .height(OVERLAY_RING_SIZE),
                TextBlock::new()
                    .text(strings.resolve(label))
                    .vertical_alignment(VerticalAlignment::Center),
            )),
    )
}

/// What a list with no rows says. Two different things: an empty dictionary
/// is a STATE the + button answers, a filter matching nothing is a RESULT
/// of what the user typed (`CustomDictionaryPage.emptyState`).
///
/// Words rather than the Mac's `tray` symbol: Segoe Fluent Icons carries no
/// empty-container glyph, and a Windows 11 empty state is a line of text —
/// which also keeps the sentence a screen reader is told from being an
/// accessibility label bolted onto a picture.
fn empty_state(model: &CustomDictionaryModel, strings: &StringResolver) -> View {
    let Some(key) = model.listing.empty_state_key() else {
        return View::empty();
    };
    TextBlock::new()
        .text(strings.resolve(key))
        .text_wrapping(TextWrapping::Wrap)
        .opacity(SECONDARY_OPACITY)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

/// The one dialog the page can have up. An entry is being edited or a
/// destructive command is being confirmed — never both: the verbs that
/// start either are on the same page and only one of them can be pressed.
fn dialog(
    model: &CustomDictionaryModel,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
) -> View {
    if model.editing.is_some() {
        return entry_dialog(model, strings, context);
    }
    let Some(confirm) = model.confirming else {
        return View::empty();
    };
    ContentDialog::new()
        .title(strings.resolve(confirm.title_key()))
        .primary_button_text(strings.resolve(StringKey::CommonDelete))
        .close_button_text(strings.resolve(StringKey::CommonCancel))
        .is_open(true)
        .on_closed(
            context
                .callback(|result| WindowMessage::CustomDictionary(Message::ConfirmClosed(result))),
        )
        .content(match confirm.message_key() {
            Some(key) => TextBlock::new()
                .text(strings.resolve(key))
                .text_wrapping(TextWrapping::Wrap)
                .into(),
            None => View::empty(),
        })
}

/// Add or edit one entry (`CustomDictionaryEntrySheet`). Enter and Escape
/// are the dialog's own: `ContentDialog` gives the primary and close
/// buttons those keys, which is what the egui sheet hand-rolled.
fn entry_dialog(
    model: &CustomDictionaryModel,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
) -> View {
    let Some(editing) = model.editing.as_ref() else {
        return View::empty();
    };
    let is_new = editing.original.roman.is_empty();
    ContentDialog::new()
        .title(strings.resolve(if is_new {
            StringKey::DictionaryAddEntry
        } else {
            StringKey::DictionaryEditEntry
        }))
        .primary_button_text(strings.resolve(StringKey::CommonSave))
        .close_button_text(strings.resolve(StringKey::CommonCancel))
        // A romanization is what the entry is found by; without one there
        // is nothing to store it under.
        .is_primary_button_enabled(!editing.roman.trim().is_empty())
        .is_open(true)
        .on_closed(
            context.callback(|result| {
                WindowMessage::CustomDictionary(Message::EntryDialogClosed(result))
            }),
        )
        .content(
            StackPanel::new().spacing(CONTROL_GAP).children((
                TextBox::new()
                    .text(editing.roman.clone())
                    .width(DIALOG_FIELD_WIDTH)
                    .on_text_changed(context.callback(|text| {
                        WindowMessage::CustomDictionary(Message::EntryFieldChanged(
                            EntryField::Roman,
                            text,
                        ))
                    }))
                    .slot(
                        TextBoxSlot::Header,
                        strings.resolve(StringKey::DictionaryRomanLabel),
                    ),
                TextBox::new()
                    .text(editing.hanji.clone())
                    .width(DIALOG_FIELD_WIDTH)
                    .on_text_changed(context.callback(|text| {
                        WindowMessage::CustomDictionary(Message::EntryFieldChanged(
                            EntryField::Hanji,
                            text,
                        ))
                    }))
                    .slot(
                        TextBoxSlot::Header,
                        strings.resolve(StringKey::DictionaryHanziLabel),
                    ),
            )),
        )
}
