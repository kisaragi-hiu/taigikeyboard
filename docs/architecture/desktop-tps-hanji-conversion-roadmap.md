# Desktop TPS — Hanji conversion in the preedit (arm B)

Under TPS (方音符號, the i18n `en` label "Phonetic Symbols") on macOS, Windows and Linux, the preedit shows the predicted Hanji while the user types, and ↓ opens the candidates of the word at the caret — the way the Zhuyin input methods work. This is arm B of U8 in [`desktop-tps-roadmap.md`](desktop-tps-roadmap.md); arm A (the preedit stays glyphs, D7) is what P1–P6 of that roadmap built.

Status: H-P1 (engine, typing forward) is merged (#398); H-P2 (engine, the caret) is merged (#401); H-P3 (engine, choosing a word and the commits) is merged (#402); H-P4 (desktop core and the three shells) is merged (#403): under TPS the desktops show the conversion. H-P5 (the macOS window's place, the invariant, the dogfood items) is merged (#404): every phase is merged. No release is assigned; scope and timing are the maintainer's call.

Not next-word prediction. The desktops never suggest a word after a commit (maintainer, 2026-10-03), and nothing here changes that: this converts the glyphs being composed, before any commit. TL and POJ are untouched (U8).

## Maintainer decisions (2026-10-05)

The request (2026-10-04, translated): "I want TPS typing to be closer to how the Zhuyin input methods type: on the desktop it predicts Hanji, and the Down arrow opens the candidate menu to choose."

| # | Decision |
|---|---|
| B1 | Selection model = the word at the caret, with everything before it nailed as shown. ↓ opens the candidates of the word before the caret; a pick nails the words before it as displayed, nails the pick, and the rest is converted again. Not chosen: a full McBopomofo-style grid where any pick anywhere survives on its own, and left-anchored picking only. |
| B2 | Enter commits the Hanji as shown. Shift+Enter commits the glyphs as typed, as today. |
| B3 | Arm B replaces arm A. No setting keeps the glyph preedit. |
| B4 | Only a word the user picked from the candidate window is written to user data, under the identity (Hanji, canonical TL) (`AGENTS.md` Core Principle 6). A word the walker chose and the user only accepted with Enter teaches nothing. |

## Today (grounded in code)

`C/` = `engine/composing/src/`, `core/` = `desktop/crates/taigi-desktop-core/src/`. Read at `a758ee2d`.

| Fact | Where |
|---|---|
| A TPS buffer goes through the same shadow → lattice → walker as TL and POJ; no gate excludes TPS | `C/continuous.rs:1088-1096`; `C/syllabifier/mod.rs:50-71`; `C/syllabifier/tps.rs:73-131` |
| A lattice edge spans 1–8 syllables; the lattice itself covers the whole buffer | `C/shadow.rs:188`; `C/lattice/builder.rs:54-103` |
| The walker's best path is a list of edges, each with its Hanji, reading and syllable count | `C/lattice/walker.rs:40-107` |
| That path leaves the engine flattened into one row: slot 0 of the candidate list, whose `hanji` is set only when every edge has one | `C/continuous.rs:104-119, 927-938` |
| Read by running (`engine/composing/tests/candidate_dump.rs`, production artifacts, 2026-10-05): `ㄍㄧㄣ ㄚˋㆢㄧㆵ˙ㄊㆪ ㄎㄧ˪ㄐㄧㄣ ㄏㄛˋ` → slot 0 `今仔日天氣真好`; a twelve-syllable buffer converts whole; a user-frequency row `臺語 / tâi-gí` turns `台語真好` into `臺語真好`, and a single-syllable row changes the homophone the walker shows | same harness, `DUMP_MODE=tps`, `DUMP_FREQ` |
| The rest of the list is left-anchored: every row's span starts at 0 | `C/continuous.rs:1112-1451`; `C/lattice/mod.rs:11-22` |
| The preedit of the pending tail never consults the dictionary: under TPS it is the raw glyphs minus the separator | `C/derived.rs:18-26` |
| A pick of a prefix nails it (nothing is written to the document), the rest stays pending and is fetched again; Backspace on an empty tail un-nails the last segment and restores its glyphs | `C/transition.rs:734-831, 457-487`; `C/api.rs:132-157` |
| So Hanji already appears in a desktop preedit — after a pick only | `C/api.rs:412-446`; `core/../tests/composing_manager.rs:382-390` |
| The engine caret is a byte offset in the pending tail and never enters a nailed segment; the display caret assumes one displayed character per raw character | `C/api.rs:17-21`; `C/transition.rs:146-169, 280-288`; `C/derived.rs:96-132` |
| A mutation does no segmentation; the fetch is a separate request the platform sends, run on a clone of the engine, twice when user data is attached | `C/transition.rs:264-273`; `C/handle.rs:13-22`; `engine/dispatch/src/user_data/with_stores.rs:48-51, 105-126` |
| A candidate pick records usage in the engine at commit; `CommitRaw` (Enter under D7) records none | `C/transition.rs:863-871, 527-582`; `with_stores.rs:63-76` |
| A final commit of two or more Hanji segments, six syllables at most, learns the phrase (§50) | `C/transition.rs:1029-1051` |
| Custom and learned phrases are not walker edges under TPS | `C/continuous.rs:527-529, 648-660` |
| The desktop fetches nothing on a TPS key; the window opens on demand | `core/composing/intent_executor.rs:76-101, 202-206` |
| TL and POJ fetch on every key | `core/composing/intent_executor.rs:66-75` |
| The classifier's TPS branch; "window up" is "the list is not empty" | `core/keys/intent.rs:156-213, 352-396` |
| Plain ← → are navigation keys: typing, they open the window. The caret chord is Ctrl+← → (⌥ on the Mac) | `core/keys/intent.rs:165-183, 360-367` |
| Every shell draws the preedit as one string, one caret and one underline; no model carries ranges | Windows `taigi-windows-tsf/src/composition.rs:234-267`, `display_attribute.rs:30-37,118`; IBus `taigikeyboard-ibus/src/wire.rs:94-107`; Fcitx5 `linux/fcitx5/src/engine.cpp:182-193`; macOS `Composing/ClientWriter.swift:28-70`, `taigi-macos-ffi/proto/desktop_shell.proto:292-296` |
| The macOS candidate window is placed from the length of the marked text | `taigi-macos-ffi/src/session.rs:382-393`; `desktop_shell.proto:315-321` |
| The three callers of the classifier and the executor | Windows `taigi-windows-tsf/src/session.rs:238-244, 1506`; Linux `taigi-linux-core/src/session.rs:284-290, 550`; macOS `taigi-macos-ffi/src/session.rs:114-122` |
| A pick that leaves no pending text finalizes: the whole composition is written and the engine goes idle | `C/transition.rs:778-804` |
| `CommitRaw` writes the nailed segments' display text (Hanji) and the pending glyphs — not the glyphs of the whole composition | `C/transition.rs:527-540` |
| Frequency rows are looked up only for the words in the candidate rows of the neutral fetch | `engine/dispatch/src/user_data/with_stores.rs:185-192` |
| A mutation answers with its glyph `UpdatePreedit` at once, and the desktop writes it before any fetch | `C/transition.rs:264-273`; `core/composing/manager.rs:395-424` |
| `FetchAtPos` field 1 (`position`) is `reserved` | `engine/protos/proto/composing.proto:149-151` |
| A next-word pair is learned from the terminal word and its preceding segments | `C/transition.rs:999-1051`; `engine/nextword/src/decide.rs:191-199` |

What carries over: the syllabifier, the lattice, the walker, the left-anchored list, nailing and un-nailing, usage recording. What is missing: the path kept as segments, a preedit made from them, a list anchored at a word other than the first, a pick that does not finalize, and a commit that writes the converted text.

## Design

### H1 — What the preedit shows

Three parts:

1. The nailed segments, as today.
2. The **conversion**: the walker's best path over the closed part of the pending tail. An edge with a Hanji shows the Hanji; an edge with none shows its glyphs.
3. The **open reading**: the glyphs of the syllable being typed, as typed, at the caret — at the end while typing forward.

A reading is closed by its tone mark, or by Space for tones 1 and 4 (D0's separator rule) — the same moment a Zhuyin input method composes a reading. The closed part ends at the last such boundary that the TPS syllabifier can reach from the start of the tail (`C/syllabifier/tps.rs:108-169`: a stop coda belongs to the body, tone 8 closes only after its mark), found through the shadow's offset map and barriers, not by scanning for characters. What cannot be closed stays glyphs: an orphan tone mark (`C/requests.rs:201`), a trailing hyphen run (`C/shadow.rs:323`), a tail the syllabifier rejects.

The walk runs when a reading closes and when closed text is removed or re-opened — not on a glyph of an open reading, during which the converted text does not move. When a reading closes, the words before it may change: the best path over a longer buffer is not the old path plus one word, and a separator's tone pin applies only at a span's end (`C/shadow.rs:793`). That re-segmentation on completion is what the Zhuyin input methods show too.

One underline over the whole preedit, one caret — what McBopomofo and its Windows port draw. No shell's drawing changes.

Typing without tone marks keeps working: the open reading grows as glyphs, Space closes it, and ↓ opens its candidates as it does today.

### H2 — The conversion is engine state

The engine keeps the conversion: `Phase::Continuous` gains the converted segments of the pending tail (raw span, displayed text, Hanji, canonical TL) and the open reading's span. The preedit text and its caret are derived from that state, segment by segment — not through `display_caret_utf16`, which matches characters one to one.

A request that changes the buffer answers with one `UpdatePreedit`, already converted: the dispatch layer applies the mutation, runs the walk when H1 asks for one (with the user-data re-rank, as a fetch does), stores the result against the state it was computed from, and only then builds the answer. One key, one write to the host. A walk that fails or finds nothing leaves the affected text as glyphs.

It is on only when the request's `AppConfig` asks for it; the desktop asks under TPS. Mobile, TL and POJ send nothing new and keep the state they have.

As built in H-P1 (`engine/composing/src/conversion.rs`):

- The walk runs inside the mutation, under the composing mutex, in one read of the lexicon for the boundary and the path. `Phase::Continuous` holds the result beside `raw` and `caret`, so replacing the phase replaces it.
- The closed part is walked as a buffer of its own, with its trailing separator, so the §41 pin holds and a tail the whole-buffer walk cannot span still converts what is closed. The boundary comes from the whole tail's lattice: a boundary that what follows makes invalid (a tone mark typed after a separator) is not closed.
- A conversion is shown and re-used only for the source filter it was walked with; a request that asks for another filter, or for none, gets the glyphs, and the next mutation walks again.
- A conversion existed only while the caret was at the end of the tail; H-P2 replaced this with the caret by word (H3, As built in H-P2).
- The walk is neutral — no user frequency, no previous-word context. H-P3 adds the user's rows with the frequency lookup of H5.
- A segment keeps its raw span and its displayed text. The Hanji and the canonical TL of H2 arrive in H-P3 with the pick that reads them.
- A tone mark typed after a reading its own mark already closed (`ㄒㄧˋ` then `ˊ`) shows that word as glyphs again: the whole tail's lattice refuses a syllable end followed by a tone mark, as the candidate list does. Kept as it is (maintainer, 2026-10-05); Backspace restores the word.
- `CommitRaw` and `CommitContinuous` keep their behaviour: the conversion changes what the preedit shows, not what a commit writes, until H-P3.

Why state rather than a fetch answer the caller hands back: a fetch runs on a clone of the engine (`C/handle.rs:13-22`), so a conversion returned by one would have to be carried back by every commit and checked against a revision the engine does not have — the caret, the nails or the context can change under equal raw text. Held in the engine, there is nothing to go stale, the commits need no payload, and the open reading is spliced into a conversion the engine still has.

### H3 — The caret

The caret sits on a boundary between words of the conversion, or inside the open reading. Plain ← and → — and the caret chord — step it: one word over the conversion, one glyph inside the open reading. Stepping left past the first pending word re-opens the last nailed segment: its glyphs are put back in front of the tail, the tail after them is kept, the caret lands before the re-opened word, and the tail is walked again — so a pick made there is dropped (librime re-opens a selected segment the same way). The usage that pick recorded stays recorded.

A glyph typed with the caret between two words starts an open reading there. The conversion on both sides is kept as it was; the walk runs again when that reading closes.

Backspace always removes one glyph. A closed syllable that loses its tone mark is an open reading again and shows as glyphs. On an empty tail it un-nails, as today.

As built in H-P2 (`engine/composing/src/conversion.rs`, `transition.rs`):

- The conversion is a list of words in raw order that need not cover the tail: the raw text no word covers shows as glyphs. No open-reading span is stored; the text between and after the words is the open text, and the caret is on a word boundary or inside that text.
- A word's raw span ends after the separator run that closes its last reading, so the caret stop after it is past the separator and a glyph typed there starts a new reading (H-P1 gave an interior separator to the next word).
- A request that leaves the text alone (a caret step) keeps the words. An edit at the end of the tail is H-P1's rule: the closed part is walked again when its text changed, and words with glyphs between them are never re-used there. An edit inside the tail keeps the words before the changed text and the words after the caret (shifted); a word the edit touched shows as glyphs. When the reading at the caret closes — a closing end of the whole tail's lattice falls in the uncovered text at or after the caret, open text may follow — the closed part is walked again, and a caret the new walk put inside a word moves to that word's end.
- So Backspace that takes a word's tone mark at the end of the tail walks the closed part again (the word's other closed readings convert), while inside the tail the whole word shows as glyphs until its reading closes: the caret must stay in that reading.
- A reading left open inside the tail that the syllabifier cannot read stops the lattice there: no reading after it closes, and a walk converts only what is before it.
- Re-opening is `MoveCaret` left from the start of the tail, under the switch and on a TPS buffer — no new intent. It answers as Backspace's un-nail (the NextWord handshake, `UpdatePreedit`, `RefreshCandidates`), and the tail is walked afresh. Without the switch the start of the tail stays an edge.
- A caret step under a request the held conversion is not for (another source filter, or the switch was off) walks the tail first, then steps over its words, so a step never moves the other way.
- A step over a hidden separator in the open text moves the raw caret and not the drawn one, as in the glyph preedit.
- With no dictionary word in reach (every source off), the walker's path can be one glyph word over several readings, and the caret steps over all of them at once.

### H4 — Choosing a word

↓, or Space after a closed reading, opens the list for the word before the caret — the last word when the caret is at the end, the first when it is at the start. The list is the engine's left-anchored list from that word's start to the end of the tail, without the whole-sentence row: longer words first where the dictionary has them, then the word's homophones. With an open reading before the caret, the list is that reading's.

A pick, in one transition: the words before it are nailed as displayed and marked not picked, the pick is nailed and its usage recorded (B4), the window closes, the caret goes to the end, and what follows the pick is walked again. A pick that reaches the end of the tail does **not** finalize: the composition stays, all of it nailed, until a commit.

As built in H-P3 (`engine/composing/src/conversion.rs`, `transition.rs`, `requests.rs`):

- The list's start (`conversion::word_list_start`, one rule for the fetch, its context and its pick): the start of the word ending at the caret, else the start of the text no word covers that the caret is in, `0` at the start of the tail. The fetch lists `raw[start..]` and shifts the spans back into the tail's coordinates; the pick resolves the same start from the same state, so `CommitContinuous` gains no field.
- "Without the whole-sentence row": the walker's row is listed only when its path is one word, so a reading the dictionary has no word for keeps its row. It applies whenever the field is set under a requested conversion, the list of the whole tail included.
- Everything before the start is nailed as shown, byte for byte: each word with its Hanji, canonical TL and syllable count, and each run of glyphs between words with no word identity. Un-nailing restores what was typed.
- A pick that does not end after the start is ignored. The pick's effects are `UpdatePreedit`, `NextWordUpdateLastSelectedWord`, `ClearCandidates` — not `RefreshCandidates`, which would ask for the list again.
- Both passes of a fetch with the stores (`dispatch` `with_stores`) and the fetch without them answer from one copy of the engine (`EngineHandle::engine_at`), so the rows read for the listed text rank that text and that start. This replaced the retry that re-read the buffer when the main thread had typed in between.
- The list's context word is the converted word ending at its start, or at the start of the tail the last nailed segment, when it is a word: picked, or carrying a canonical TL. Glyphs before the start cut the context, and the committed context applies only when the list starts the composition (`ListContext`).

### H5 — Engine surface

Names and wire shapes are settled in each phase's pre-implementation review; the contract is:

| Piece | Contract |
|---|---|
| Switch | `AppConfig.hanji_conversion` turns H2 on for the request by being present. It carries the dictionary source toggles (`FetchAtPos.toggles`' message): a key carries no fetch, so the walk's source filter rides the config |
| The word's list | `FetchAtPos` under the switch lists the word before the caret (as built: no new field — see As built in H-P3); the engine resolves the anchor from its own segments. Frequency rows are looked up for the walker's edge alternatives too, not only for the rows listed |
| Pick | `CommitContinuous`, under H2, nails the words before the anchor itself (not picked), then the pick, and keeps the composition |
| Commit as shown | A new intent: writes nailed text + conversion + open reading and goes idle. Enter, a printable that commits first, a mode switch and a host's Finalize use it |
| Commit as typed | Under H2 the key bound to Commit as Typed writes the glyphs of the **whole** composition, rebuilt from each segment's raw text with the separators stripped — a new intent, since `CommitRaw` writes nailed Hanji |
| Caret | `MoveCaret` under H2 steps as H3 says; a step left from the start of the tail re-opens the last nailed segment (no new intent — H-P2's pre-implementation review) |
| Learning | A nailed segment carries whether it was picked. A segment not picked gives no usage, keeps §50 from learning a phrase, and is no predecessor or successor in a next-word pair — the pair is not bridged across it |

Legacy requests keep their behaviour byte for byte: `FetchAtPos` without the new field, `CommitContinuous` and `CommitRaw` without H2.

As built in H-P3: `FetchAtPos` gains no field. Under the switch its list is always the word before the caret's — no key under arm B asks for the whole tail's, and a field that could ask for it let a pick of a whole-tail row be resolved against the word's start (cloud review, below). The commits are `CommitAsShown` (43) and `CommitAsTyped` (44). Commit as shown writes exactly the preedit's display. A walk ranks with the user's counts through `ConversionFrequency`, which the dispatch layer supplies with a mutation when the stores are open (the engine keeps no clock): before the walk, the words every edge of the closed part could take are looked up in one read of the store (`rows_for_words` binds 500 words per statement); a key that closes no reading looks up nothing. The store's reads never wait: a read that finds the reader busy answers nothing, and that walk ranks neutral, as a fetch's re-rank does. A pick's count is written after the request's own walk and is read by the walks after it. Whether the conversion is asked for is judged on the whole composition (`conversion::is_requested_for_composition`), so a tail a pick left with tone marks only stays in the word's list and its pick keeps composing. Un-nailing back to a segment that was not picked answers `NextWordClearForNewComposing`, not `NextWordUpdateLastSelectedWord`. Under H2 `CommitRaw` writes what it always wrote; a composition holding a segment that was not picked teaches it nothing.

### H6 — Keys, against D7

Rows that change:

| State | Key | D7 today | Arm B |
|---|---|---|---|
| Typing | A layout key, Backspace | Edits the glyphs | Edits the glyphs; the answer's preedit is converted (H1); no window |
| Typing | ← → | Open the window | Step the caret (H3) |
| Typing | The caret chord | Steps one glyph | Steps as ← → do |
| Typing | ↓ ↑, Page Up / Down, and the keys bound to navigation or paging rows (Tab, `[` `]`) | Open the window on the first candidate of the whole buffer | Open the window for the word before the caret (H4) |
| Typing | Space after a closed reading, caret at the end | Opens the window (whole buffer) | Opens the window for the last word |
| Typing | The key bound to Confirm (Enter) | Commits the glyphs as typed | Commits as shown (B2) |
| Typing | The key bound to Commit as Typed (Shift+Enter) | Commits picked Hanji + the glyphs of the tail | Commits the glyphs of the whole composition |
| Window up | Number row / keypad `1`–`9`, Space, Enter | Picks; the window re-opens on the remainder; a pick of the last word commits | Picks; the window closes; the rest is walked again; nothing is committed |
| Either | Any other printable, Shift+Space | Commits the glyphs, then the key | Commits as shown, then the key |
| — | A mode switch, a host's Finalize, the Windows Shift-tap, the symbol picker's commit-first | Commits the glyphs | Commits as shown |
| Show Candidate Window off | Space after a closed reading | Commits the glyphs as typed | Commits as shown |
| Show Candidate Window off | A navigation key | Commits the glyphs and goes on to the host | ← → step the caret; ↓ and the rest commit as shown and go on to the host |

Rows that do not change: Space inside an open reading is the separator (it now closes the reading, so the word appears); a Space the engine refuses with the caret inside the composition does nothing; with the window up, plain arrows / Tab / `[` `]` navigate (tier 1), the caret chord closes the window and steps (tier 0), Escape closes the window, a second Escape cancels, Backspace and a layout key close the window and edit; a fetch that finds nothing leaves the window shut and the composition up; Ctrl + a layout punctuation key; a click on the key panel types its glyph at the caret (U9).

The classifier still never asks the engine (the Windows Test phase): every row above is decided from the key, "composing" and "window up". Whether a reading is closed is the engine's answer to the Space, as under D0.

With Show Candidate Window off the preedit is still converted — the conversion is not the window.

### H7 — Learning (B4)

A pick records usage as every pick does today, under (Hanji, canonical TL), and the walker reads it on the next walk — verified above for TPS, single-syllable words included. Words nailed as displayed and the commit as shown record nothing: no usage, no learned phrase, no next-word pair. A phrase is learned (§50) only when every segment of the commit was picked.

As built in H-P3: commit as shown records no usage, and teaches the §50 phrase and the next-word sequence only when nothing is pending and every segment was picked — the composition then is the user's picks, as a final pick's is today. Next-word learning is all or nothing. `CommitAsShown`, and `CommitRaw` or a legacy final pick whose nailed segments include one not picked, teach the sequence only when every segment before the last word was picked (a pending tail is the last word for `CommitRaw`, as before); otherwise they answer `NextWordClearForNewComposing` and no `NextWordWordSelected`. Learning only the picked run at the end would still pair the committed context with its first word across the unpicked ones, since a `NextWordWordSelected` chain always starts there. `NextWordClearForNewComposing` keeps the committed context, so the desktop must forget it after such a commit (H8).

### H8 — Shells

| Layer | Changes |
|---|---|
| Desktop core | The engine bridge sets the switch under TPS; the classifier's TPS branch (H6) — Confirm and Commit as Typed part ways (`core/keys/intent.rs:372-375`), ← → become `MoveCaret`; the executor: the anchored open (every TPS fetch under the switch is the word's list; the bridge sends no new field), the pick that keeps composing, the two commits. No fetch is added after a TPS key and the core keeps no conversion. From H-P3's review: after every commit that answers `NextWordClearForNewComposing` without `NextWordWordSelected` the manager forgets the next-word context, as it does after `commit_preedit_then_insert` (`core/composing/manager.rs:268-275`); a caret step, an edit or a settings change closes the list, and a pick is sent only from the list the current state answered (the engine resolves the start again at the pick) |
| Windows, Linux, macOS | All three change together: they share `perform_intent`. Session tests and the Swift tests that pin the glyph preedit and the old pick |
| macOS | Whether the window sits under the chosen word instead of the end of the marked text is decided on device |

No display attribute, no IBus / Fcitx5 attribute and no `SetMarkedText` field is added.

As built in H-P4 (`core/engine/bridge.rs`, `core/composing/{manager,intent_executor}.rs`, `core/keys/intent.rs`):

- The switch rides `app_config`, the one config builder: every request under TPS carries `hanji_conversion` with the dictionary switches; TL and POJ requests carry none.
- A TPS composition commits through `ComposingManager::commit_composition` as `CommitAsShown`, so every path that already commits through the manager — Enter, a key that commits and goes on to the host, Space with the window off, the switch across TPS, a host's Finalize, focus changes — writes as shown with no change of its own. A new `ComposingKeyIntent::CommitAsTyped` carries the Commit as Typed row. A TPS composition is listed, picked from and committed under the settings its preedit was last written under: after a switch across TPS or changed dictionary switches the engine would otherwise not be shown the held conversion — a commit would write the glyphs, not the Hanji on screen, and a pick from a list still up would be resolved against another start and swallow the words before it. A commit that answers `NextWordClearForNewComposing` without `NextWordWordSelected` forgets the next-word context.
- `CommitPreeditThenInsertExternal` writes the preedit as shown (`transition::phase_preedit`), so a key that commits first writes the Hanji and itself in one write. Without a conversion for the request the text is the same as before: mobile and TL / POJ unchanged.
- The symbol picker's commit-first and the Windows candidate list's `Finalize` choose through `ComposingKeyIntent::commit_first`: the highlighted cell under TL and POJ, the composition as shown under TPS, where a pick only nails a word.
- A pick closes the list when the engine answers `ClearCandidates` rather than `RefreshCandidates` (`CandidateCommitOutcome::Nailed { refetch }`), which is what a pick under the conversion answers; a settings change closes an open list of a TPS composition (`represent_list`, `ComposingManager::is_tps_composition`).
- Linux focus-out keeps committing the preedit the daemon holds, which is now the Hanji shown.

As built in H-P5 (`taigi-macos-ffi/src/session.rs` `RecordingSurface::finish`, `core/composing/manager.rs`):

- The macOS window anchors a TPS list at the caret: `CandidatesChanged.anchor_end_utf16` (field 3, renamed from `marked_text_length_utf16`, same tag) is the display caret the manager now mirrors, and the controller's caret walk asks for the character before it, then further left. The window sits under the last character of the word it lists; with the caret at the end it is where it always was. TL and POJ lists anchor at the end of the marked text, as before.
- McBopomofo asks for the character after the caret (`references/McBopomofo/Source/InputMethodController.swift:886-893`), one character to the right of this anchor mid-sentence and the same at the end. The character before the caret is the word being chosen.
- Accepted: at the start of the tail with a nailed segment before it, the list is the word after the caret and the window sits under the nailed text's last character, one word to the left. The anchor is the caret's position, which the core has; the start of the listed word in display units it has not.
- Windows also anchors at the composition's end (`taigi-windows-tsf/src/ui/caret.rs`); H8 scoped the window's place to macOS, and Windows is unchanged.

### H9 — Latency

A glyph of an open reading costs less than a TL key does today — no walk. A key that closes a reading costs one walk over the pending tail, inside the key's own request. The new cost is length: a sentence held in the preedit is longer than a TL composition usually gets. The gate is qualitative (`code-review-rules` §9): typing a long sentence shows no visible lag on the three desktops, no keyboard dismiss, no growth in memory. No head auto-commit is planned; if the dogfood finds lag, that is the first answer to weigh (vChewing commits the head past 20 readings).

## Guard rails

### TL and POJ are not affected (maintainer, 2026-10-05)

- The switch (H5) is set only when the stored input mode is TPS. Without it the engine takes none of the new paths.
- Classifier and executor edits stay inside the TPS branch (`core/keys/intent.rs:206-213`, `:352-396`; `core/composing/intent_executor.rs:76-101`). A shared function gains behaviour only behind the switch or the TPS arm.
- No TL or POJ test expectation changes in any phase — engine goldens, desktop-core tests, the three shells' session tests. A diff that edits one is a finding, not an update.
- Each phase's PR runs the TL and POJ suites of every touched platform and says so; each dogfood item opens with a TL and a POJ sentence typed as before.

### Baseline

- The cites here are `a758ee2d`. The work other sessions had in flight when this was written has merged (#395, #397, 2026-10-05) and touched no file this plan cites; no other session is working in the repository.
- H-P1 opens on a rebase and re-reads every Today row and H6's "D7 today" column against the code then; a row that moved is corrected in the phase's PR. Done at `0fdbd6e3`: nothing under `engine/composing`, `engine/dispatch` or the desktop key path changed since `a758ee2d`, so every row stands. Two rows move with H-P1 itself: a mutation now answers with a converted `UpdatePreedit` when the request asks for the conversion, and the walker's path is kept as segments (`C/conversion.rs`) beside being flattened into slot 0.
- D7 and U8 in `desktop-tps-roadmap.md` are marked as revised only in H-P5, after arm B is on main. Done in H-P5.

## Phases

Sizes are estimates. The engine phases are unreachable until H-P4 sets the switch.

| Phase | Type | Scope | Builds / tests | Size | Status |
|---|---|---|---|---|---|
| H-P0 | docs | This roadmap, the `roadmap.md` row | — | — | Merged |
| H-P1 | feat (engine) | H1, H2 for typing forward: the switch, the conversion in the state, the closed-part boundary, the walk on closing, the derived preedit and caret, Backspace; tests from production syllables (fixture rule: every strict-prefix syllable asserted) | engine; `make build` for the mobile artifacts (additive) | ~450 | Merged #398 `3e81146b` |
| H-P2 | feat (engine) | H3: the caret by word, the open reading inside the tail, re-opening a nailed segment | engine; `make build` | ~400 | Merged #401 `1f54d61b` |
| H-P3 | feat (engine) | H4, H5, H7: the word's list and its frequency lookup, the pick that keeps composing, commit as shown, commit as typed, the picked mark and what it withholds | engine, dispatch, nextword; `make build` | ~500 | Merged #402 `241cd651` |
| H-P4 | feat (desktop-core, all three shells) | H6, H8: the switch on under TPS, the classifier and executor rows, every shell's tests | desktop-core, Windows, Linux, macOS | ~500 | Merged #403 `8ff54b50` |
| H-P5 | feat (macOS) + docs | The window's place on device; the behavioural invariant for H1–H7; the dogfood items; D7 and U8 in `desktop-tps-roadmap.md` marked as revised, S91 reworded | macOS | ~250 | Merged #404 `5ba4158f` |

Each implementation phase: its own branch and PR; an engine PR with a new op takes the full Codex sandwich; a Claude cloud review per PR (U7).

Regression surface, checked in every phase:

- Engine: request decoding and the exhaustive intent matches (`C/requests.rs:60-73`); the legacy fetch golden (`engine/composing/tests/golden_fetch_at_pos.rs`); the continuous phase, commit-resolution, caret, TPS key / separator / tone, learned-phrase and lifecycle tests; dispatch user-data reads and writes; next-word decisions. Old expectations stay; arm B gets its own matrix.
- Mobile: regenerated bindings only (`ios/.../RustEngineBridge+Composing.swift`, `android/.../ComposingBridge.kt`); no call changes.
- Desktop: the engine bridge (`core/engine/composing.rs`), the manager, executor and classifier tests; the three callers listed under Today; Linux `tests/session_characterisation.rs` (`tps_opens_the_table_on_demand_and_the_number_row_picks`, `switch_tps_round_trip_commits_the_glyphs_on_the_next_key`); macOS `taigi-macos-ffi/src/session.rs` tests (`under_tps_the_list_opens_on_demand_and_the_number_row_picks`, `a_tps_keyboard_press_types_its_glyph_even_over_an_open_list`) and the Swift TPS tests; the Windows key sink's Test / Deliver and panel clicks (box harness — the TSF crate tests only on Windows).
- TL and POJ: nothing above runs without the switch.

## Best practices alignment

Rules: `planning.md` § Grounded in actual code, § No redundant fallback; `AGENTS.md` § Design principles (shared logic lives in the engine), Core Principle 6; `docs/contributing/known-pitfalls.md` § Tests and diagnosis (fixture rule, trace before assert) for H-P1 to H-P3; `docs/contributing/cross-platform-alignment.md` for H-P4.

| Mainstream practice | Source | This plan |
|---|---|---|
| The preedit is the walked values with the reading in progress spliced in at the caret | `references/McBopomofo/Source/KeyHandler.mm:2471-2478`; `references/vChewing-macOS/Packages/vChewing_Typewriter/Sources/Typewriter/InputHandler/InputHandler_HandleStates.swift:49-74` | H1, H3 (H-P1, H-P2) |
| The walk runs when a reading is completed, not on each glyph; the handler keeps the walk | `McBopomofo/Source/KeyHandler.mm:499-528` | H1, H2 (H-P1) |
| Space or ↓ opens the candidates of the node before the caret, only with no reading in progress | `McBopomofo/Source/KeyHandler.mm:586-626, 2523-2542`; vChewing `InputHandler_HandleStates.swift:1175-1195` | H4 (H-P3, H-P4); with an open reading the list is that reading's, which D7 already does |
| The list holds the nodes over that place, longest first | `McBopomofo/Source/Engine/gramambular2/reading_grid.cpp:206-232` | H4, from the left-anchored list (H-P3) |
| A pick fixes the choice, the rest is converted again, and nothing is committed | `references/khiin-rs/khiin/src/buffer/buffer_mgr.rs:1096-1129` (the candidate + `convert_all(remainder)`); `references/librime/src/rime/context.cc:246-258` | H4 (H-P3) |
| A selected segment is locked; moving back re-opens it | `references/librime/src/rime/segmentation.h:19-25`, `segmentation.cc:26-40`, `context.cc:204-214` | H3 (H-P2) |
| Enter commits the whole converted buffer | `McBopomofo/Source/KeyHandler.mm:1224-1238`; vChewing `InputHandler_HandleStates.swift:513-525` | B2, H5 (H-P3, H-P4) |
| Backspace takes one glyph of a reading in progress | `McBopomofo/Source/KeyHandler.mm:1078-1107` | H3 — and one glyph always, since the raw buffer is glyphs |
| Learning happens on a pick, not on a commit | `McBopomofo/Source/KeyHandler.mm:189-191` | B4, H7 (H-P3) |
| One underline over the buffer, the caret as a zero-length selection | `McBopomofo/Source/InputState.swift:339-346`; `references/KeyKey41-Eten-Tribute/src/Client/StateEditSession.cpp:241-303` | H1, H8 |

Deliberately not adopted:

- **A reading grid with overrides anywhere** (`reading_grid.cpp:360-403`, vChewing `consolidateNode`) — offered, not chosen (B1). It needs constraints in the walker; nailing reuses what the engine has.
- **A setting that keeps the glyph preedit** — McBopomofo's Plain Bopomofo (`KeyHandler.mm:548-571`), vChewing's `useSCPCTypingMode`. Not chosen (B3): one path under TPS.
- **Learning from a commit** — not chosen (B4).
- **A separate decaying override model** (`McBopomofo/Source/Engine/UserOverrideModel.cpp:232-298`) — the engine's user frequency already feeds the walker.
- **The conversion returned by a fetch and carried back by the caller** — the first draft of this plan; see H2 and Reviews.
- **A fetch after every TPS key, as TL does** — two writes to the host per key (glyphs, then Hanji), and a walk per glyph.
- **Per-segment underlines** (`khiin-rs/windows/ime/src/tip/composition_mgr.rs:140-166`) — three shells and four wire formats for a cue the Zhuyin input methods do not draw.
- **Tab rotating the candidate in place** (`KeyHandler.mm:817-891`) — Tab is bound to a navigation row here.
- **Settings for the caret after a pick, and for the word before or after the caret** (`KeyHandler.mm:193-197`, vChewing `useRearCursorMode`) — one behaviour: the word before the caret, the caret to the end.
- **An Escape setting** (`KeyHandler.mm:893-926`) — Escape keeps D7's two steps.
- **Head auto-commit past a length** (`KeyKey41-Eten-Tribute/src/Server/KeyHandler.cpp:1964-1992`; vChewing `InputHandler_CoreProtocol.swift:983-1016`) — see H9.
- **Converting the reading before it is closed** — the engine can (it probes the inventory), but the word under the fingers would change on every glyph.
- **Hanji conversion in the TL / POJ preedit** — U8: the romanizations keep their always-on list.

## Reviews

- H-P0 pre-implementation, 2026-10-05: Codex (`codex-cli 0.160.0`, `gpt-6.1-sol`) GO-WITH-CHANGES on a first draft in which a fetch after each key returned the conversion and the commits carried its segments back. Applied: the closed-part boundary comes from the syllabifier and the shadow's barriers, with the unclosable cases named (H1); re-segmentation on completion is stated, not denied (H1); an open reading inside the tail keeps the conversion on both sides and re-walks on closing (H3); field 1 of `FetchAtPos` is reserved, so a new tag (H5); a pick that reaches the end keeps the composition (H4); one preedit write per key (H2); the picked mark also cuts next-word adjacency, compound pairs included (H5, H7); re-opening puts the raw text back in front of a kept tail, and Commit as Typed rebuilds glyphs from every segment (H3, H5); the undefined key states are named (H6); frequency lookup covers the walker's edges (H5); all three desktops flip in one phase and the engine phases stay unreachable before it (Phases).
- Where this plan goes further than the review: Codex kept the caller-carried conversion and asked for a revision the engine would check under its mutex. The plan moves the conversion into the engine's state instead (H2), which removes the race rather than validating it and is how McBopomofo and vChewing hold their walk. H-P1's pre-implementation review, below, is where this shape was reviewed.
- H-P1 pre-implementation, 2026-10-05: Codex (`codex-cli 0.160.0`, `gpt-6.1-sol`) AGREE on the conversion as engine state held in `Phase::Continuous` and computed inside the mutation — the shape the H-P0 review had not seen — and GO-WITH-CHANGES on the phase's plan. Applied: no conversion while the caret is inside the tail, instead of a display caret snapped to a word while the raw caret steps by glyph; a conversion is valid only for the source filter it was walked with, and a request without the switch ignores one the state holds; the separator barriers are read from the shadow pipeline, not derived from the merged barrier set, and the boundary is the furthest closing end among the lattice's edges; one lexicon read for the boundary and the walk, the render outside it; the walker is split by extraction only, slot 0 keeping its guard, flattening and order; a segment holds nothing H-P1 does not read.
- H-P2 pre-implementation, 2026-10-05: Codex (`codex-cli 0.160.0`, `gpt-6.1-sol`) CHANGE on the caret design (words with derived gaps, the at-end path kept from H-P1, the edit-inside path, the separator moved to the word it closes, re-open through `MoveCaret`). Applied: the at-end path re-uses only words that cover the tail from its start; a re-open walks afresh by construction instead of being detected from the text; no new intent, H5 corrected; a walk after the switch returns keeps the caret out of a word; tests for those, a separator in the open text, two nailed segments and a non-BMP word.
- H-P2 post-implementation, 2026-10-05: Codex SHIP-WITH-FIXES (two tests, added); Claude cloud review SHIP-WITH-FIXES — `cargo test --workspace`, 135k-operation differential against `main` without the switch (identical), 160k-operation caret-invariant fuzz (no violation). Applied: a reading closed at the caret converts even with open text after it (it waited for the end of the uncovered text); a caret step under a request the held conversion is not for walks first; tests for those, a pick with the caret inside the tail, a re-open that closes nothing. Not added: a property test under the switch.

- H-P3 pre-implementation, 2026-10-05: Codex (`codex-cli 0.160.0`, `gpt-6.1-sol`) CONFIRM with changes on the word's list, the pick, the commits, the picked mark and the walk's frequency rows. Applied: the frequency source is an explicit optional argument threaded through the transition, not a hidden one; the walk asks for every edge's homophones through the walker's own per-edge key derivation, in bounded queries, and nothing when no source is supplied; both fetch passes answer from one copy of the engine; the list's context word must end at its start, glyphs before it cut the context; glyphs before the start are nailed byte for byte with no word identity; next-word learning is all or nothing and the desktop forgets the context after a commit that teaches nothing (H-P4); the property test runs under the conversion with a fixture lexicon and random picks.

- H-P3 post-implementation, 2026-10-05: Codex SHIP-WITH-FIXES (context after nailed glyphs, longer words first, H7 wording — fixed); Claude cloud review (`session_0196SUH6K7X5GKLsc5cFheWF`) SHIP-WITH-FIXES — `cargo test --workspace --all-features` 1,432 passed; 48,000-step differential against the merge base without the switch, with and without the stores: byte-identical; fuzz under the conversion (120k fixture / 90k production steps, picks, both commits, re-opens): no panic, no invariant broken but one. Applied: the request is judged on the whole composition (a pick leaving tone marks only finalized); the `word_before_caret` field removed (a pick from a whole-tail list duplicated text); un-nailing to an unpicked segment answers Clear. Not changed: the edge enumeration doubles the lexicon lookups of a closing key — measured against H9 in the H-P4 dogfood.

- H-P4 pre-implementation, 2026-10-05: Codex (`codex-cli 0.160.0`, `gpt-6.1-sol`) CHANGE on the desktop plan (switch in `app_config`, commit as shown through the manager, a commit-as-typed intent, the classifier rows, the pick closing the list, the picker's commit-first). Applied: a TPS commit goes out under the settings its preedit was written with, not only with the input mode forced to TPS (changed dictionary switches would commit glyphs under Hanji); the Windows candidate list's `Finalize` sent the highlighted pick and joins the picker's `commit_first`; the engine's commit-then-insert takes no `raw` / `nailed` arguments any more; tests for a commit after the switches changed, forgetting the context and keeping the learning of an all-picked commit, the refused Space on a converted display, a rebound Commit as Typed row, Backspace after a pick. Confirmed: the engine one-liner instead of two host writes; closing the list on a settings change; Linux focus-out needs no engine commit.

- H-P4 post-implementation, 2026-10-05: Codex SHIP-WITH-FIXES — a list left up across a dictionary-switch change (a settings reload that does not go through `represent_list`) was picked from under the new switches, the engine resolved the list's start as `0`, and the pick nailed the whole tail. Fixed: the fetch and the pick of a TPS composition go out under the settings its preedit was written with, as its commits do; test `a_tps_pick_after_the_dictionaries_change_keeps_the_words_before_it` (without the fix: 罷, 家 lost).

- H-P5 pre-implementation, 2026-10-05: Codex (`codex-cli 0.160.0`, `gpt-6.1-sol`) CONFIRM on anchoring a TPS list at the mirrored display caret (every list reaches the anchor after the manager mirrors the state it was fetched for; `represent_list` closes a TPS list), on gating by `is_tps_composition` rather than the mode in force, and on renaming field 3; CHANGE on the claims and the tests. Applied: the window is not promised under the listed word at the start of the tail (As built in H-P5); tests for an open reading, the end, mid-sentence and the start of the tail after a pick (seam), the mirrored caret and its reset (desktop core), the walk's first index (Swift); §59 names the macOS window as H8; S91 revised for Space, the picks, the commits and the window-off rows; S95 with a wrapped line and TL / POJ controls.

- H-P5 post-implementation, 2026-10-05: Codex SHIP-WITH-FIXES — no regression (TL / POJ anchor values and wire tag unchanged; `desktop_shell.pb.swift` byte-identical to protoc 36.2 output); two §59 claims too broad, fixed: glyphs can sit between words and a key at the end of such a tail walks again; Space opens the list (or commits with the window off) only with the caret at the end. `/simplify`: no change (a Swift settings helper for one test not extracted).

## Dogfood

One `Sn` per phase a shell can reach (H-P4, H-P5) is added to `dogfood-checklist.md` when its PR opens, with sentences from `corpus/taigi-typing`. The engine phases (H-P1 to H-P3) cannot be typed on a device before H-P4; their check is the engine tests (`engine/composing/tests/tps_hanji_conversion.rs`, `tps_hanji_conversion_prod.rs`). S91 (a) ("typing shows no window") stays true; its picks, Space, commits and window-off rows were revised in H-P5. H-P4 added S94, H-P5 S95 (the macOS window's place).
