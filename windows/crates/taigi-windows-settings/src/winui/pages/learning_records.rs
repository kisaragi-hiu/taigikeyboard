//! Learning Records: what the keyboard learned from the user's picks. The
//! Custom Dictionary page's shape (`custom_dictionary.rs`) over the engine's
//! learning stores: a kind picker (word frequency, phrases) and an order
//! picker over a filter, rows fetched one PAGE at a time (10), a list whose
//! selection drives the edit-count / delete pair, and the pager under it.
//! No add — a word the user wants is a custom word — and no wipe here: the
//! one destructive verb for every learning record stays on Custom
//! Dictionary.
//!
//! The listing rules and every job body are
//! `taigi_desktop_core::settings::learning_records`'s, shared with the
//! Linux pane; this file draws them and runs them off the UI thread, in the
//! page's one work slot (refused, not queued) — as Custom Dictionary does.
//! Deleting one row asks nothing, as deleting one custom word does not: the
//! keyboard learns the row again on the next pick.

use crate::winui::cards;
use crate::winui::list_pager::{self, icon_button};
use crate::winui::list_selection::{selectable_list, SettledRows};
use crate::winui::pages::choice_row;
use crate::winui::window::{Message as WindowMessage, SettingsWindow};
use taigi_desktop_core::engine::user_data::{
    LearningRecord, LearningRecordKind, LearningRecordOrder, LearningRecordPage,
};
use taigi_desktop_core::settings::learning_records::{
    count_note, delete_job, fetch, last_used_label, order_label, set_count_job, Listing, KINDS,
    ORDERS,
};
use taigi_desktop_core::settings::listing::{
    JobOutcome, JobState, LoadLanded, FILTER_SETTLE, LOAD_DID_NOT_FINISH, OVERLAY_DELAY,
};
use taigi_desktop_core::settings::presentation::PageMessage;
use taigi_desktop_core::strings::{StringKey, StringResolver};
use windows_reactor::*;

/// Custom Dictionary's table height: the page is sized from the page size.
const TABLE_HEIGHT: f64 = 300.0;
const DIALOG_FIELD_WIDTH: f64 = 320.0;
const CONTROL_GAP: f64 = 8.0;
const TABLE_COLUMN_GAP: f64 = 12.0;
/// The header sits over the list's own item inset.
const TABLE_HEADER_INSET: f64 = 12.0;
const TABLE_HEADER_GAP: f64 = 8.0;
const OVERLAY_RING_SIZE: f64 = 20.0;
/// WinUI's secondary text, as opacity, so it follows the theme.
const SECONDARY_OPACITY: f64 = 0.65;
/// Reading, word, count, last used: the two texts share most of the line,
/// the count and the day take what a number and `YYYY-MM-DD` need.
const TABLE_COLUMNS: [GridLength; 4] = [
    GridLength::Star(1.0),
    GridLength::Star(1.0),
    GridLength::Star(0.5),
    GridLength::Star(0.8),
];
/// The largest count the field offers; the engine clamps to the same.
const MAX_COUNT: f64 = 1_000_000.0;

#[derive(Clone)]
pub enum Message {
    /// `None` when a pop-up cleared its selection: nothing changes.
    ChooseKind(Option<(LearningRecordKind, StringKey)>),
    ChooseOrder(Option<LearningRecordOrder>),
    FilterChanged(String),
    /// The filter stopped changing: reload from the first page.
    FilterSettled(u64),
    /// A page the user stepped to, already clamped by the caller.
    ShowPage(usize),
    Loaded(u64, Box<Result<LearningRecordPage, String>>),
    Select(Option<usize>),
    Edit,
    Delete,
    /// The count field's value; `None` while it is cleared.
    CountChanged(Option<f64>),
    CountDialogClosed(ContentDialogResult),
    JobFinished(u64, Box<JobOutcome>),
    /// The job at this generation has run long enough to say so.
    ShowBusy(u64),
    /// What the list on screen holds, as `list_selection` reports it.
    RowsApplied(Option<Vec<String>>),
}

/// The row whose count is being edited in the dialog.
struct EditingCount {
    record: LearningRecord,
    /// What the field holds; `None` while it is cleared.
    count: Option<f64>,
}

pub struct LearningRecordsModel {
    listing: Listing,
    kind: LearningRecordKind,
    order: LearningRecordOrder,
    is_first_load_requested: bool,
    /// Which rows the list on screen holds, so the selection index reaches
    /// XAML a render after the rows it counts do (`list_selection`).
    settled: SettledRows,
    editing: Option<EditingCount>,
    /// The page's one work slot: the job it waits on, and whether the
    /// overlay shows.
    job: JobState,
    next_job_generation: u64,
}

impl Default for LearningRecordsModel {
    fn default() -> Self {
        Self {
            listing: Listing::default(),
            kind: KINDS[0].0,
            order: ORDERS[0],
            is_first_load_requested: false,
            settled: SettledRows::default(),
            editing: None,
            job: JobState::default(),
            next_job_generation: 0,
        }
    }
}

/// Starts the first load, once, when the page is first shown.
pub fn ensure_loaded(model: &mut LearningRecordsModel, context: &ComponentContext<SettingsWindow>) {
    if model.is_first_load_requested {
        return;
    }
    model.is_first_load_requested = true;
    load(model, context);
}

/// `alert` is the window's one notice: what a finished job has to say.
pub fn update(
    model: &mut LearningRecordsModel,
    message: Message,
    alert: &mut Option<PageMessage>,
    context: &ComponentContext<SettingsWindow>,
) {
    match message {
        Message::ChooseKind(Some((kind, _))) => {
            if kind != model.kind {
                model.kind = kind;
                show_from_first_page(model, context);
            }
        }
        Message::ChooseOrder(Some(order)) => {
            if order != model.order {
                model.order = order;
                show_from_first_page(model, context);
            }
        }
        Message::ChooseKind(None) | Message::ChooseOrder(None) => {}
        Message::FilterChanged(filter) => {
            let Some(generation) = model.listing.set_filter(filter) else {
                return;
            };
            // A wait the runtime will not start applies the filter at once
            // instead (Custom Dictionary's reasoning).
            _ = context.spawn_background_with_rejection(
                move |_| {
                    std::thread::sleep(FILTER_SETTLE);
                    WindowMessage::LearningRecords(Message::FilterSettled(generation))
                },
                WindowMessage::LearningRecords(Message::FilterSettled(generation)),
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
            // Resolved through the rows XAML holds, as on Custom Dictionary;
            // the key names the kind too, so a row of the kind just left is
            // never taken for this kind's row with the same id.
            let Some(key) = index.and_then(|index| model.settled.key_at(index)) else {
                return;
            };
            if let Some(row) = model.listing.rows.iter().find(|row| row_key(row) == key) {
                model.listing.selected_id = Some(row.id);
            }
        }
        Message::Edit => {
            if let Some(row) = model.listing.selected_row() {
                model.editing = Some(EditingCount {
                    count: Some(row.count.max(1) as f64),
                    record: row.clone(),
                });
            }
        }
        Message::Delete => {
            let Some(row) = model.listing.selected_row().cloned() else {
                return;
            };
            begin_job(model, context, move || delete_job(row));
        }
        Message::CountChanged(count) => {
            if let Some(editing) = model.editing.as_mut() {
                editing.count = count;
            }
        }
        Message::CountDialogClosed(result) => {
            let Some(EditingCount { record, count }) = model.editing.take() else {
                return;
            };
            if result != ContentDialogResult::Primary {
                return;
            }
            // The primary button is disabled while the field is cleared, so
            // this is the belt.
            let Some(count) = count.and_then(whole_count) else {
                return;
            };
            begin_job(model, context, move || set_count_job(record, count));
        }
        Message::JobFinished(generation, outcome) => {
            if !model.job.finish(generation) {
                return;
            }
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

/// Another kind or order: the first page of it, nothing selected, the
/// filter kept. The load it starts makes any older one stale.
fn show_from_first_page(
    model: &mut LearningRecordsModel,
    context: &ComponentContext<SettingsWindow>,
) {
    model.listing.page = 0;
    model.listing.selected_id = None;
    load(model, context);
}

/// The field's value as a count the engine takes: a whole number in
/// `1..=MAX_COUNT`. `None` for no number at all. The `NumberBox` coerces
/// to its bounds but not to a whole number, so a typed fraction rounds.
fn whole_count(value: f64) -> Option<i64> {
    value
        .is_finite()
        .then(|| value.round().clamp(1.0, MAX_COUNT) as i64)
}

/// The list item's key: the kind and the store's row id — ids are per
/// store, so the two kinds share them.
fn row_key(record: &LearningRecord) -> String {
    format!("{}:{}", record.kind, record.id)
}

/// Starts a load of the page on screen. A load has no overlay: the rows
/// already on screen stay put while it runs.
fn load(model: &mut LearningRecordsModel, context: &ComponentContext<SettingsWindow>) {
    let request = model.listing.begin_load();
    let generation = request.generation;
    let (kind, order) = (model.kind, model.order);
    _ = context.spawn_background_with_rejection(
        move |_| {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fetch(&request, kind, order)
            }))
            .unwrap_or_else(|_| Err(LOAD_DID_NOT_FINISH.to_owned()));
            WindowMessage::LearningRecords(Message::Loaded(generation, Box::new(outcome)))
        },
        // A thread the runtime would not start is a failure the user sees,
        // never a load that quietly never lands.
        WindowMessage::LearningRecords(Message::Loaded(
            generation,
            Box::new(Err("the load could not be started".to_owned())),
        )),
    );
}

/// Takes the page's one work slot for `job`, or does nothing because
/// something else holds it (Custom Dictionary's `begin_job`).
fn begin_job(
    model: &mut LearningRecordsModel,
    context: &ComponentContext<SettingsWindow>,
    job: impl FnOnce() -> JobOutcome + Send + 'static,
) {
    if model.job.is_running() {
        return;
    }
    model.next_job_generation = model.next_job_generation.wrapping_add(1);
    let generation = model.next_job_generation;
    model.job.start(generation);
    _ = context.spawn_background_with_rejection(
        move |_| {
            // A panicking store call must not leave the slot held for the
            // life of the window.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                .unwrap_or_else(|_| JobOutcome::did_not_finish::<LearningRecord>());
            WindowMessage::LearningRecords(Message::JobFinished(generation, Box::new(outcome)))
        },
        WindowMessage::LearningRecords(Message::JobFinished(
            generation,
            Box::new(JobOutcome {
                message: Some(PageMessage::failure(
                    StringKey::DictionaryLearningRecordsWriteFailed,
                    "the operation could not be started",
                )),
                is_reload_wanted: false,
            }),
        )),
    );
    // The overlay waits, so a millisecond-long write does not flash it.
    _ = context.spawn_background(move |_| {
        std::thread::sleep(OVERLAY_DELAY);
        WindowMessage::LearningRecords(Message::ShowBusy(generation))
    });
}

/// Segoe Fluent Icons: Edit, Remove.
const EDIT_GLYPH: &str = "\u{E70F}";
const REMOVE_GLYPH: &str = "\u{E738}";

pub fn view(
    window: &SettingsWindow,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
) -> View {
    let info = TextBlock::new()
        .text(strings.resolve(StringKey::DictionaryLearningRecordsInfo))
        .text_wrapping(TextWrapping::Wrap)
        .opacity(SECONDARY_OPACITY);
    // No user-data directory: the stores never opened, and the banner at
    // the top of the window says so — nothing to list, nothing to write.
    if window.is_read_only() {
        return info.into();
    }
    let model = window.learning_records();
    // Mutual exclusion is the slot's; the greyed look waits the same
    // 400 ms as the overlay so a millisecond-long write does not flash it.
    let is_enabled = !model.job.is_busy_shown();
    let current_kind = KINDS
        .iter()
        .copied()
        .find(|(kind, _)| *kind == model.kind)
        .unwrap_or(KINDS[0]);
    View::fragment((
        info,
        choice_row(
            strings.resolve(StringKey::DictionaryLearningRecords),
            &KINDS,
            current_kind,
            is_enabled,
            |(_, label)| strings.resolve(label).to_owned(),
            |kind| WindowMessage::LearningRecords(Message::ChooseKind(kind)),
            context,
        ),
        choice_row(
            strings.resolve(StringKey::DictionaryLearningRecordsOrder),
            &ORDERS,
            model.order,
            is_enabled,
            |order| strings.resolve(order_label(order)).to_owned(),
            |order| WindowMessage::LearningRecords(Message::ChooseOrder(order)),
            context,
        ),
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
                    .callback(|text| WindowMessage::LearningRecords(Message::FilterChanged(text))),
            ),
        record_table(model, strings, context, is_enabled),
        busy_overlay(model, strings),
        count_dialog(model, strings, context),
    ))
}

/// One row's cells, or the header's, on the table's four columns.
fn table_line(cells: [View; 4]) -> View {
    let [reading, word, count, last_used] = cells;
    Grid::new()
        .columns(TABLE_COLUMNS)
        .column_spacing(TABLE_COLUMN_GAP)
        .children((
            Border::new().grid_column(0).content(reading),
            Border::new().grid_column(1).content(word),
            Border::new().grid_column(2).content(count),
            Border::new().grid_column(3).content(last_used),
        ))
}

/// The records in one card: the column names, the list, then the edit /
/// delete pair and the pager under it (Custom Dictionary's `entry_table`).
fn record_table(
    model: &LearningRecordsModel,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
    is_enabled: bool,
) -> View {
    let items = model
        .listing
        .rows
        .iter()
        .map(|row| {
            let key = row_key(row);
            // The day in the user's calendar at the time it was used.
            let last_used = last_used_label(
                row.last_used_ms,
                taigi_windows_platform::utc_offset_seconds_at(row.last_used_ms),
            );
            (
                key.clone(),
                ListViewItem::new().tag(key).content(table_line([
                    TextBlock::new()
                        .text(row.tl.clone())
                        .opacity(SECONDARY_OPACITY)
                        .into(),
                    TextBlock::new().text(row.text.clone()).into(),
                    TextBlock::new().text(row.count.to_string()).into(),
                    TextBlock::new()
                        .text(last_used)
                        .opacity(SECONDARY_OPACITY)
                        .into(),
                ])),
            )
        })
        .collect::<Vec<_>>();
    let has_selection = model.listing.selected_row().is_some();
    // The list stays live while a job runs; the verbs are what a job turns
    // off (Custom Dictionary's reasoning, `list_selection`).
    let list = selectable_list(
        "learningRecords.rows",
        &model.settled,
        items,
        model.listing.selected_index(),
        ListView::new()
            .selection_mode(ListViewSelectionMode::Single)
            .on_selection_changed(
                context.callback(|index| WindowMessage::LearningRecords(Message::Select(index))),
            )
            .height(TABLE_HEIGHT),
        context,
        |rows| WindowMessage::LearningRecords(Message::RowsApplied(rows)),
    );
    // OVER the list, not in place of it, as on Custom Dictionary.
    let list = Grid::new().children((list, Border::new().content(empty_state(model, strings))));
    let header = |key| -> View {
        TextBlock::new()
            .text(strings.resolve(key))
            .font_weight(FontWeight::SEMI_BOLD)
            .into()
    };
    cards::frame(View::fragment((
        // The column names, above the list: a `ListView` has no header.
        Border::new()
            .margin(Thickness::new(
                TABLE_HEADER_INSET,
                0.0,
                0.0,
                TABLE_HEADER_GAP,
            ))
            .content(table_line([
                header(StringKey::DictionaryRomanLabel),
                header(StringKey::DictionaryHanziLabel),
                header(StringKey::DictionaryLearningRecordsCount),
                header(StringKey::DictionaryLearningRecordsLastUsed),
            ])),
        list,
        list_pager::bar(
            (
                icon_button(
                    EDIT_GLYPH,
                    strings.resolve(StringKey::DictionaryLearningRecordsEditCount),
                    is_enabled && has_selection,
                    context.callback(|()| WindowMessage::LearningRecords(Message::Edit)),
                ),
                icon_button(
                    REMOVE_GLYPH,
                    strings.resolve(StringKey::CommonDelete),
                    is_enabled && has_selection,
                    context.callback(|()| WindowMessage::LearningRecords(Message::Delete)),
                ),
            ),
            model.listing.page,
            model.listing.page_count(),
            is_enabled,
            strings,
            context,
            |page| WindowMessage::LearningRecords(Message::ShowPage(page)),
        ),
    )))
}

/// The job's name over a ring, once it has run long enough to say so.
fn busy_overlay(model: &LearningRecordsModel, strings: &StringResolver) -> View {
    if !model.job.is_busy_shown() {
        return View::empty();
    }
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
                    .text(strings.resolve(StringKey::DesktopProgressWorking))
                    .vertical_alignment(VerticalAlignment::Center),
            )),
    )
}

/// What a list with no rows says: nothing learned yet (a STATE) or a filter
/// matching nothing (a RESULT of what was typed).
fn empty_state(model: &LearningRecordsModel, strings: &StringResolver) -> View {
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

/// Edit one row's count: the word and its reading as the body, a
/// `NumberBox` for the count, and — for word frequency — the note that
/// counts past 40 rank the same. Escape is the dialog's own close key.
fn count_dialog(
    model: &LearningRecordsModel,
    strings: &StringResolver,
    context: &mut ViewContext<SettingsWindow>,
) -> View {
    let Some(editing) = model.editing.as_ref() else {
        return View::empty();
    };
    let record = &editing.record;
    let note = match LearningRecordKind::try_from(record.kind)
        .ok()
        .and_then(count_note)
    {
        Some(key) => TextBlock::new()
            .text(strings.resolve(key))
            .text_wrapping(TextWrapping::Wrap)
            .opacity(SECONDARY_OPACITY)
            .width(DIALOG_FIELD_WIDTH)
            .into(),
        None => View::empty(),
    };
    ContentDialog::new()
        .title(strings.resolve(StringKey::DictionaryLearningRecordsEditCount))
        .primary_button_text(strings.resolve(StringKey::CommonSave))
        .close_button_text(strings.resolve(StringKey::CommonCancel))
        .is_primary_button_enabled(editing.count.and_then(whole_count).is_some())
        .is_open(true)
        .on_closed(
            context.callback(|result| {
                WindowMessage::LearningRecords(Message::CountDialogClosed(result))
            }),
        )
        .content(
            StackPanel::new().spacing(CONTROL_GAP).children((
                TextBlock::new()
                    .text(format!("{}  {}", record.text, record.tl).trim().to_owned())
                    .text_wrapping(TextWrapping::Wrap)
                    .width(DIALOG_FIELD_WIDTH),
                NumberBox::new()
                    .minimum(1.0)
                    .maximum(MAX_COUNT)
                    .value(editing.count)
                    .width(DIALOG_FIELD_WIDTH)
                    .on_value_changed(context.callback(|count| {
                        WindowMessage::LearningRecords(Message::CountChanged(count))
                    }))
                    .slot(
                        NumberBoxSlot::Header,
                        strings.resolve(StringKey::DictionaryLearningRecordsCount),
                    ),
                note,
            )),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_count_field_answers_a_whole_number_inside_the_engines_range() {
        // trace: round, then clamp to 1..=1_000_000 (`set_count` clamps the
        // same, `engine/userdata`).
        assert_eq!(whole_count(25.0), Some(25));
        assert_eq!(whole_count(2.5), Some(3), "round half away from zero");
        assert_eq!(whole_count(0.2), Some(1), "never below one");
        assert_eq!(whole_count(5_000_000.0), Some(1_000_000));
        assert_eq!(whole_count(f64::NAN), None, "a cleared field");
    }

    #[test]
    fn a_row_key_tells_the_two_kinds_apart_at_the_same_id() {
        let frequency = LearningRecord {
            kind: LearningRecordKind::Frequency as i32,
            id: 7,
            ..LearningRecord::default()
        };
        let phrase = LearningRecord {
            kind: LearningRecordKind::LearnedPhrase as i32,
            ..frequency.clone()
        };
        assert_ne!(row_key(&frequency), row_key(&phrase));
    }
}
