# Learning Records page — roadmap

> **Status**: in progress — P0 (this document). Requested by the maintainer 2026-10-03: "a page where users can view and edit learning records and ranking scores, for mobile and desktop".

A settings page, on all five platforms, that lists what the keyboard has learned from the user, lets them correct one row's count and delete one row. Today the only control is "Delete Learning Records", which empties every learning store at once.

## Today (grounded in code)

| Fact | Source |
|---|---|
| Three learning stores, engine-owned: word frequency `(word, tl, count, last_used)` cap 20 000; next-word association `(prev_word, prev_tl, next_word, next_tl, count, last_used)` cap 50 000; learned phrases `(hanzi, roman, learn_count, updated_at)` cap 2 000 | `engine/userdata/src/frequency.rs:13-50`, `association.rs:13-64`, `learned_phrases.rs:14-48` |
| A frequency row feeds ranking as `(boost − 1) × decay`: `boost = min(1 + count × 0.1, 5.0)` (saturates at count 40), decay over `last_used` with τ = 30 days. An internal weight, not a number any UI shows | `engine/ranking/src/score.rs:39-47`, `:196`, `:218`, `:258-269` |
| An association row's count has no such ceiling and its decay has a floor, so a raised count on an old row still ranks | `engine/nextword/src/scorer.rs:25-32`, `:63-71` |
| A learned phrase's `learn_count` decides which five rows a whole-buffer match returns and which row is evicted first; it is not handed to candidate ranking | `learned_phrases.rs:156`, `:232-247`, `engine/dispatch/src/user_data/with_stores.rs:274-278` |
| Frequency and association eviction also order by count, then time | `engine/userdata/src/capacity.rs:81` |
| No request lists, edits or deletes ONE learning row: the stores expose `all_rows` (tests / backup) and `delete_all` only; `UserDataRequest` has 11 methods, the page ones all custom-dictionary | `engine/protos/proto/user_data.proto:17-29`, `engine/userdata/src/requests.rs:44-86` |
| "Delete Learning Records" is a row on each Custom Dictionary page and sends `ResetUserData {frequency, association, learned_phrases}` | iOS `CustomDictionaryView.swift:101-108`, Android `CustomDictionaryScreen.kt:296-306`, macOS `CustomDictionaryPage.swift:288-292`, desktop `settings/custom_dictionary.rs:146` |
| Custom Dictionary page shape per platform — the style this page copies: iOS / Android load every row and show at most 100, bottom search bar, swipe / icon delete, alert / dialog edit; macOS `Form` + paged `Table` (10 rows) + `UserDataPageChrome`; Windows / Linux share `desktop-core` `settings::custom_dictionary::Listing` (paged, settled filter, one job slot) | iOS `CustomDictionaryView.swift:6-240`, Android `CustomDictionaryScreen.kt:86-441`, macOS `CustomDictionaryPage.swift:21-442` + `UserDataPageChrome.swift`, `desktop/crates/taigi-desktop-core/src/settings/custom_dictionary.rs:18-209`, Windows `pages/custom_dictionary.rs`, Linux `pages/custom_dictionary.rs` |
| The desktop has no next-word prediction; its association store is write-only | `association.rs:46-48` |

## Design

### What the user gets

One page, **Learning Records**, with a kind switch:

| Kind | Row shows | Editable | Platforms |
|---|---|---|---|
| Word frequency (the ranking score) | Hanji / word, TL reading, count, last used | count; delete row | all five |
| Learned phrases | Hanji, TL reading, count | count; delete row | all five |
| Next-word association | previous word → next word, count | count; delete row | iOS, Android (the desktop predicts no next word, so the rows rank nothing there) |

- **The count is what the user edits.** The engine's boost / decay maths stays where it is. For word frequency only, the edit dialog says that counts above 40 rank the same; the other two kinds make no such promise.
- **Search** filters by Hanji or romanization substring, in SQL, like the Custom Dictionary filter.
- **Order**: most used first (default) or most recent first — "the word I just picked by mistake" is the row a user comes to delete.
- **No add.** A word the user wants is a custom word; this page corrects what was learned.
- **"Delete Learning Records" stays where it is** on the Custom Dictionary page. Moving it is a separate decision.

### Engine (one implementation, every platform)

New `UserDataRequest` methods (tags 12–14), answered by `engine/userdata`:

```proto
enum LearningRecordKind { FREQUENCY = 0; LEARNED_PHRASE = 1; ASSOCIATION = 2; }
enum LearningRecordOrder { MOST_USED = 0; MOST_RECENT = 1; }

message LearningRecord {
  LearningRecordKind kind = 1;
  int64 id = 2;              // the store's row id — a page handle, not identity
  string text = 3;           // word / phrase Hanji / next word
  string tl = 4;             // canonical TL of `text`
  string previous_text = 5;  // association only
  string previous_tl = 6;    // association only
  int64 count = 7;
  int64 last_used_ms = 8;
}
message ListLearningRecords { kind, filter, limit, offset, order }   → LearningRecords { records, total, matching_total, offset }
message SetLearningRecordCount { record, count }                     → LearningRecordSaved { optional record }
message DeleteLearningRecord { record }                              → LearningRecordDeleted { removed }
```

- Paging contract = `ListCustomEntries`: the offset is pulled back to the last page that exists (`engine/userdata/src/paging.rs`, shared). Unlike it, `total`, `matching_total`, the offset clamp and the rows are read in ONE read transaction (the page statement counts both itself: one scan per page), `limit` 0 is refused (50 000 rows never travel in one answer), and every order ends in `id` so two pages never share or skip a row at a tie.
- **Identity guard.** A mutation carries the whole `LearningRecord` the page listed; the SQL matches `id` AND the row's identity columns (`(word, tl)`; `(hanzi, roman)`; both association pairs). `learned_phrases.id` is a plain `INTEGER PRIMARY KEY`, so a deleted id can be reused — a stale dialog must not edit the phrase that took it. No match (evicted, deleted, reused) answers "no record" / `removed = false`, which the page reports as "this record is gone" and reloads.
- `SetLearningRecordCount` clamps to `1..=1 000 000` (the learned-phrase ceiling, `learned_phrases.rs:43`) and keeps `last_used` — an edit is not a use. One `UPDATE … SET count = ? … RETURNING` statement.
- **Concurrent keyboard writes**: each process has its own writer queue (`database.rs:358-385`), so the order is the database's commit order — a set overwrites the increments before it, later picks add to it, and a deleted row is learned again on the next pick. No cross-process flush protocol.
- Deleting a learned phrase deletes its search keys in the same transaction. No `VACUUM` per row.
- The filter escapes `%` / `_` / `\` as the custom dictionary's does (`custom_dictionary.rs:520-524`); a NULL association TL lists as `''` and the guard compares `COALESCE(tl, '')`.
- Word identity stays the `(Hanji, canonical TL)` pair (Core Principle #6): the row id only addresses a row the page already listed; no lookup, dedup or merge keys on it.
- Unknown `kind` / `order` values are refused (`FAIL_INVARIANT`, as every refused user-data request).
- Backup format unchanged.

### Platforms

| Platform | Entry | Page |
|---|---|---|
| iOS | Dictionary tab → "Data management", under Custom Dictionary (`DictionaryTab.swift:80-90`) | `LearningRecordsView` + view model over `UserDataClient`; segmented kind picker, list, bottom `SearchBar`, swipe delete, alert edit — the Custom Dictionary idiom, but engine-paged (100 rows, load more at the end) with the filter sent to the engine |
| Android | Dictionary settings → "Data management" (`DictionarySettingsScreen.kt:137-141`) | `LearningRecordsActivity` / `Screen` / `ViewModel`; `SettingsCard` rows, `FilterSearchBar`, dialog edit; same engine paging as iOS |
| macOS | sidebar pane after Custom Dictionary (`SettingsSplitView.swift:13`) | `LearningRecordsPage` on `UserDataPageChrome` (`UserDataFilterField`, `Table`, `UserDataListPager`) |
| Windows / Linux | sidebar pane after Custom Dictionary (`settings/choices.rs:316`) | shared model `desktop-core/src/settings/learning_records.rs` (reusing the `Listing` paging / settle logic) + one page file per toolkit |

Every page: a change of kind, order or filter invalidates a load still in flight; "record is gone", "could not read" and "nothing learned yet" are three different states.

### Not colliding with macOS-over-desktop-core

That track has one phase left, P15: rewriting `*.swift` cites in Rust comments plus `system-overview.md` / `AGENTS.md` (`macos-desktop-core-roadmap.md:198`). This work therefore:

- puts the request handling and the page model in **new files** (`engine/userdata/src/learning_records.rs`, `desktop-core/src/settings/learning_records.rs`); each store file gains only a table descriptor (`LEARNING_TABLE`) beside its schema, without moving or rewording any line that carries a Swift cite; the branch is rebased and re-diffed once P15 merges;
- adds no `*.swift` cite to any Rust comment;
- leaves the macOS key path, `taigi-macos-ffi` and `CoreComposingBackend` alone — the macOS page uses the Swift user-data client the Custom Dictionary page already uses (`RustEngineBridge.swift:93-95`: the user-data slice is the one macOS sends itself).

## Phases

| Phase | Scope | Status |
|---|---|---|
| P0 | this roadmap | Done |
| P1 | engine: proto + store methods + `learning_records.rs` + routing; store tests (id reuse, two connections, phrase keys, paging, NULL TL) + dispatch tests; regenerated Android Java / iOS Swift / macOS Swift protos in the same PR | In review |
| P2 | i18n keys with every generated output (incl. `ios/Localizable.xcstrings`) + `desktop-core` page model + Linux page; gate = every platform in the keys' scope | Pending |
| P3 | Windows page | Pending |
| P4 | macOS page | Pending |
| P5 | iOS page | Pending |
| P6 | Android page | Pending |

## Best practices alignment

Per phase: P1 — `docs/contributing/rust-migration-policy.md` §6 (user data is engine-owned), AGENTS.md "Shared logic lives in the engine", Core Principle #6; P2–P6 — each platform's guide in `docs/contributing/`, `docs/contributing/i18n.md`, `docs/contributing/cross-platform-alignment.md`.

| Mainstream practice | Source | This plan |
|---|---|---|
| Auto-learned data is kept apart from the user's own words | `moe_taigi_apk` UserVoc / LearnedVoc (`docs/references/mainstream-ime-comparison.md:89`, `:305`) | own page beside Custom Dictionary, never mixed into it (all phases) |
| A learned entry carries a count and a last-used time; rank = count with time decay | `references/librime/src/rime/dict/user_dictionary.cc` `c= d= t=` (`mainstream-ime-comparison.md:80`, `:87`) | the page shows and edits exactly those two stored facts; the formula is not duplicated in any UI (P1) |
| Bounded learning store, fewest-selections-then-least-recent eviction | ChiaKey `Manjusri/Headers/LanguageModel.h` (`mainstream-ime-comparison.md:90`) | a count edit keeps `last_used`, so eviction order only moves through the count the user set (P1) |

**Deliberately not adopted**

- A suppression / blocklist dictionary (mozc `user_dictionary.cc`, `mainstream-ime-comparison.md:364`): deleting the learned row is enough to undo a mistaken pick; a "never show" list is a different feature.
- Showing the computed boost: it changes with the clock (30-day decay); a number that drifts while the page is open reads as a bug. Count + last used are the stable inputs.
- Adding rows by hand: that is the custom dictionary.
- A generic "user-data table browser" abstraction over all four stores: three row shapes, one flat message — no trait.
