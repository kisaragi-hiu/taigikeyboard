//! The words the user added themselves, and the keys that make them findable
//! from any romanization. Port of `Storage/CustomDictionaryStore.swift` +
//! `CustomDictionaryRow.swift`; SQL byte-identical.

use crate::database::{immediate_transaction, UserDataDatabase, UserDataDatabaseError};
use crate::timestamp::utc_timestamp_now;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use taigi_windows_core::composing::CustomDictionarySource;
use taigi_windows_core::engine::{CustomEntry, CustomSearchKey, LearnedPhrase};

const TABLE_NAME: &str = "custom_dictionary";
const SEARCH_KEY_TABLE_NAME: &str = "custom_search_key";
/// v4 (2026-09-18): search-key abbreviation face = leading spelling unit per
/// syllable (`behavioral-invariants.md` §46) — `rederive_search_keys_if_needed`
/// rebuilds every entry's keys once.
const SCHEMA_VERSION: i64 = 4;
/// One transaction per this many accepted rows, so a large import never
/// holds the write lock for its whole run. CROSS-PLATFORM INVARIANT —
/// mirrors iOS `CustomDictionaryRepository.swift:180`.
const IMPORT_CHUNK_SIZE: usize = 500;

/// How a stored roman becomes the keys it is findable under. Injected so a
/// test can drive the store without the engine, and because the shipped
/// implementation is an engine round-trip that must happen OUTSIDE the
/// write transaction.
pub type SearchKeyDeriver = Arc<dyn Fn(&str) -> Option<Vec<CustomSearchKey>> + Send + Sync>;

/// Why a custom-dictionary write did not happen.
#[derive(Debug, thiserror::Error)]
pub enum CustomDictionaryError {
    /// The dictionary already holds `limit` words and this would be one
    /// more. Editing an entry that is already there is never refused.
    #[error("custom dictionary is full (max {limit} entries)")]
    CapacityReached { limit: usize },
    /// The engine could not derive the search keys. The row is not written:
    /// an entry with no side keys is visible in the list and unreachable
    /// from the keyboard, which is worse than a refusal.
    #[error("could not derive search keys for {roman}")]
    SearchKeyDerivationFailed { roman: String },
    #[error(transparent)]
    Database(#[from] UserDataDatabaseError),
}

impl From<rusqlite::Error> for CustomDictionaryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(UserDataDatabaseError::Sqlite(error))
    }
}

/// What a CSV import did. `skipped` does not say why — a duplicate, the cap
/// and a rejected row all land in one bucket, as on iOS and Android.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CustomDictionaryImportResult {
    pub imported: usize,
    pub skipped: usize,
}

/// What makes two entries the same word to an import: the same word spelled
/// the same way (a file has no ids).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CustomDictionaryIdentity {
    pub roman: String,
    pub hanzi: String,
}

/// A word the user added: the romanization exactly as typed (TL or POJ
/// display form — nothing here folds it; the engine canonicalises per mode)
/// and the 漢字 it stands for, which may be empty. Timestamps are the stored
/// `yyyy-MM-dd HH:mm:ss` UTC text. CROSS-PLATFORM INVARIANT — the stored
/// shape mirrors iOS `CustomDictionaryEntry.swift:11-31` and Android's table.
/// Who wrote a row (`behavioral-invariants.md` §50); the raw values are the
/// stored `origin` column and the backup contract. CROSS-PLATFORM INVARIANT —
/// mirrors iOS `CustomDictionaryEntry.Origin` and Android `Entry.Origin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomDictionaryOrigin {
    /// Added or imported by the user.
    Manual = 0,
    /// Learned from a segment-by-segment continuous composition.
    Learned = 1,
}

impl CustomDictionaryOrigin {
    fn from_raw(raw: i64) -> Self {
        if raw == 1 {
            Self::Learned
        } else {
            Self::Manual
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomDictionaryRow {
    /// Stable across edits so the side table can be replaced rather than
    /// accumulated — a UUID, because editing either column keeps identity.
    pub id: String,
    pub roman: String,
    pub hanzi: String,
    pub created_at: String,
    pub updated_at: String,
    pub origin: CustomDictionaryOrigin,
    /// Times the phrase was composed or picked; `0` for a manual row.
    pub learn_count: i64,
}

impl CustomDictionaryRow {
    pub fn new(roman: &str, hanzi: &str) -> Self {
        Self::with_id(
            &uuid::Uuid::new_v4().to_string().to_uppercase(),
            roman,
            hanzi,
        )
    }

    pub fn with_id(id: &str, roman: &str, hanzi: &str) -> Self {
        let now = utc_timestamp_now();
        Self {
            id: id.to_owned(),
            roman: roman.to_owned(),
            hanzi: hanzi.to_owned(),
            created_at: now.clone(),
            updated_at: now,
            origin: CustomDictionaryOrigin::Manual,
            learn_count: 0,
        }
    }

    pub fn is_learned(&self) -> bool {
        self.origin == CustomDictionaryOrigin::Learned
    }

    pub fn identity(&self) -> CustomDictionaryIdentity {
        CustomDictionaryIdentity {
            roman: self.roman.clone(),
            hanzi: self.hanzi.clone(),
        }
    }
}

/// The user's own dictionary: reads it on the keystroke path, writes it
/// when the user asks.
pub struct CustomDictionaryStore {
    database: UserDataDatabase,
    derive_search_keys: SearchKeyDeriver,
    entry_limit: usize,
    learned_limit: usize,
}

impl CustomDictionaryStore {
    /// The MANUAL quota. CROSS-PLATFORM INVARIANT — mirrors iOS
    /// `CustomDictionaryCapacityPolicy.swift` `maxEntries` and Android `MAX_ENTRIES`.
    pub const MAX_ENTRIES: usize = 30_000;
    /// The LEARNED quota (§50): past it the fewest-composed, then least
    /// recently touched, row goes so a learn never fails. CROSS-PLATFORM
    /// INVARIANT — mirrors iOS `maxLearnedEntries` / Android `MAX_LEARNED_ENTRIES`.
    pub const MAX_LEARNED_ENTRIES: usize = 2_000;
    /// Largest `learn_count` a row can carry — a backup file is untrusted input.
    pub const MAX_LEARN_COUNT: i64 = 1_000_000;
    /// What the keystroke path is handed — the iOS call site's 20.
    pub const KEYSTROKE_LIMIT: usize = 20;
    /// Learned rows per fetch (exact whole-buffer match; homophone phrases).
    pub const LEARNED_KEYSTROKE_LIMIT: usize = 5;

    /// What a fresh install can find before the user has added anything.
    /// Ids included, so the same word is the same row on every platform
    /// (iOS `CustomDictionaryService.swift:22-25`).
    pub fn seed_entries() -> [CustomDictionaryRow; 2] {
        [
            CustomDictionaryRow::with_id("default-gau-tsa", "gâu-tsá", "𠢕早"),
            CustomDictionaryRow::with_id("default-tsiah-pa-bue", "tsia̍h-pá--buē", "食飽未"),
        ]
    }

    /// `entry_limit` is injectable ONLY so a test can reach the cap without
    /// writing 30000 rows.
    pub fn new(
        directory: PathBuf,
        derive_search_keys: SearchKeyDeriver,
        entry_limit: usize,
    ) -> Self {
        Self {
            database: UserDataDatabase::new(
                "custom_dictionary.db",
                "CustomDictionaryStore",
                directory,
                apply_schema,
            ),
            derive_search_keys,
            entry_limit,
            learned_limit: Self::MAX_LEARNED_ENTRIES,
        }
    }

    /// Injectable ONLY so a test can reach the learned cap without writing
    /// 2000 rows.
    pub fn with_learned_limit(mut self, learned_limit: usize) -> Self {
        self.learned_limit = learned_limit;
        self
    }

    pub fn is_ready(&self) -> bool {
        self.database.is_ready()
    }

    pub fn open(&self) {
        self.database.open();
    }

    pub fn open_blocking(&self) {
        self.database.open_blocking();
    }

    // Keystroke path

    /// The entries matching `query_key`, for the composition being typed.
    /// Synchronous and best-effort: a store that is not open answers `[]`
    /// rather than making the keystroke wait. `form IN (?, 'abbrev')` lets
    /// an abbreviation row satisfy a query in the same family; `DISTINCT`
    /// because one entry owns several side rows. CROSS-PLATFORM INVARIANT —
    /// mirrors iOS `CustomDictionaryRepository.swift:332-367`.
    pub fn rows_matching(
        &self,
        query_key: &CustomSearchKey,
        limit: usize,
    ) -> Vec<CustomDictionaryRow> {
        // `entry.origin = 0`: manual rows only (§50) — learned rows ride
        // `FetchAtPos.learned_entries` via `learned_rows_matching`, never
        // the custom-dictionary override.
        self.database
            .read(|connection| {
                let mut statement = connection.prepare_cached(&format!(
                    "SELECT DISTINCT {JOINED_ENTRY_COLUMNS}\nFROM {TABLE_NAME} AS entry\nJOIN {SEARCH_KEY_TABLE_NAME} AS search_key ON search_key.entry_id = entry.id\nWHERE entry.origin = 0\n  AND search_key.family = ?\n  AND search_key.form IN (?, 'abbrev')\n  AND search_key.key LIKE ? || '%'\nORDER BY entry.roman\nLIMIT ?;"
                ))?;
                let rows = statement.query_map(
                    params![query_key.family, query_key.form, query_key.key, limit as i64],
                    decode_row,
                )?;
                rows.collect()
            })
            .unwrap_or_default()
    }

    /// The LEARNED entries whose derived key EQUALS `query_key` — the whole
    /// typed buffer, not a prefix — for `FetchAtPos.learned_entries` (§50).
    /// Exact so a learned whole-buffer match never falls out of the prefix
    /// search's `LIMIT`. CROSS-PLATFORM INVARIANT — mirrors iOS
    /// `learnedExactSearchSQL` / Android `LEARNED_EXACT_SQL`.
    pub fn learned_rows_matching(
        &self,
        query_key: &CustomSearchKey,
        limit: usize,
    ) -> Vec<CustomDictionaryRow> {
        self.database
            .read(|connection| {
                let mut statement = connection.prepare_cached(&format!(
                    "SELECT {JOINED_ENTRY_COLUMNS}\nFROM {TABLE_NAME} AS entry\nJOIN {SEARCH_KEY_TABLE_NAME} AS search_key ON search_key.entry_id = entry.id\nWHERE entry.origin = 1\n  AND search_key.family = ?\n  AND search_key.form = ?\n  AND search_key.key = ?\nORDER BY entry.learn_count DESC, entry.updated_at DESC\nLIMIT ?;"
                ))?;
                let rows = statement.query_map(
                    params![query_key.family, query_key.form, query_key.key, limit as i64],
                    decode_row,
                )?;
                rows.collect()
            })
            .unwrap_or_default()
    }

    // Learned phrases (§50)

    /// Records one `Effect::PhraseLearned` (or one backup row, `count` > 1):
    /// inserts the `(hanzi, canonical TL)` pair as a learned row or adds
    /// `count` to its `learn_count`, in one statement on the learned-pair
    /// unique index. A manual row for the same pair wins — the learn is a
    /// no-op, never a downgrade. Best-effort and off the keystroke path: the
    /// keys are derived first (an FFI round-trip has no business holding the
    /// write lock), then the manual check, the upsert, the side keys and the
    /// eviction share one transaction; the row just written is never evicted.
    pub fn learn_phrase(&self, hanzi: &str, canonical_tl: &str, count: i64) {
        if hanzi.is_empty() || canonical_tl.is_empty() {
            return;
        }
        let Some(search_keys) =
            (self.derive_search_keys)(canonical_tl).filter(|keys| !keys.is_empty())
        else {
            return;
        };
        let count = count.clamp(1, Self::MAX_LEARN_COUNT);
        let limit = self.learned_limit;
        let hanzi = hanzi.to_owned();
        let canonical_tl = canonical_tl.to_owned();
        self.database.write(move |connection| {
            immediate_transaction::<_, rusqlite::Error>(connection, |connection| {
                if manual_row_exists(connection, &canonical_tl, &hanzi)? {
                    return Ok(());
                }
                let now = utc_timestamp_now();
                // `RETURNING id` answers with the row that took the write — the
                // fresh id on an insert, the existing id on a bump — so the side
                // keys are written for the right row either way.
                let row_id: String = connection.query_row(
                    &format!(
                        "INSERT INTO {TABLE_NAME} (id, roman, hanzi, created_at, updated_at, origin, learn_count)\nVALUES (?, ?, ?, ?, ?, 1, ?)\nON CONFLICT(hanzi, roman) WHERE origin = 1 DO UPDATE SET\n    learn_count = MIN(learn_count + excluded.learn_count, {}),\n    updated_at = excluded.updated_at\nRETURNING id;",
                        Self::MAX_LEARN_COUNT
                    ),
                    params![
                        uuid::Uuid::new_v4().to_string().to_uppercase(),
                        canonical_tl,
                        hanzi,
                        now,
                        now,
                        count
                    ],
                    |row| row.get(0),
                )?;
                replace_search_keys(connection, &row_id, &search_keys)?;
                evict_learned_past_cap(connection, limit, &row_id)
            })
        });
    }

    /// Bumps a learned row the user just committed as one candidate, so a
    /// phrase that is used stays ahead of the eviction line. No-op for a
    /// manual row or an unknown pair.
    pub fn touch_learned_phrase(&self, hanzi: &str, canonical_tl: &str) {
        if hanzi.is_empty() || canonical_tl.is_empty() {
            return;
        }
        let hanzi = hanzi.to_owned();
        let canonical_tl = canonical_tl.to_owned();
        self.database.write(move |connection| {
            connection.execute(
                &format!(
                    "UPDATE {TABLE_NAME}\nSET learn_count = MIN(learn_count + 1, {}), updated_at = ?\nWHERE origin = 1 AND hanzi = ? AND roman = ?;",
                    Self::MAX_LEARN_COUNT
                ),
                params![utc_timestamp_now(), hanzi, canonical_tl],
            )?;
            Ok(())
        });
    }

    // User-driven writes

    /// Every entry, newest edit first — the order the settings list shows.
    /// Unbounded, for the export.
    pub fn all_rows(&self) -> Result<Vec<CustomDictionaryRow>, CustomDictionaryError> {
        self.database
            .perform::<_, CustomDictionaryError>(|connection| {
                let mut statement = connection.prepare(&format!(
                    "SELECT {ENTRY_COLUMNS}\nFROM {TABLE_NAME}\nORDER BY updated_at DESC;"
                ))?;
                let rows = statement.query_map([], decode_row)?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            })
    }

    /// A page of entries for the settings list, newest edit first. The
    /// filter and the limit are both SQL. `LIKE` is case-insensitive for
    /// ASCII, which is the romanization; 漢字 have no case to fold.
    pub fn rows(
        &self,
        filter: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<CustomDictionaryRow>, CustomDictionaryError> {
        let trimmed = filter.trim().to_owned();
        self.database.perform::<_, CustomDictionaryError>(move |connection| {
            if trimmed.is_empty() {
                let mut statement = connection.prepare(&format!(
                    "SELECT {ENTRY_COLUMNS}\nFROM {TABLE_NAME}\nORDER BY updated_at DESC\nLIMIT ? OFFSET ?;"
                ))?;
                let rows = statement.query_map(params![limit as i64, offset as i64], decode_row)?;
                return Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?);
            }
            let pattern = format!("%{}%", escaped_for_like(&trimmed));
            let mut statement = connection.prepare(&format!(
                "SELECT {ENTRY_COLUMNS}\nFROM {TABLE_NAME}\nWHERE roman LIKE ? ESCAPE '\\' OR hanzi LIKE ? ESCAPE '\\'\nORDER BY updated_at DESC\nLIMIT ? OFFSET ?;"
            ))?;
            let rows = statement.query_map(params![pattern, pattern, limit as i64, offset as i64], decode_row)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn count(&self) -> Result<usize, CustomDictionaryError> {
        self.database
            .perform::<_, CustomDictionaryError>(|connection| Ok(entry_count(connection)?))
    }

    /// How many entries `filter` matches — the number the pager divides
    /// into pages. The same two predicates `rows` filters on.
    pub fn count_matching(&self, filter: &str) -> Result<usize, CustomDictionaryError> {
        let trimmed = filter.trim();
        if trimmed.is_empty() {
            return self.count();
        }
        let pattern = format!("%{}%", escaped_for_like(trimmed));
        self.database.perform::<_, CustomDictionaryError>(move |connection| {
            let count: i64 = connection.query_row(
                &format!("SELECT COUNT(*)\nFROM {TABLE_NAME}\nWHERE roman LIKE ? ESCAPE '\\' OR hanzi LIKE ? ESCAPE '\\';"),
                params![pattern, pattern],
                |row| row.get(0),
            )?;
            Ok(count as usize)
        })
    }

    /// Adds `row`, or replaces the one that already carries its id. The
    /// search keys are derived first, outside the transaction.
    pub fn upsert(&self, row: &CustomDictionaryRow) -> Result<(), CustomDictionaryError> {
        let search_keys = self.derived_keys(&row.roman)?;
        let limit = self.entry_limit;
        let row = row.clone();
        self.database
            .perform::<_, CustomDictionaryError>(move |connection| {
                immediate_transaction(connection, |connection| {
                    // A learned row adopted under its own id is a new manual
                    // row for the quota (`manual_entry_exists`).
                    if !manual_entry_exists(connection, &row.id)?
                        && entry_count(connection)? >= limit
                    {
                        return Err(CustomDictionaryError::CapacityReached { limit });
                    }
                    write_row(connection, &row, &search_keys)
                })
            })
    }

    /// Removes one entry. `false` means there was nothing with that id.
    pub fn delete(&self, id: &str) -> Result<bool, CustomDictionaryError> {
        let id = id.to_owned();
        self.database
            .perform::<_, CustomDictionaryError>(move |connection| {
                immediate_transaction(connection, |connection| {
                    let existed = entry_exists(connection, &id)?;
                    connection.execute(
                        &format!("DELETE FROM {TABLE_NAME} WHERE id = ?;"),
                        params![id],
                    )?;
                    delete_search_keys(connection, &id)?;
                    Ok(existed)
                })
            })
    }

    /// Empties the dictionary and reports how many entries went.
    pub fn delete_all(&self) -> Result<usize, CustomDictionaryError> {
        self.database
            .perform::<_, CustomDictionaryError>(|connection| {
                let removed =
                    immediate_transaction::<_, CustomDictionaryError>(connection, |connection| {
                        let existing = entry_count(connection)?;
                        connection.execute(&format!("DELETE FROM {TABLE_NAME};"), [])?;
                        connection.execute(&format!("DELETE FROM {SEARCH_KEY_TABLE_NAME};"), [])?;
                        Ok(existing)
                    })?;
                // Outside the transaction, best-effort: a file that stays large
                // is not a failed clear.
                connection.execute("VACUUM;", []).ok();
                Ok(removed)
            })
    }

    /// Rebuilds every entry's search keys when the stored database predates
    /// the current derivation (v2 → v3: the POJ `o͘` / ⁿ fix; v3 → v4: the
    /// leading-unit abbreviation face), then records the shape. An entry whose roman will not derive keeps its keys.
    pub fn rederive_search_keys_if_needed(&self) -> Result<(), CustomDictionaryError> {
        let stored: Option<Vec<(String, String)>> = self
            .database
            .perform::<_, CustomDictionaryError>(|connection| {
                let version: i64 =
                    connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
                if version >= SCHEMA_VERSION {
                    return Ok(None);
                }
                Ok(Some(entry_romans(connection)?))
            })?;
        let Some(stored) = stored else {
            return Ok(());
        };
        let mut derived_by_roman: HashMap<String, Vec<CustomSearchKey>> = HashMap::new();
        for (_, roman) in &stored {
            if derived_by_roman.contains_key(roman) {
                continue;
            }
            if let Some(keys) = (self.derive_search_keys)(roman).filter(|keys| !keys.is_empty()) {
                derived_by_roman.insert(roman.clone(), keys);
            }
        }
        self.database
            .perform::<_, CustomDictionaryError>(move |connection| {
                immediate_transaction(connection, |connection| {
                    // Re-read inside the transaction: an edit between the snapshot
                    // and this write already left current keys behind.
                    let current: HashMap<String, String> =
                        entry_romans(connection)?.into_iter().collect();
                    for (id, roman) in &stored {
                        if current.get(id) != Some(roman) {
                            continue;
                        }
                        if let Some(keys) = derived_by_roman.get(roman) {
                            replace_search_keys(connection, id, keys)?;
                        }
                    }
                    connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                    Ok(())
                })
            })
    }

    /// Writes the seed entries, but only into a dictionary nobody has
    /// touched — deleting one seed and relaunching must not bring it back.
    pub fn seed_if_empty(&self) -> Result<(), CustomDictionaryError> {
        let seeds: Vec<(CustomDictionaryRow, Vec<CustomSearchKey>)> = Self::seed_entries()
            .into_iter()
            .map(|row| self.derived_keys(&row.roman).map(|keys| (row, keys)))
            .collect::<Result<_, _>>()?;
        self.database
            .perform::<_, CustomDictionaryError>(move |connection| {
                immediate_transaction(connection, |connection| {
                    if entry_count(connection)? != 0 {
                        return Ok(());
                    }
                    for (row, keys) in &seeds {
                        write_row(connection, row, keys)?;
                    }
                    Ok(())
                })
            })
    }

    /// Imports parsed rows, skipping the ones already stored and stopping at
    /// the cap. Reaching the cap partway is not an error; a file whose own
    /// row count is over the cap is refused before anything is written. Both
    /// the cap and the duplicate check happen INSIDE each chunk's transaction.
    pub fn batch_import(
        &self,
        rows: &[CustomDictionaryRow],
    ) -> Result<CustomDictionaryImportResult, CustomDictionaryError> {
        let limit = self.entry_limit;
        if rows.len() > limit {
            return Err(CustomDictionaryError::CapacityReached { limit });
        }
        if rows.is_empty() {
            return Ok(CustomDictionaryImportResult {
                imported: 0,
                skipped: 0,
            });
        }
        let mut seen_in_file = HashSet::new();
        let candidates: Vec<&CustomDictionaryRow> = rows
            .iter()
            .filter(|row| seen_in_file.insert(row.identity()))
            .collect();
        let mut derived_by_roman: HashMap<&str, Vec<CustomSearchKey>> = HashMap::new();
        for row in &candidates {
            if !derived_by_roman.contains_key(row.roman.as_str()) {
                derived_by_roman.insert(&row.roman, self.derived_keys(&row.roman)?);
            }
        }
        let mut imported = 0;
        for chunk in candidates.chunks(IMPORT_CHUNK_SIZE) {
            // Paired here, owned: the worker runs this after the loop's
            // borrow of the file has to be over.
            let chunk: Vec<(CustomDictionaryRow, Vec<CustomSearchKey>)> = chunk
                .iter()
                .map(|row| ((*row).clone(), derived_by_roman[row.roman.as_str()].clone()))
                .collect();
            imported += self
                .database
                .perform::<_, CustomDictionaryError>(move |connection| {
                    immediate_transaction(connection, |connection| {
                        let mut stored_count = entry_count(connection)?;
                        let mut written = 0;
                        for (row, search_keys) in &chunk {
                            if stored_count >= limit {
                                break;
                            }
                            if manual_row_exists(connection, &row.roman, &row.hanzi)? {
                                continue;
                            }
                            write_row(connection, row, search_keys)?;
                            stored_count += 1;
                            written += 1;
                        }
                        Ok(written)
                    })
                })?;
        }
        Ok(CustomDictionaryImportResult {
            imported,
            skipped: rows.len() - imported,
        })
    }

    fn derived_keys(&self, roman: &str) -> Result<Vec<CustomSearchKey>, CustomDictionaryError> {
        (self.derive_search_keys)(roman)
            .filter(|keys| !keys.is_empty())
            .ok_or_else(|| CustomDictionaryError::SearchKeyDerivationFailed {
                roman: roman.to_owned(),
            })
    }
}

impl CustomDictionarySource for CustomDictionaryStore {
    fn rows_matching(&self, family: &str, form: &str, key: &str) -> Vec<CustomEntry> {
        let query = CustomSearchKey {
            family: family.to_owned(),
            form: form.to_owned(),
            key: key.to_owned(),
        };
        CustomDictionaryStore::rows_matching(self, &query, Self::KEYSTROKE_LIMIT)
            .into_iter()
            .map(|row| CustomEntry {
                roman: row.roman,
                hanzi: row.hanzi,
            })
            .collect()
    }

    fn learned_rows_matching(&self, family: &str, form: &str, key: &str) -> Vec<LearnedPhrase> {
        let query = CustomSearchKey {
            family: family.to_owned(),
            form: form.to_owned(),
            key: key.to_owned(),
        };
        CustomDictionaryStore::learned_rows_matching(self, &query, Self::LEARNED_KEYSTROKE_LIMIT)
            .into_iter()
            .map(|row| LearnedPhrase {
                hanzi: row.hanzi,
                canonical_tl: row.roman,
            })
            .collect()
    }

    fn learn_phrase(&self, hanzi: &str, canonical_tl: &str) {
        CustomDictionaryStore::learn_phrase(self, hanzi, canonical_tl, 1);
    }

    fn touch_learned_phrase(&self, hanzi: &str, canonical_tl: &str) {
        CustomDictionaryStore::touch_learned_phrase(self, hanzi, canonical_tl);
    }
}

/// `%` / `_` / `\` in user text, made literal for a `LIKE … ESCAPE '\'`.
fn escaped_for_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// The columns `decode_row` expects, in order; `JOINED_ENTRY_COLUMNS` is the
/// same list qualified for the side-table join.
const ENTRY_COLUMNS: &str = "id, roman, hanzi, created_at, updated_at, origin, learn_count";
const JOINED_ENTRY_COLUMNS: &str = "entry.id, entry.roman, entry.hanzi, entry.created_at, entry.updated_at, entry.origin, entry.learn_count";

fn manual_row_exists(connection: &Connection, roman: &str, hanzi: &str) -> rusqlite::Result<bool> {
    connection
        .query_row(
            &format!(
                "SELECT 1 FROM {TABLE_NAME} WHERE origin = 0 AND roman = ? AND hanzi = ? LIMIT 1;"
            ),
            params![roman, hanzi],
            |_| Ok(()),
        )
        .optional()
        .map(|found| found.is_some())
}

/// The MANUAL write (§50): the entry as the user's own word (`origin` 0, no
/// count — a learned `row` written here is adopted), its search keys, then
/// the takeover of any learned row for the same pair — after the write, so a
/// failed write leaves the learned row intact. Must be called inside a
/// transaction.
fn write_row(
    connection: &Connection,
    row: &CustomDictionaryRow,
    search_keys: &[CustomSearchKey],
) -> Result<(), CustomDictionaryError> {
    connection.execute(
        &format!(
            "INSERT INTO {TABLE_NAME} (id, roman, hanzi, created_at, updated_at, origin, learn_count)\nVALUES (?, ?, ?, ?, ?, 0, 0)\nON CONFLICT(id) DO UPDATE SET\n    roman = excluded.roman,\n    hanzi = excluded.hanzi,\n    updated_at = excluded.updated_at,\n    origin = 0,\n    learn_count = 0;"
        ),
        params![row.id, row.roman, row.hanzi, row.created_at, row.updated_at],
    )?;
    // Replace rather than add: an edited roman must not stay findable under
    // the keys of the roman it replaced.
    replace_search_keys(connection, &row.id, search_keys)?;
    remove_learned_row(connection, &row.roman, &row.hanzi, &row.id)?;
    Ok(())
}

/// Deletes the learned row (and its side keys) for `(roman, hanzi)` unless
/// it is `except_id` itself — the manual write that just landed takes it over.
fn remove_learned_row(
    connection: &Connection,
    roman: &str,
    hanzi: &str,
    except_id: &str,
) -> rusqlite::Result<()> {
    let learned = format!(
        "SELECT id FROM {TABLE_NAME} WHERE origin = 1 AND roman = ? AND hanzi = ? AND id <> ?"
    );
    connection.execute(
        &format!("DELETE FROM {SEARCH_KEY_TABLE_NAME} WHERE entry_id IN ({learned});"),
        params![roman, hanzi, except_id],
    )?;
    connection.execute(
        &format!("DELETE FROM {TABLE_NAME} WHERE id IN ({learned});"),
        params![roman, hanzi, except_id],
    )?;
    Ok(())
}

/// Drops learned rows past `cap`, never `kept_id` (the row just written).
/// Side keys first, so the subquery still resolves against the intact main
/// table; `OFFSET cap - 1` selects exactly the rows past the cap once the
/// kept row is set aside. CROSS-PLATFORM INVARIANT — mirrors iOS
/// `evictLearnedPastCap` / Android `LEARNED_PAST_CAP_SQL`.
fn evict_learned_past_cap(
    connection: &Connection,
    cap: usize,
    kept_id: &str,
) -> rusqlite::Result<()> {
    let past_cap = format!(
        "SELECT id FROM {TABLE_NAME} WHERE origin = 1 AND id <> ? ORDER BY learn_count DESC, updated_at DESC, id LIMIT -1 OFFSET ?"
    );
    let offset = cap.saturating_sub(1) as i64;
    connection.execute(
        &format!("DELETE FROM {SEARCH_KEY_TABLE_NAME} WHERE entry_id IN ({past_cap});"),
        params![kept_id, offset],
    )?;
    connection.execute(
        &format!("DELETE FROM {TABLE_NAME} WHERE id IN ({past_cap});"),
        params![kept_id, offset],
    )?;
    Ok(())
}

fn replace_search_keys(
    connection: &Connection,
    entry_id: &str,
    search_keys: &[CustomSearchKey],
) -> rusqlite::Result<()> {
    delete_search_keys(connection, entry_id)?;
    for search_key in search_keys {
        connection.execute(
            &format!("INSERT INTO {SEARCH_KEY_TABLE_NAME} (entry_id, family, form, key)\nVALUES (?, ?, ?, ?);"),
            params![entry_id, search_key.family, search_key.form, search_key.key],
        )?;
    }
    Ok(())
}

fn delete_search_keys(connection: &Connection, entry_id: &str) -> rusqlite::Result<()> {
    connection.execute(
        &format!("DELETE FROM {SEARCH_KEY_TABLE_NAME} WHERE entry_id = ?;"),
        params![entry_id],
    )?;
    Ok(())
}

fn entry_romans(connection: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement = connection.prepare(&format!("SELECT id, roman FROM {TABLE_NAME};"))?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

/// MANUAL rows only — the user's quota; learned rows have their own.
fn entry_count(connection: &Connection) -> rusqlite::Result<usize> {
    let count: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM {TABLE_NAME} WHERE origin = 0;"),
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// A MANUAL row with this id — a learned row being adopted under its own id
/// is still a new manual row for the quota.
fn manual_entry_exists(connection: &Connection, id: &str) -> rusqlite::Result<bool> {
    connection
        .query_row(
            &format!("SELECT 1 FROM {TABLE_NAME} WHERE id = ? AND origin = 0 LIMIT 1;"),
            params![id],
            |_| Ok(()),
        )
        .optional()
        .map(|found| found.is_some())
}

fn entry_exists(connection: &Connection, id: &str) -> rusqlite::Result<bool> {
    connection
        .query_row(
            &format!("SELECT 1 FROM {TABLE_NAME} WHERE id = ? LIMIT 1;"),
            params![id],
            |_| Ok(()),
        )
        .optional()
        .map(|found| found.is_some())
}

fn decode_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CustomDictionaryRow> {
    Ok(CustomDictionaryRow {
        id: row.get(0)?,
        roman: row.get(1)?,
        hanzi: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        origin: CustomDictionaryOrigin::from_raw(row.get(5)?),
        learn_count: row.get(6)?,
    })
}

/// The current shape, created directly. Deliberately does NOT stamp
/// `user_version`: creating tables says nothing about whether the ROWS were
/// derived by the current logic — `rederive_search_keys_if_needed` records
/// the shape once it has made the data match it.
fn apply_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE_NAME} (\n    id TEXT PRIMARY KEY,\n    roman TEXT NOT NULL,\n    hanzi TEXT NOT NULL,\n    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,\n    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP\n);\nCREATE INDEX IF NOT EXISTS idx_custom_roman ON {TABLE_NAME}(roman);\nCREATE TABLE IF NOT EXISTS {SEARCH_KEY_TABLE_NAME} (\n    entry_id TEXT NOT NULL,\n    family   TEXT NOT NULL,\n    form     TEXT NOT NULL,\n    key      TEXT NOT NULL\n);\nCREATE INDEX IF NOT EXISTS idx_csk_lookup ON {SEARCH_KEY_TABLE_NAME}(family, form, key);\nCREATE INDEX IF NOT EXISTS idx_csk_entry ON {SEARCH_KEY_TABLE_NAME}(entry_id);"
    ))?;
    // §50 provenance: `origin` (`CustomDictionaryOrigin`) and `learn_count`,
    // added by `ALTER` on a database created before them (every existing row
    // reads as manual), then the learned-pair unique index that makes learning
    // one atomic upsert. Idempotent. CROSS-PLATFORM INVARIANT — mirrors iOS
    // `CustomDictionarySchema` (`provenanceColumns`, `learnedPairIndexSQL`) and
    // Android v9.
    let present: HashSet<String> = connection
        .prepare(&format!("PRAGMA table_info({TABLE_NAME});"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    for column in ["origin", "learn_count"] {
        if !present.contains(column) {
            connection.execute(
                &format!(
                    "ALTER TABLE {TABLE_NAME} ADD COLUMN {column} INTEGER NOT NULL DEFAULT 0;"
                ),
                [],
            )?;
        }
    }
    connection.execute(
        &format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_custom_learned_pair ON {TABLE_NAME}(hanzi, roman) WHERE origin = 1;"
        ),
        [],
    )?;
    Ok(())
}
