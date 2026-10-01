//! The Custom Dictionary pane's model, shared by both settings windows
//! (`CustomDictionaryPageModel` in `CustomDictionaryPage.swift`): which page
//! of which filter is on screen and which load put it there, the selection,
//! the destructive commands that ask first, and what each job answers. The
//! shells own the widgets, the timers, and the work slot a job runs in —
//! Windows holds one per page, Linux one per window so an outcome outlives a
//! page rebuilt under it.

use super::presentation::PageMessage;
use crate::engine::user_data::{
    self, CustomDictionaryEntry, CustomDictionaryPage, CustomDictionaryRefusal, UserDataError,
};
use crate::strings::StringKey;
use std::path::Path;
use std::time::Duration;

/// `CustomDictionaryPageModel.pageSize`.
pub const PAGE_SIZE: usize = 10;
/// `reloadWhenFilterSettles`: a word typed letter by letter is one query,
/// not six.
pub const FILTER_SETTLE: Duration = Duration::from_millis(200);
/// How long a job may run before the page says so (`overlayDelay`): a
/// millisecond-long write must not flash a spinner.
pub const OVERLAY_DELAY: Duration = Duration::from_millis(400);
/// The detail when a load's worker died before it answered.
pub const LOAD_DID_NOT_FINISH: &str = "the load did not finish";

/// A command that empties a store, waiting on its confirmation.
///
/// Confirmed rather than run on the press (the Mac does not ask): the
/// button that runs it is one row among the pane's, so the press is easy
/// to make by accident, and there is no undo — the ✎ / − verbs act on one
/// row, these two empty a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    DeleteAll,
    ClearLearningRecords,
}

impl Confirm {
    pub fn title_key(self) -> StringKey {
        match self {
            Self::DeleteAll => StringKey::DictionaryDeleteAll,
            Self::ClearLearningRecords => StringKey::DictionaryClearLearningRecords,
        }
    }

    /// The question under the title. `ClearLearningRecords` has none
    /// authored, and its title already asks it.
    pub fn message_key(self) -> Option<StringKey> {
        match self {
            Self::DeleteAll => Some(StringKey::DictionaryDeleteAllMessage),
            Self::ClearLearningRecords => None,
        }
    }
}

/// What a job hands back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobOutcome {
    pub message: Option<PageMessage>,
    /// Whether the list changed and must be reloaded.
    pub is_reload_wanted: bool,
}

impl JobOutcome {
    /// A job whose worker died: said, and the list reloaded in case the
    /// store changed before it did.
    pub fn did_not_finish() -> Self {
        Self {
            message: Some(PageMessage::failure(
                StringKey::DesktopCustomDictWriteFailed,
                "the operation did not finish",
            )),
            is_reload_wanted: true,
        }
    }
}

/// A write answers with nothing but its failure, and asks for a reload
/// either way.
fn write_outcome(result: Result<(), UserDataError>) -> JobOutcome {
    JobOutcome {
        message: result
            .err()
            .map(|error| PageMessage::failure(StringKey::DesktopCustomDictWriteFailed, error)),
        is_reload_wanted: true,
    }
}

/// Adds a word (`id` empty — the engine mints one) or edits the one with
/// `id`: an edit is an edit, not a new entry that happens to replace one.
/// Both fields are stored trimmed.
pub fn save_entry_job(id: &str, roman: &str, hanji: &str) -> JobOutcome {
    write_outcome(user_data::save_custom_entry(id, roman.trim(), hanji.trim()).map(|_| ()))
}

pub fn delete_entry_job(id: &str) -> JobOutcome {
    write_outcome(user_data::delete_custom_entry(id))
}

pub fn delete_all_job() -> JobOutcome {
    write_outcome(user_data::delete_all_custom_entries())
}

/// Imports a CSV file into the custom dictionary.
pub fn import_job(path: &Path) -> JobOutcome {
    let message = match user_data::import_custom_csv_file(path) {
        Ok(imported) => PageMessage::Imported {
            imported: imported.imported as usize,
            skipped: imported.skipped as usize,
        },
        Err(UserDataError::Refused {
            refusal: CustomDictionaryRefusal::NotUtf8,
            ..
        }) => PageMessage::NotUtf8,
        Err(error) => PageMessage::failure(StringKey::CommonImportFailed, error),
    };
    JobOutcome {
        message: Some(message),
        is_reload_wanted: true,
    }
}

/// Exports the WHOLE dictionary — not the page or the filter's matches —
/// through `write_file`, each window's own atomic write.
pub fn export_job(
    path: &Path,
    write_file: impl FnOnce(&Path, &[u8]) -> Result<(), String>,
) -> JobOutcome {
    let outcome = user_data::export_custom_csv()
        .map_err(|error| error.to_string())
        .and_then(|csv| write_file(path, &csv));
    JobOutcome {
        message: outcome
            .err()
            .map(|error| PageMessage::failure(StringKey::CommonExportFailed, error)),
        is_reload_wanted: false,
    }
}

/// Empties the three learning tables — three files, no transaction that
/// could span them; the engine attempts each even when an earlier one
/// fails, and the notice reports rather than claims. The custom dictionary
/// is untouched, so nothing reloads.
pub fn clear_learning_records_job() -> JobOutcome {
    let message = match user_data::clear_learning_records() {
        Ok(()) => PageMessage::Done(StringKey::DictionaryClearLearningRecordsDone),
        Err(error) => PageMessage::Failure {
            title: StringKey::DictionaryClearLearningRecordsFailed,
            detail: error.to_string(),
        },
    };
    JobOutcome {
        message: Some(message),
        is_reload_wanted: false,
    }
}

/// The name the export's save dialog suggests.
pub fn export_file_name(local_date: &str) -> String {
    format!("taigi_custom_dictionary_{local_date}.csv")
}

/// The page of the dictionary on screen and the filter that chose it.
#[derive(Debug, Default)]
pub struct Listing {
    pub rows: Vec<CustomDictionaryEntry>,
    /// Every entry, for the section header — what the dictionary HOLDS,
    /// which is not what the current filter matches.
    pub total_count: usize,
    /// How many entries the current filter matches; what the pager divides.
    pub match_count: usize,
    /// Which page is on screen, zero-based.
    pub page: usize,
    /// The row the list has selected, by ID — never by index, which moves
    /// under a reload.
    pub selected_id: Option<String>,
    /// The box as typed; written only through `set_filter`, which also
    /// makes every load in flight stale.
    filter: String,
    /// Which load the rows on screen came from: a load started under an
    /// older filter can come back after a newer one has, and would put rows
    /// on screen that do not match the box. The newest wins by number.
    load_generation: u64,
}

/// A load to run off the UI thread.
#[derive(Debug)]
pub struct LoadRequest {
    pub generation: u64,
    pub filter: String,
    pub page: usize,
}

impl LoadRequest {
    /// The page asked for. The engine answers the page it really served —
    /// pulled back inside the list if the list shrank under it (a delete on
    /// the last page, a filter that now matches less) — and `land` adopts
    /// that one.
    pub fn fetch(&self) -> Result<CustomDictionaryPage, String> {
        user_data::list_custom_page(&self.filter, self.page, PAGE_SIZE)
            .map_err(|error| error.to_string())
    }
}

/// What a finished load did to the listing.
#[derive(Debug, PartialEq, Eq)]
pub enum LoadLanded {
    /// A newer load or filter has started since; nothing changed.
    Stale,
    Adopted,
    /// Not an empty list: "empty" and "could not be read" look the same on
    /// screen, and only one is worth doing something about. The rows
    /// already shown stay; this is the notice to show.
    Failed(PageMessage),
}

impl Listing {
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The filter box changed. `None` when it did not really; otherwise the
    /// generation the settle timer carries — the reload waits for the box
    /// to settle.
    pub fn set_filter(&mut self, filter: String) -> Option<u64> {
        if filter == self.filter {
            return None;
        }
        self.filter = filter;
        Some(self.next_generation())
    }

    /// The settle timer for `generation` fired. `true` when the box has not
    /// moved since: the list reloads from the first page.
    pub fn settle(&mut self, generation: u64) -> bool {
        if generation != self.load_generation {
            return false;
        }
        self.page = 0;
        true
    }

    /// A load of the page on screen starts; every earlier one is now stale.
    pub fn begin_load(&mut self) -> LoadRequest {
        LoadRequest {
            generation: self.next_generation(),
            filter: self.filter.trim().to_owned(),
            page: self.page,
        }
    }

    /// The load started at `generation` came back.
    pub fn land(
        &mut self,
        generation: u64,
        outcome: Result<CustomDictionaryPage, String>,
    ) -> LoadLanded {
        if generation != self.load_generation {
            return LoadLanded::Stale;
        }
        match outcome {
            Ok(loaded) => {
                self.page = loaded.page;
                self.rows = loaded.rows;
                self.match_count = loaded.match_count;
                self.total_count = loaded.total_count;
                // A selection the new page does not hold is no selection:
                // the ✎ and − buttons must not act on a row that is not on
                // screen.
                if self.selected_index().is_none() {
                    self.selected_id = None;
                }
                LoadLanded::Adopted
            }
            Err(detail) => LoadLanded::Failed(PageMessage::failure(
                StringKey::DesktopCustomDictReadFailed,
                detail,
            )),
        }
    }

    fn next_generation(&mut self) -> u64 {
        self.load_generation = self.load_generation.wrapping_add(1);
        self.load_generation
    }

    pub fn selected_row(&self) -> Option<&CustomDictionaryEntry> {
        let id = self.selected_id.as_deref()?;
        self.rows.iter().find(|row| row.id == id)
    }

    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected_id.as_deref()?;
        self.rows.iter().position(|row| row.id == id)
    }

    /// Never fewer than one: an empty list is still page 1 of 1.
    pub fn page_count(&self) -> usize {
        self.match_count.div_ceil(PAGE_SIZE).max(1)
    }

    /// What the dictionary holds — and, while a filter narrows it, how much
    /// of that the filter matches. Against the counts, not against the
    /// filter box (`CustomDictionaryPage.swift:countLabel`): a filter that
    /// matches everything says nothing by saying "17000 / 17000".
    pub fn count_label(&self) -> String {
        if self.match_count < self.total_count {
            format!("{} / {}", self.match_count, self.total_count)
        } else {
            self.total_count.to_string()
        }
    }

    /// Which sentence the empty list shows, or `None` while there are rows:
    /// an empty dictionary is a STATE the + button answers, a filter
    /// matching nothing is a RESULT of what was typed
    /// (`CustomDictionaryPage.swift:378-396`).
    pub fn empty_state_key(&self) -> Option<StringKey> {
        if !self.rows.is_empty() {
            return None;
        }
        Some(if self.filter.is_empty() {
            StringKey::DictionaryCustomDictEmpty
        } else {
            StringKey::DictionaryNoResults
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(match_count: usize, total_count: usize, filter: &str) -> Listing {
        Listing {
            match_count,
            total_count,
            filter: filter.to_owned(),
            ..Listing::default()
        }
    }

    fn entry(id: &str) -> CustomDictionaryEntry {
        CustomDictionaryEntry {
            id: id.to_owned(),
            roman: "tsia̍h".to_owned(),
            hanji: "食".to_owned(),
            ..CustomDictionaryEntry::default()
        }
    }

    /// Page `number` of a dictionary holding `total_count` entries, of which
    /// the filter matches `match_count`.
    fn page(
        number: usize,
        ids: &[&str],
        match_count: usize,
        total_count: usize,
    ) -> CustomDictionaryPage {
        CustomDictionaryPage {
            page: number,
            rows: ids.iter().map(|id| entry(id)).collect(),
            match_count,
            total_count,
        }
    }

    /// Everything a load may change, to compare before and after.
    fn on_screen(listing: &Listing) -> (Vec<String>, usize, usize, usize, Option<String>) {
        (
            listing.rows.iter().map(|row| row.id.clone()).collect(),
            listing.page,
            listing.match_count,
            listing.total_count,
            listing.selected_id.clone(),
        )
    }

    /// A listing showing page 1 (`a`, `b`) of 12 matches out of 20, `a` selected.
    fn shown() -> Listing {
        let mut listing = Listing::default();
        let request = listing.begin_load();
        listing.land(request.generation, Ok(page(1, &["a", "b"], 12, 20)));
        listing.selected_id = Some("a".to_owned());
        listing
    }

    #[test]
    fn the_count_reads_as_one_number_until_a_filter_actually_narrows_it() {
        assert_eq!(listing(2, 2, "").count_label(), "2");
        assert_eq!(listing(2, 2, "tsia").count_label(), "2");
        assert_eq!(listing(1, 2, "tsia").count_label(), "1 / 2");
    }

    #[test]
    fn an_empty_list_says_which_kind_of_empty_it_is() {
        assert_eq!(
            listing(0, 0, "").empty_state_key(),
            Some(StringKey::DictionaryCustomDictEmpty)
        );
        assert_eq!(
            listing(0, 2, "zzz").empty_state_key(),
            Some(StringKey::DictionaryNoResults)
        );
        let mut listed = listing(1, 1, "");
        listed.rows.push(entry("a"));
        assert_eq!(listed.empty_state_key(), None, "rows say it themselves");
    }

    #[test]
    fn pages_are_ten_rows_and_never_fewer_than_one() {
        // trace: 0 → 1 page, 10 → 1, 11 → 2, 25 → 3.
        for (match_count, pages) in [(0, 1), (10, 1), (11, 2), (25, 3)] {
            assert_eq!(listing(match_count, match_count, "").page_count(), pages);
        }
    }

    #[test]
    fn both_destructive_commands_are_confirmed_and_only_one_asks_a_question() {
        assert_eq!(
            Confirm::DeleteAll.title_key(),
            StringKey::DictionaryDeleteAll
        );
        assert_eq!(
            Confirm::DeleteAll.message_key(),
            Some(StringKey::DictionaryDeleteAllMessage)
        );
        assert_eq!(
            Confirm::ClearLearningRecords.title_key(),
            StringKey::DictionaryClearLearningRecords
        );
        assert_eq!(Confirm::ClearLearningRecords.message_key(), None);
    }

    #[test]
    fn a_filter_that_did_not_change_starts_nothing_and_only_the_newest_settles() {
        let mut listing = Listing::default();
        assert_eq!(listing.set_filter(String::new()), None);
        let first = listing.set_filter("ts".to_owned()).unwrap();
        let second = listing.set_filter("tsia".to_owned()).unwrap();
        listing.page = 3;
        assert!(!listing.settle(first), "an older keystroke's timer");
        assert_eq!(listing.page, 3);
        assert!(listing.settle(second));
        assert_eq!(listing.page, 0, "a settled filter reloads from page one");
    }

    #[test]
    fn a_stale_load_changes_nothing() {
        let mut listing = shown();
        let before = on_screen(&listing);
        let old = listing.begin_load();
        let new = listing.begin_load();
        assert_eq!(
            listing.land(old.generation, Ok(page(0, &["c"], 1, 1))),
            LoadLanded::Stale
        );
        assert_eq!(on_screen(&listing), before);
        assert_eq!(
            listing.land(old.generation, Err("gone".to_owned())),
            LoadLanded::Stale,
            "a stale failure is not reported either"
        );
        // A keystroke after the load started makes it stale too.
        listing.set_filter("x".to_owned());
        assert_eq!(
            listing.land(new.generation, Ok(page(0, &["c"], 1, 1))),
            LoadLanded::Stale
        );
        assert_eq!(on_screen(&listing), before);
    }

    #[test]
    fn a_load_trims_the_filter_and_asks_for_the_page_on_screen() {
        let mut listing = Listing::default();
        listing.set_filter(" tsia ".to_owned());
        listing.page = 2;
        let request = listing.begin_load();
        assert_eq!(request.filter, "tsia");
        assert_eq!(request.page, 2);
    }

    #[test]
    fn an_adopted_page_is_the_engines_and_drops_an_off_page_selection() {
        let mut listing = shown();
        // The user asked for page 3; the engine pulled it back to page 0.
        listing.page = 3;
        let request = listing.begin_load();
        assert_eq!(
            listing.land(request.generation, Ok(page(0, &["b", "a"], 11, 19))),
            LoadLanded::Adopted
        );
        assert_eq!(
            on_screen(&listing),
            (
                vec!["b".to_owned(), "a".to_owned()],
                0,
                11,
                19,
                Some("a".to_owned())
            ),
            "an on-page selection stays, at its new index"
        );
        assert_eq!(listing.selected_index(), Some(1));
        let request = listing.begin_load();
        listing.land(request.generation, Ok(page(1, &["c", "d"], 11, 19)));
        assert_eq!(listing.selected_id, None);
    }

    #[test]
    fn a_failed_load_changes_nothing_on_screen() {
        let mut listing = shown();
        // The user stepped on before the failing load started.
        listing.page = 2;
        let before = on_screen(&listing);
        let request = listing.begin_load();
        assert_eq!(
            listing.land(request.generation, Err("disk".to_owned())),
            LoadLanded::Failed(PageMessage::failure(
                StringKey::DesktopCustomDictReadFailed,
                "disk"
            ))
        );
        assert_eq!(
            on_screen(&listing),
            before,
            "rows, counts, the page asked for and the selection all stay"
        );
    }

    #[test]
    fn a_blank_filter_is_kept_as_typed_and_reads_as_a_filter() {
        let mut listing = Listing::default();
        listing.set_filter(" ".to_owned());
        assert_eq!(listing.filter, " ");
        assert_eq!(listing.begin_load().filter, "");
        assert_eq!(
            listing.empty_state_key(),
            Some(StringKey::DictionaryNoResults)
        );
    }

    #[test]
    fn a_write_reports_only_its_failure_and_reloads_either_way() {
        assert_eq!(
            write_outcome(Ok(())),
            JobOutcome {
                message: None,
                is_reload_wanted: true,
            }
        );
        let refused = UserDataError::EngineUnavailable("customDictionarySave");
        assert_eq!(
            write_outcome(Err(refused.clone())),
            JobOutcome {
                message: Some(PageMessage::failure(
                    StringKey::DesktopCustomDictWriteFailed,
                    refused
                )),
                is_reload_wanted: true,
            }
        );
        assert!(JobOutcome::did_not_finish().is_reload_wanted);
    }

    #[test]
    fn the_export_suggests_a_dated_file_name() {
        assert_eq!(
            export_file_name("2026-09-30"),
            "taigi_custom_dictionary_2026-09-30.csv"
        );
    }
}
