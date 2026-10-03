# Dogfood Checklist (real-device acceptance gate)

> **Type**: Reference (living)
> **Keywords**: `dogfood`, `acceptance`, `perf-gate`, `invariant`
> **Related**: behavioral-invariants.md, ../contributing/known-pitfalls.md

The qualitative perf gate (`docs/contributing/known-pitfalls.md` § Review and device gates) made concrete. Read this **when preparing a device dogfood pass or when a PR / memory entry cites an `Sn` item** — it is not always-on context.

- Base checklist: **S1 POJ diacritics**, **S2 TPS composition**, **S3 Hanji candidate scroll**, plus iOS keyboard-extension 64 MB hard cap, leak-free + no-keyboard-dismiss. Translates "perceptible regression on real interactive sequences" into a concrete acceptance gate.

A pending **Sn** = a dogfood acceptance item written in full: **type X → expect Y** + the `INVARIANT_*` it pins. Full root-cause / fix-file receipts live in `docs/architecture/behavioral-invariants.md §N` — not duplicated here. Recurring patterns (state once):

- **Evasion shape** (S4/S5/S9/S11/S13/S15/S17–S21): every hermetic fixture used a single reading / family / initial-form, so the production collision never fired in tests → real-device + the `engine/composing/tests/candidate_dump.rs` dev harness (production artifacts) are the catch-net. Android JVM can't load the `.so`/SQLite, so SQL-path items are dogfood-pinned there.
- **Fix-location lesson** (S5/§18, reused by S9/S17/S22): a display-only candidate-strip change belongs in the span-local key builder (`composing::shadow` / display-layer seam), never the shared segmentation primitive (`syllabifier::valid_span_endings`) — touching the primitive reintroduces the #290 non-greedy-recovery regression.
- **Build gate**: engine-touching items need `make build` (refresh xcframework/jniLibs) before device dogfood; `make dict` only when dict artifacts change. Platform-only items skip both.

Index (S7 intentionally absent). Every one-line item below passed device dogfood by 2026-10-02 (USER: "all dogfood passed"); an item written in full is pending. The full "type X → expect Y" text of each item lives in git history (`git log -p -- docs/architecture/dogfood-checklist.md`); root cause and fix live in `behavioral-invariants.md §N`. A new item is added in full while pending and collapsed to one index line once it passes.

- **S4** explicit-tone candidate filter — 2026-05-30 · §17 · #367
- **S5** longest-match prefix suppression — 2026-05-31 · §18 · #371
- **S6** first-candidate keycap-color hint — 2026-06-01 · §19 · #373
- **S8** leading khinsiann `--` literal — 2026-06-02 · §21 · #379
- **S9** continuous slot-0 respects dict separator — 2026-06-02 · §22 · #380
- **S10** auto-space attaching-punctuation swap — 2026-06-02 · §23 · #381
- **S11** association cross-mode recall + no duplicate — 2026-06-03 · §24 · #382
- **S12** Continuous-input commit carries canonical TL (write-layer root fix) — 2026-06-03 · §25
- **S13** Custom words findable across input modes — 2026-06-03 · §26 · #384
- **S14** Custom-word row-count cap parity — 2026-06-03 · §27
- **S15** Polyphonic-character frequency (Hanji,canonical-TL) pair-key — 2026-06-04 · §28 · #386
- **S16** user data excluded from OS auto-backup — 2026-06-04 · §29 · #388
- **S17** Literal composing — TL+POJ, no spelling conversion — 2026-06-05 · §30 · #390
- **S18** TPS space key = soft syllable separator (tone-1 continuous input) — 2026-06-05 · §31 · #391
- **S19** TPS stop-coda gate — tone 1 without space — 2026-06-05 · §32 · #392
- **S20** TPS nasal-coda gate — tone 1 without space — 2026-06-06 · §33 · #394
- **S21** TPS de-fold enumerate — hidden words surface as candidates — 2026-06-06 · §35 · #396
- **S22** Literal romanization candidate — fast Hanji-with-romanization input — 2026-06-06 · §34 · #395 · #397
- **S23** TPS ambiguity-aware lookup — one-key-many-glyph ambiguity is deferred to the dictionary — 2026-08-19 · §35
- **S24** TPS space pins the unmarked tones (tone 1 / tone 4) — 2026-08-20 · §41
- **S25** A candidate may not have more syllables than the input — 2026-08-21 · §43
- **S26** Candidate display = Romanization Only — 2026-09-01 · §44 · §42 · #7
- **S27** Candidate display = Hanji with Romanization (Hanji and roman mixed) — 2026-09-01 · §42 · #666
- **S28** Nasal rime `o͘ⁿ` alias spellings + double-tap nn/oo — 2026-09-04 · §45 · #685 · #686
- **S29** Windows app name follows the system locale — 2026-09-05
- **S30** macOS font-management pane + custom font upload — 2026-09-08
- **S31** Windows font-management pane + custom font upload — 2026-09-08
- **S32** Telex tone keys — 2026-09-09 · #17 · #19
- **S33** Candidate-window toggle — 2026-09-09 · #20
- **S34** Telex guide shortcut — 2026-09-09 · #21 · #22
- **S35** The literal roman cell has no slot key — 2026-09-09 · §34
- **S36** Symbol picker — 2026-09-09 · #26
- **S37** Composing caret moves left / right — 2026-09-09
- **S38** ⇧ + slot key = pick a cell in the other script for this commit — 2026-09-10
- **S39** Custom font delete-then-re-add + row name = filename — 2026-09-10 · #37
- **S40** Adding a font no longer crashes; list selection stays put — 2026-09-11
- **S41** Typing with a custom font no longer kills the host — 2026-09-11
- **S42** Font-management lists installed typefaces (macOS) — 2026-09-11
- **S43** Font-management lists installed typefaces (Windows) — 2026-09-11
- **S44** NextWord prediction row follows Candidate Display + the literal cell inherits the dictionary identity — 2026-09-12
- **S45** First key after switching is no longer lost (Android) — 2026-09-12 · §13
- **S46** Partial-tone input keeps the typed tone (all four platforms) — 2026-09-14 · §17
- **S50** Abbreviation input reaches the abbreviated words (all four platforms) — 2026-09-18 · §46
- **S51** Symbol picker keys stay in the picker under Chromium hosts (macOS) — 2026-09-18 · §47
- **S52** Hanji-first out of the box, and the desktop General pane owns its output script and reset (all four platforms) — 2026-09-18 · §48
- **S53** Symbol picker: `⋯` and the recent picks lead (macOS + Windows) — 2026-09-19
- **S54** Custom theme: one background surface, gradient direction, scheme-invariant colours, new editor order (iOS PR A; Android PR B) — 2026-09-19
- **S57** Telex `x` = tone 1 / 4, `v` = tone 2 / 8 (macOS + Windows) — 2026-09-19
- **S55** Custom theme: photo background (iOS PR C; Android PR D) — 2026-09-20
- **S58** Shortcut recorder takes Enter on any composing row, and bare Tab (macOS + Windows) — 2026-09-19
- **S59** Input-source menu lists the global shortcuts (macOS + Windows) — 2026-09-19
- **S56** Closed tone-8 syllable no longer trails its shorter prefix family (all four platforms) — 2026-09-19 · §18
- **S60** No Hyphens: candidates and commits drop the dictionary hyphen, khinsiann `--` becomes `·` (all four platforms) — 2026-09-20 · §49
- **S61** About page from the input-source menu (macOS + Windows) — 2026-09-20
- **S62** Learned phrases — a phrase composed segment by segment becomes one candidate (all four platforms) — 2026-09-20 · §34 · #109
- **S63** Ctrl + punctuation types the other width once (macOS + Windows) — 2026-09-20 · #119
- **S64** Shortcuts pane shows the three fixed rows: Pick a Candidate Directly / Move Through Candidates / Delete What You Are Typing (macOS + Windows) — 2026-09-20
- **S65** Settings rows in pipeline order (macOS + Windows) — 2026-09-21
- **S66** Typed separator joins the continuous best reading (all four platforms) — 2026-09-21 · #129
- **S67** Frequency Records / Association Records pages removed, recording always on (iOS + Android) — 2026-09-22 · #130
- **S68** Typed separator: committed between picks, remembered by the learned phrase (all four platforms) — 2026-09-22 · #131
- **S69** Typed hyphen is a syllable boundary (all four platforms) — 2026-09-22 · #5655
- **S70** "ⁿ becomes ᴺ in capitals" toggle (all four platforms) — 2026-09-22 · §53
- **S71** Typing after a nail keeps the Hanji-first prefix unspaced (iOS + Android) — 2026-09-22 · §54 · #103
- **S72** Caps Lock keeps a POJ custom entry all caps (all four platforms) — 2026-09-22 · #89
- **S73** Typed hyphen renders inside a dictionary word (all four platforms) — 2026-09-22 · §55 · #5517
- **S74** Linux first machine — Fcitx5 on KDE Plasma, then IBus on GNOME (Ubuntu 24.04 VM) — 2026-09-23
- **S75** One size pop-up for the candidate window, Standard a step smaller (macOS + Windows) — 2026-09-23
- **S76** Linux bundled typefaces are system fonts; no Manage Typefaces pane (Ubuntu VM, both shells) — 2026-09-24
- **S77** Identical desktop menus; no update check on Linux (Ubuntu VM, KDE + Fcitx5 then GNOME + IBus; macOS; Windows) — 2026-09-25 · #193
- **S78** Custom theme: one Key Fill row, white by default (iOS + Android) — 2026-09-26
- **S79** Custom theme card: background only, no sample key (iOS + Android) — 2026-09-26
- **S80** Custom theme photo: decoded off the main thread (iOS + Android) — 2026-09-26
- **S81** One-handed mode from the toolbar keyboard button (iOS + Android) — 2026-09-27
- **S82** About page: Facebook + Instagram + Threads rows (all five platforms) — 2026-09-27
- **S83** Next-word strip predicts from the committed word (iOS + Android) — bigram LM P4a
- **S84** Composing candidates re-ranked by the previous word (iOS + Android) — bigram LM P5
- **S85** A continuous composition teaches its word pairs (iOS + Android) — bigram LM P4b
- **S86** Android: a selected word ending in sentence punctuation is still learned — 2026-09-30 · §40
- **S87 Next-word: a clause mark breaks the context** (USER 2026-10-02 「切斷」, §40 `INVARIANT_NEXTWORD_CLAUSE_MARK_BREAKS_CONTEXT`; engine change → `make build` first; every platform — the rule is in `engine/nextword`). Hanji output, TL; every step within 10 s of the previous tap. **Setup**: type `tsiah` → tap 食 → scroll 食's next-word strip to its end and pick a word that is **not** in it (e.g. 冊 `tsheh`); if it is there, pick another. **(a) Each mark clears the strip**: type `gua` → tap 我; `beh` → tap 欲; `tsiah` → tap 食; type `，` on the keyboard → the strip clears. Same for `、`, `；`, `：`; in Romanization output (candidates show `guá` / `beh` / `tsia̍h`) for half-width `,`, `;`, `:`. **(b) The pick after the mark is the new context**: after `，`, type `tsheh` → tap 冊 → the strip shows the same predictions as tapping 冊 on its own (from 冊, nothing from 食). **(c) No pair across the mark**: do 我欲食，冊 three times, then type `tsiah` → tap 食 → **冊 is not in the strip**. **Negative control**: type `tsiah` → tap 食, `tsheh` → tap 冊 with no mark, twice → then 食 → **冊 appears** in the strip (the pair is learned when nothing separates the words).
- **S88 macOS: the desktop-core key path is the default** (macOS desktop-core roadmap P12, `docs/architecture/macos-desktop-core-roadmap.md` D9.5; the one device pass before P13 deletes the Swift key path). **Build**: `make build`, then `make -C macos install` from the P12 branch; log out and back in if an old input-method process survives. Every step must behave as the released 3.6.x app does — any difference is a P12 regression, not a new rule (the accepted differences are listed in the P12 PR). Hosts: TextEdit, Notes, a Safari text area, Terminal, and a Chromium / Electron host (Chrome, VS Code). A step you are unsure of: note it in the report — it is checked against the legacy back end in the test suite. Sentences from `corpus/taigi-typing` (教典): A 紅嬰仔哭甲一身軀汗。 `Âng-enn-á khàu kah tsi̍t sin-khu kuānn.` (https://sutian.moe.edu.tw/und-hani/su/1); B 熱人到矣，電風好提出來矣。 `Jua̍h--lâng kàu--ah, tiān-hong hó the̍h--tshut-lâi--ah.` (su/11211); C 莫講大人，這个道理連囡仔都知影。 `Mài-kóng tuā-lâng, tsit ê tō-lí liân gín-á to tsai-iánn.` (su/7836). **(a) Typing + caret geometry**: Output Script Hanji, Numeric Tones — type A word by word (`ang-enn-a` → 紅嬰仔, `khau` → 哭, …, `.` → `。`) in each host; the marked text is underlined at the caret and the candidate window sits under it — at a line end, after the text wraps to a new line, near the screen's bottom edge (the window flips above), in a scrolled Safari text area. **(b) Long lists**: type `si` → page with `]` / `[` (the default page keys) and the arrow keys to page 3+, pick a cell there by its slot number → that cell commits, and the window keeps its place while paging; Escape on a fresh `si` empties the composition and closes the window. **(c) Held keys (auto-repeat)**: hold `a` mid-composition → every repeat lands, none dropped or doubled; hold Delete → the composition shrinks one letter per repeat, the window closes when it empties, then the repeats delete host text as usual; in `sinkhu` hold ⌥← → the composition caret walks left and stops at the start (bare ← with a list up moves the highlight instead). **(d) Focus changes**: type `tsiah` (do not pick) → click elsewhere in the same document → the marked text as shown is committed, once; type `tsiah` → ⌘Tab to Notes and back → the same: committed as shown in TextEdit, the window gone, the next key starts a new composition; with a list up, open Spotlight (⌘Space) and type there → no candidate window is left behind over TextEdit. **(e) Partial pick (a nailed segment)**: type `tsiahpng`, highlight a one-syllable 食 and pick it → 食 is nailed, `png` keeps composing; then (1) ⌃⌘, → the highlighted cell is committed for the picker — if that only nails another segment the picker does not open — end the composition, then ⌃⌘, opens it; (2) again from 食 + `png`, ⌘Tab away → the marked text as shown commits once, nothing doubled. **(f) Re-entrancy**: type B quickly in Terminal and in the Safari text area — no duplicated or lost characters; mid-composition ⌘Z / ⌘A behave as in 3.6.x. **(g) Symbol picker**: ⌃⌘, idle → pick `「`; with `tsiah` composing and a candidate highlighted → that candidate commits first, then the picker opens and `「` follows it; Escape closes the picker. **(h) Auto-Space swap**: Output Script Romanization, Auto-Space on, Show Typed Text First off (the default) — `gua` → Return (`guá` highlighted) → `guá ` → `?` → `guá? ` → `!` → `guá?! `; move the caret with ← before typing `?` → no swap (`?` lands at the caret). **(i) Full-width**: Output Script Hanji — type B; `,` after 矣 → `，`; the Punctuation in the Other Width chord (⌃`,`) → `,`. **(j) Telex**: Settings → Telex; `angd` `f` `enn` `f` `av` typed as one composition → `âng-enn-á` / 紅嬰仔; end it, then `tsiahv` → `tsia̍h`; ⌃⌘/ shows the Telex guide, Escape hides it. **(k) Next-word context across apps** (the desktop learns, never predicts — check the store): in TextEdit `gua` → pick 我, then within 10 s `tsiah` → pick 食 → `sqlite3 -readonly ~/Library/Application\ Support/com.siansiansu.inputmethod.TaigiKeyboard/user_association.db "select prev_word,prev_tl,next_word,next_tl,count from user_association where prev_word='我' and next_word='食'"` shows the pair read `guá` → `tsia̍h` (count +1 on a repeat). **Negative control**: pick 我 in TextEdit, ⌘Tab to Notes within 10 s and pick 食 there → that row's count does **not** change. **(l) Settings while composing**, with a list up: ⌃⌘H cycles Candidate Display and the open list is refetched; ` (Switch Hanji or Romanization) under Hanji–Romanization Pairing swaps the leading script and repaints the list in place (highlight kept), under Hanji with Romanization flips only the punctuation width, under Romanization Only does nothing; turning Show Candidate Window off in Settings takes the list down, and typing on behaves as in 3.6.x. **(m) Show Typed Text First on** (Settings): type `taigi` → the first cell is the typed text with no slot number and the slot numbers start on the second cell; Return / Space / the slot keys commit as in 3.6.x; set it back off. **(n) A rebound composing key**: Settings → Shortcuts, record another chord on the Confirm Key row → mid-composition the new chord confirms at once and the old one no longer does; set the row back. **(o) POJ**: with a list up, ⌃⌘C (Switch Tâi-lô or Pe̍h-ōe-jī) → the list comes down and input is POJ; `chiah` → `chia̍h` / 食; ⌃⌘C back to TL.
- **S89 Windows + Linux: Switch TPS and the romanization it returns to** (desktop TPS roadmap P3, § D5; `make build` first — desktop core change). Hosts: Notepad / a browser text area on Windows; gedit / a browser on Linux under IBus and under Fcitx5. Words are typed as TPS on the Dachen layout (`e` ㄍ, `8` ㄚ, `1` ㄅ; roadmap § D2). **(a) Round trip from POJ**: Settings → General → Input Script = Pe̍h-ōe-jī → close Settings; Ctrl+Alt+P → the flash / panel label reads 方音符號 (Linux indicator 方音); type `e` `8` → `ㄍㄚ` and a list led by 家-family Hanji; Space confirms the highlighted cell; Ctrl+Alt+P → label reads 白話字 (not 台羅). **(b) Switch Romanization from TPS**: Ctrl+Alt+P (TPS, last used POJ) → Ctrl+Alt+C → Tâi-lô (the romanization NOT last used). **(c) A composition across the switch**: in TPS type `e` `8` (do not pick) → Ctrl+Alt+P → the list closes, `ㄍㄚ` stays underlined; type `a` → `ㄍㄚ` is committed as shown and `a` starts a romanization composition. Same from the menu: tray button (Windows) / panel menu (Linux) → Switch Phonetic Symbols. Same from Settings: with `ㄍㄚ` underlined, open Settings (Ctrl+Alt+S, Open Settings Menu), pick Tâi-lô, return to the host, type `a`. **Negative control**: TL `ta` (not picked) → Ctrl+Alt+C → type `i` → one composition `tai`, nothing committed. **(d) Menus and the Shortcuts pane under TPS**: the menu shows Switch Tâi-lô or Pe̍h-ōe-jī, Switch Phonetic Symbols, no Switch Candidate Display; Settings → Shortcuts shows Pick a Candidate Directly = `Num 1–9` and no Output the Other Script (Hanji / Romanization) row; back under TL both rows read as before. **(e) Rebind and reset**: record another chord on Switch Phonetic Symbols → the new chord switches, Ctrl+Alt+P no longer does; Shortcuts reset restores Ctrl+Alt+P. General reset → Tâi-lô, and the next Ctrl+Alt+P round trip returns to Tâi-lô.
