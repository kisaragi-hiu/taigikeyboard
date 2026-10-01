# Mobile smart suggestions — brainstorm, references survey, maintainability audit

Status: **brainstorm, since executed in part** — R1–R4 merged, batches A/B delivered by the bigram LM (`docs/architecture/bigram-lm-roadmap.md`, closed 2026-09-30); leftovers in `docs/roadmap.md` § Open candidates. Original status: brainstorm only, nothing decided, no round open (USER 2026-09-24: 「this is brainstorm rounds … 我目前是對於手機輸入法的智慧建議有興趣，目前的字詞預測是比較簡單的」).
No release scope is implied. Open USER decisions are listed at the end.

Sources: three read-only research passes on 2026-09-24 (current pipeline map, `references/` prediction survey, engine + mobile maintainability audit). High-severity claims were spot-checked by hand (marked **verified**); the rest are `file:line` cites from the research passes and must be re-grepped before a round quotes them.

---

## 1. Today — the "next-word prediction" is dictionary-word completion (grounded in code)

### Pipeline

- `engine/nextword` is a state machine + filter; it fetches nothing. The platform does the lookups (`api.rs:12-25`, `filter.rs:1-5`).
- **Trigger**: `WordSelected` (final commit) or `Backspace` only (`decide.rs:76-177`). Nothing at sentence start / empty field.
- **Context**: one previous word. Reset by `。！？.!?` (`decide.rs:30,93-94`), 30 s idle (`decide.rs:26,64`), ResetFull. A user pair is recorded only if the previous commit was < 10 s ago (`decide.rs:22,283-289`).
- **Scoring** (`scorer.rs:7-59`): bundled pair = `count × 1.0`; user pair = `count×50 × max(decay, floor) + 300`, decay halves per week, floor 0.95 when count ≥ 3 else 0.30.
- **Filter** (`filter.rs:29-146`): stale-generation drop → merge by (漢字, TL) → fold spelling variants → display shaping → sort → limit 30 (both platforms pass 30).
- **UI**: iOS replaces the KeyboardKit suggestion row (`ActionHandler.swift:249-312`); Android smartbar `LazyRow` (`SmartbarCandidateStrip.kt:118`, `NextWordHandler.kt:558-596`). Tapping a prediction chains (records + re-predicts). No on/off setting.

### Bundled data — `association.bin`

- Built by `dictionary/build/create_association_bin.py` → `build/associations.py::compute_associations` from `output/dictionary.csv`. **No sentence corpus.**
- Pairs = adjacent characters inside each 2–5-char dictionary word, plus first char → rest of word (≤ 3 chars) (`associations.py:61-110`); count = summed word frequency (`associations.py:151-186`). Key is always one hanji character.
- 3.28 MB, 5,662 keys, 184,430 entries. Decoded examples: 台 → 灣 2137 / 員 1659 / 語 558; 語 → 言 / 文 / 義.
- Lookup key = **last character** of the committed word (`NextWordService.swift:108-114`, `NextWordService.kt:253`).
- Reader applies `limit` before the source filter (`lexicon/src/search.rs:236-237`), so disabled sources can shrink the list.

### User data

- `user_association.db` v6, unique `(prev_word, prev_tl, next_word, next_tl)`, `count` + `last_used`; cap 50,000, prune ~5,000 lowest-count-then-oldest every 100 writes (`NextWordService.swift:23-25,370-391`, `NextWordService.kt:72-78,705-736`). Recording always on (`SharedSettings.swift:287-289`, `PrefHelper.kt:628-629`).
- `user_frequency.db` feeds composing-candidate ranking only. `learned_phrases.db` (`PhraseLearned`, ≤ 6 syllables, `transition.rs:892-895`) is used only as a whole-buffer composing candidate (`lexicon/src/continuous.rs:750-760`) — never for next-word.

### Gaps, ranked by user impact

| # | Gap | Where |
|---|---|---|
| 1 | Bundled table only completes dictionary words; after 台語 → 言/文/義, not what follows 台語 in a sentence. Weak cold start. | `associations.py:61-110` |
| 2 | Bundled lookup keys on the last character only; 食飯 is looked up as 飯, heteronyms share one key. | `NextWordService.swift:108`, `.kt:253` |
| 3 | One-word context; no trigram / backoff. | `decide.rs:22,26,30` |
| 4 | No prediction at sentence start / empty field. | `decide.rs:76-177` |
| 5 | Space commit hides predictions — **deliberate** (Model B §10.3), **verified**. Space is the most common commit key. | `ios/.../ActionHandler+KeyActions.swift:172-183` |
| 6 | Continuous mid-segment commit sends `UpdateLastSelectedWord` → compound pairs only, no `prev→this`. A→B and prev→A are never learned. **Verified.** | `composing/src/transition.rs:915`, `nextword/src/decide.rs:229-269` |
| 7 | Composing candidates ignore the previous word; `BoostCandidates` has no production caller (**verified**). No context-aware homophone ranking. | `nextword/src/booster.rs:11-32` |
| 8 | No sentence-level data: unigram only (`char_freq_merged.txt` 5,185 chars, provenance "TBD" in `docs/SOURCES.md:116-117`; `khiin_frequency.csv` 17,905 words). `corpus/taigi-typing` is CC BY-ND and never a build input (`corpus/README.md`). | `dictionary/shared/data/` |
| 9 | Score mixing: bundled 台→灣 2137 beats a user pair learned ~37 times. | `scorer.rs:7-59` |
| 10 | Clause punctuation (`，`) neither resets context nor records. | `decide.rs:30` |

---

## 2. References survey — techniques that transfer

Commits read: mozc `afbf1d089`, azooKey `15bf8d11` (converter cites from AzooKeyKanaKanjiConverter `7304509`), librime-predict `920bd41`, ChiaKey `89aebc8`, KeyKey `81e05f0`, McBopomofo `b32cbf8`, vChewing-macOS `a4cccea6`, khiin-rs `71eb929`, lexical-models `f628994`, florisboard `859b00ce`, trime `277b8ea2` (GPL — architecture only).

| Rank | Technique | Reference | Size | Data prerequisite |
|---|---|---|---|---|
| 1 | Associated phrases keyed by **(漢字, TL)**, not one character (Core Principle #6); optionally extend the word being composed | McBopomofo `AssociatedPhrasesV2.h:48-72`, `derive_associated_phrases.py:9` (≤ 60 per prefix); vChewing `lmAssociates.swift:175-184` (pair first, then value) | S–M | Existing lexicon |
| 2 | Record the lost continuous pairs (gap #6) | this repo | S | none |
| 3 | Zero-query continuation from learned history: committed X, stored X+Y → suggest Y; chain only if committed within a short window | mozc `user_history_predictor.cc:730-760`, `:1325-1328` (10 s), `:96` (≤ 6 next links), `:2233` (whole multi-segment entry) | M | `learned_phrases.db` + word boundaries/timestamps |
| 4 | Two-word context with backoff (2 → 1 → bundled); punctuation resets | McBopomofo `UserOverrideModel.cpp:255-300`; vChewing `LXPerceptor.swift:828` (backoff), `:968-1017` (age factor `(1−age/T)²`) | M | `user_association.db` migration |
| 5 | Negative feedback: drop user entries picked in < 5 % of times shown | mozc `user_history_predictor.cc:1958-1986`; FlorisBoard `NlpProviders.kt:175` `removeSuggestion` | S | one shown-counter column |
| 6 | Rule table for zero-query: `「`→`」`, number → measure words, `@` → domains | mozc `data/zero_query/zero_query.def:37`, `zero_query_number.def:43`; McBopomofo `Data/associated-punctuation.txt` | S | hand-curated; measure words checked against `knowledge/` per `.claude/rules/phonetics.md` |
| 7 | Previous-word re-rank of composing candidates (homophone disambiguation) | revive `booster.rs` or feed bigrams into the lattice; mozc bigram source `dictionary_prediction_aggregator.cc:510,866` | M | data from 1 + 4 |
| 8 | Static word-bigram next-word table from a sentence corpus, top-K + min-weight prune, start-of-sentence key `$`, capped chaining | librime-predict `tools/make_predict_data/src/main.rs:31-89`, `predictor.cc:56-89` | S–M code | **licensed Taigi sentence corpus** |
| 9 | Word → emoji, ≤ 3 per commit | azooKey `KanaKanjiConverter.swift:1186-1201`; mozc `emoji_rewriter.cc:69` | S | `taigi-emojis/` output |
| 10 | Offensive-word filter on unsolicited suggestions only | mozc `suggestion_filter.cc:65-69`; `AssociatedPhraseCooker.rb:44` | S | curated list |
| 11 | Privacy gates: no learning / suggestions in password or no-personalized-learning fields, digits-only keys | mozc `engine_converter.cc:505`, `user_history_predictor.cc:241-282,1993-2020`; FlorisBoard `NlpManager.kt:170` | S | none — current mobile behaviour not yet audited |
| ✗ | Neural LM rerank (Zenzai) | azooKey `zenzai.swift:80-159`, `ZenzContext.swift:47-66` (GGUF, llama.cpp, no memory guard) | L | no Taigi model; collides with iOS 64 MB extension cap |

Also noted: ChiaKey `LearningStore` O(1) eviction (fewest picks, then LRU; `LanguageModel.h:97-122`), capacities measured on a 417k-token corpus (8k overrides / 16k bigrams, `:462-467`), "generalize after 3 contexts" (`:514`), and a learned-bigram weight kept decisive because weakening it cost 11 % more manual selections (`:516-537`). azooKey `LearningMemory.swift:268-281`: count halves every 32 days, drop after 128 days unused, cap 65,536.

---

## 3. Maintainability audit (engine + mobile)

No crate cycles (`phonetics ← ranking ← lexicon ← composing`; `nextword` depends on `phonetics` only); `cargo check --workspace` warning-free; no hand-written mobile file > 1,500 lines. Justified-looking layers: `mmap-host` (isolates `unsafe`), per-crate `dispatch.rs` (Path G proto boundary), `prev_tl`-first ordering (`behavioral-invariants.md` §24). The problems are **duplication and dead surface**, not over-design.

| Sev | Finding | Where | Est. |
|---|---|---|---|
| High | Prediction assembly (source mask → `assocLookup` → user SQL → 2× over-fetch → tagging → `filterPredictions`) exists in Swift **and** Kotlin; already drifted (Android-only ResetFull before record, generation bump on empty backspace, second `SENTENCE_END_PUNCTUATION`). Root cause: `nextword` cannot see `lexicon` (**verified**, `engine/nextword/Cargo.toml`). | `NextWordService.kt:241-363`, `NextWordService.swift:102-322`, `NextWordHandler.kt:169-180,273-285,506-510` | engine +250 / platforms −200 |
| High | Per-keystroke candidate fetch = two FFI calls (neutral fetch → platform frequency lookup → re-fetch), implemented on four platforms | Android `ComposingManager.kt:488-560`, iOS `ComposingManager.swift:340-435`, macOS `RustEngineBridge+Composing.swift:230` | −80 per platform |
| High | `ProcessCandidates` op + `ranking/src/process.rs` + both bridges have no production caller (**verified**: remaining mentions are doc comments) | `dispatch/src/lib.rs:136`, iOS `RustEngineBridge+Lexicon.swift:590-715`, Android `LexiconBridge.kt:402-527` | −600 |
| High | `platformFallbackFilters` re-encodes `compute_filters` bit layout on iOS + Android (redundant fallback per `planning.md`) | iOS `RustEngineBridge+Lexicon.swift:393-520`, Android `LexiconBridge.kt:201-330` (+ test `KautianSubcollWireEncodeTest.kt`) | −230 |
| Med | Ops without production callers: `NormalizeTone`, `RestoreTone`, `NormalizeToTl`, `ContainsTps`, `ClassifyInput`, `BoostCandidates`, `CapitalizeCandidate`, `SetSelectedCandidateIndex`; test-only `QueryState` / `NextWordQueryState` | `phonetics/src/dispatch.rs:63,69,111`, `lexicon/src/dispatch.rs:55`, `nextword/src/dispatch.rs:89`, `dispatch/src/case.rs:32`, `composing/src/transition.rs:89` | −600 |
| Med | Retired toggle still plumbed: `is_association_recording_enabled` (constant `true` since #130); iOS `isAutoCap` has no reader | `envelope.proto:128`, `decide.rs:107,252`; `EngineSettings.swift:16` | −75 |
| Med | Key-label casing: iOS native `.uppercased()`, Android via engine ops | `ButtonTextProvider.swift:170-178` vs `KeyLabelCaseCache.kt:66-68` | ±40 |
| Med | Toneless-key derivation implemented 5+ times | `phonetics/derivation.rs:29`, `phonetics/api.rs:503`, `composing/shadow.rs:49,1204`, `lexicon/continuous.rs:1315,1374,1401` | net −80 |
| Med | `lexicon/src/continuous.rs` 2,210 production lines (3,503 total) | split `toneless_match` / `candidate` / `sort_key` | net 0 |
| Med | 4 user-data stores × 4 implementations (~11k lines). `data-artifacts-portability.md:266` marks `wont_migrate`, but the memory file it cites no longer exists — rationale lost | iOS / Android / macOS / `taigi-desktop-storage` | USER decision |
| Low | Unused `Command` message, unread `CommandType`; `FetchAtPos.position` always 0; unused `nowMs` param; bridge naming drift (`lexiconAssocLookup` vs `assocLookup`, `NextWordController` vs `NextWordHandler`) | `envelope.proto:18,138,163`, `composing.proto:207`, `NextWordService.kt:245` | small |
| Low | Doc drift: `system-overview.md:49` misses `learned_phrases.db` + Linux; `data-artifacts-portability.md:17` wrong association paths; `behavioral-invariants.md:899` old proto path; stale `taigi-windows-core` mentions in macOS/iOS comments | admin lane | ~20 |

---

## 4. Draft sequence (unscheduled; each PR 200–500 LOC)

1. **R1 — dead surface removal** (2 PRs, delete-only, removed proto tags → `reserved`): `ProcessCandidates` + routing special-case; the unused ops, retired flag, `Command`, `isAutoCap`, `nowMs`.
2. **R3 — engine-owned prediction assembly** (2 PRs): one engine op takes prev word + prev TL + user rows + source mask and returns ranked predictions; composed in `dispatch` so `nextword` stays independent of `lexicon`. PR-a engine + golden tests, PR-b iOS + Android adopt (~−100 each). Every later suggestion feature then lands once.
3. **Suggestions batch A — no new data**: techniques 1, 2, 5, 6.
4. **Suggestions batch B**: techniques 3, 4, 7.
5. **Corpus track** (parallel): find a licensed Taigi sentence source → technique 8.
6. Slot in anywhere: R2 (drop `platformFallbackFilters` + unify bridge names), R4 (behaviour-freeze split of `lexicon/continuous.rs` + shared toneless key, `refactor-reviewer`), R5 (single-FFI candidate fetch; touches the `wont_migrate` boundary and the iOS 64 MB cap).

## 5. Open USER decisions (ranked by impact, each with a recommendation)

1. **Start point** — recommend R1 → R3 before features; alternative: features first, paying the two-platform cost each time.
2. **Sentence corpus** — candidates to license-check (unverified from memory): Mozilla Common Voice Taiwanese sentence collection (believed CC0), zh-min-nan Wikipedia (CC BY-SA, mostly POJ). `corpus/taigi-typing` is CC BY-ND and excluded.
3. **Predictions after Space** — hidden today by design; recommend a dogfood A/B of showing them.
4. **User-data stores into the engine** (audit Med row) — recommend leaving until R3 lands.

---

## 6. Addendum 2026-09-28 — bigram language model (added at USER request, to be picked up in a separate session)

Trigger: USER heard ChiaKey described as a "bigram lexicon" and asked what that means and which model we use. Nothing decided; no round open; no release scope implied.

### Status of §4 / §5 as of 2026-09-28

- Done: R1 (#187–#191), R3 (#194 + #196), R2 (#197), R4 file split only (#198; shared toneless key refuted), small items (#200). R5 absorbed into the user-data-engine roadmap (all user-data stores now live in `engine/userdata`, so batch A/B schema work lands there once).
- Delivered by the bigram LM (P3–P5): techniques 1, 2, 7, 8; technique 4 cut (bigram D7). Unscheduled: 3, 5, 6, 9, 10, 11.
- §5 decision 2 answered: USER 2026-09-27, "these licences are all fine, ignore them" → `corpus/taigi-corpus` opt-in submodule (#248, ~670 MB, `update = none`). The §5.2 text above (Common Voice / Wikipedia) is superseded. `corpus/taigi-typing` status unchanged.
- §5 decision 3 (predictions after Space) still open.

### What "bigram lexicon" means, and where we stand (grounded in code)

- **Unigram**: each word scored alone, P(w). **Bigram**: word-pair counts, P(w₂ | w₁). A bigram LM walks the candidate graph maximizing Σ log P(wᵢ | wᵢ₋₁), so the same reading converts differently depending on the previous word.
- **ChiaKey** (`references/ChiaKey/`, `chiakey-reference.md:12,46-47,98-102`): Manjusri engine, SQLite unigram + bigram log10 probabilities, `Graph::walk`; user learning is a capped `user_bigram_cache(previous, current)` with `LearnedBigramScore = log10(1)`.
- **Ours**:

| Stage | Model | Where |
|---|---|---|
| Whole-sentence segmentation | Unigram — `cost = ln(1/p)`, p = (1+freq)/corpus total, length / syllable bias, user-weight discount; every edge priced independently | `engine/composing/src/lattice/cost.rs:1-40` |
| Candidate sort | Unigram — `freq × syllable bias`, user weight as a separate leading dimension | `engine/ranking/src/score.rs:295` |
| Next-word | Pair-shaped (`prev_word → next_word`) + user pairs | `engine/nextword/src/scorer.rs:6-16` |
| Next-word bundled data | Pairs derived from inside 2–5-char dictionary words (台→灣), no running text | `dictionary/build/associations.py:1-30` |

- **Key point**: intra-word pairs add almost nothing to segmentation — the whole word (台灣) already competes as one unigram edge. The useful signal is **cross-word** (食→飯, 真→好), which only running text provides: articles, dictionary example sentences, or the user's own typing (ChiaKey-style learned bigrams, empty at install). Mandarin corpora (different grammar) and LLM-generated text (model bias, unstable Taigi quality) are poor sources.

### How the remaining items map onto a bigram LM

| # | Item | Relation |
|---|---|---|
| **8** | Static word-bigram next-word table from a sentence corpus, top-K + min-weight prune, start-of-sentence key `$` | **Is** the bigram LM (corpus track). Closes gaps #1, #8, and #4 via `$` |
| **7** | Previous-word re-rank of composing candidates | **Is** ChiaKey's use of it — feed bigrams into ranking / the lattice (or revive `booster.rs`). Data from 1 + 4 or the table from 8 |
| **1** | Associations keyed by (漢字, TL) | The table from 8 must share this key format — design together |
| **2** | Record lost continuous-commit pairs | = user bigrams (ChiaKey `user_bigram_cache`); A→B is never learned today |
| **4** | Two-word context with backoff | Bigram → trigram extension |
| 3 | Zero-query continuation from learned phrases | Indirect (`learned_phrases` data) |
| 5, 6, 9, 10, 11 | Negative feedback, rule table, emoji, offensive filter, privacy | Unrelated |
| §5.3 | Predictions after Space | Gates visibility: Space is the most common commit key and hides predictions today |

Two tracks, different risk:

- **Next-word track** (8, 1, 2, 4): `engine/nextword` + `association.bin`; contained.
- **Composing track** (7): bigram term in `lattice/cost.rs` changes whole-sentence segmentation; needs `/code-review` and re-checks of fixed cases such as `taiuan → 台灣` (cost.rs S5 note).

Both depend on the data from 8.

### Suggested first step (read-only spike, no branch)

Export `corpus/taigi-corpus` (`corpus export`), count (漢字, TL) word pairs across word boundaries, and report: total distinct pairs; top-K coverage; how many of the 5,662 `association.bin` keys get at least one cross-word continuation. Confirm first how the corpus marks word boundaries (TL hyphenation vs. spaces) — not yet checked.
