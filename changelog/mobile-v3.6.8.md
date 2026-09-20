## v3.6.8

Mobile release for iOS and Android. Headline: a **候選詞顯示 picker** — candidates can now be shown as 漢羅對應 (the previous side-by-side cell, renamed), 漢羅濫 (漢字 and 羅馬字 as separate cells in one list), or 羅馬字 only — and **顯示當咧拍的字**, which offers the literal text being typed as the first candidate, now on by default. POJ tone placement and the `o͘ⁿ` nasal spelling are corrected, automatic spacing follows what a commit actually wrote, and Android's candidate strip no longer lags on every keystroke. The numbers between v3.6.5 and v3.6.8 were used by the desktop train (v3.6.7, v3.6.8 for macOS + Windows); mobile skips straight from v3.6.5.

### Shared (iOS + Android)

#### New

- **候選詞顯示 picker.** A three-way setting chooses how each candidate cell reads: **漢羅對應** — 漢字 with its romanization underneath, the previous layout under a new name; **漢羅濫** — every dictionary hit becomes two adjacent cells, a 漢字 cell and a 羅馬字 cell, so either script is one tap away and each cell commits exactly what it shows; **羅馬字** — romanization only, with same-reading rows (食 / 𤆬 `tsia̍h`) collapsed to one cell. Changing the picker re-fetches the open candidate list so the collapse applies immediately. The 漢羅 Space swap is unchanged in every mode. Next-word predictions follow the same picker: under 漢羅濫 each prediction becomes a 漢字 cell and a 羅馬字 cell, under 羅馬字 the romanization alone. (#662–#668, #674, #60)
- **顯示當咧拍的字, on by default.** Renamed from 顯示原本羅馬字候選 and now enabled for new installs and default-reliant users: the first candidate is always the preedit exactly as typed, so an out-of-dictionary name or spelling is one tap away. Anyone who turned the old setting off stays off. (#673)

#### Bug Fixes

- **POJ tone marks on `au` before a coda.** "Mark the second vowel in a closed syllable" is a POJ exception owned by `oa` / `oe` only (`oa̍h` 活, `choân` 全), but the rule also fired on `au`, so 落 rendered `lau̍h` instead of `la̍uh`. `ere` and `iri` now mark the trailing vowel (`erê`, not `e̍re`), matching the MOE manual. A 20088-combination sweep is byte-identical to the canonical converter; dictionary POJ readings for 落, 沓, 貿, 雹, 哮 corrected accordingly. POJ mode only. (#622)
- **The `o͘ⁿ` spelling of the nasal final is indexed.** Canonical spelling is `onn` / `oⁿ`, but the MOE2 POJ layout's `o͘` + `nn` keys produce `ho͘nn`, which found nothing in POJ, while in TL the whole-buffer `oonn→onn` fold fired across syllable seams and lost 滷卵 `lo͘nng` and 可惡 `khooⁿ`. The alias is now emitted per syllable at dictionary build time, so `ho͘nn` / `hoonn` reach 好 / 否 / 呼 / 齁 and the seam cases keep their candidates. (#685)
- **POJ double-tap `oo` then `nn` folds both.** With both double-tap affordances on, `hoonn` showed `ho͘nn`: the `oo` fold inserted the combining dot exactly where the `nn` rule looked for a plain vowel. The nasal fold now runs first, as in the canonical converter. Also fixed: a shift between the two `o` taps (`hoO`) folded nothing, and `OOoo` stacked three dots on one letter — the fold is a single left-to-right scan, first tap deciding the case. (#686, #687)
- **Automatic spacing follows what the commit wrote, not the output mode.** The space gate asked whether the *mode* leads with romanization, so a candidate that carried no 漢字 — the literal-preedit candidate, an out-of-dictionary name, a romanization-only custom entry — lost its space in 漢字優先 and 漢羅濫 even though it wrote romanization. The verdict now travels with the string chosen for the document. A space the user typed is no longer swapped by attaching punctuation, and Enter on a raw TPS buffer no longer earns a space. (#670)
- **No duplicate cells under 漢羅濫 / 羅馬字.** Single-script display hides what tells two cells apart, so a cell reading exactly like an earlier one is a defect: 重/tîng and 重/tāng now draw one 重 cell while both 羅馬字 cells stay. 漢羅對應 is untouched — its subtitle disambiguates. (#674)
- **The typed-literal candidate learns and predicts like the word it stands for.** Under 漢羅濫 and 羅馬字 the literal cell absorbs the same-reading dictionary row, but it kept only its romanized text, so committing it fed no next-word predictions and 詞頻 learned a separate romanized bucket. The literal now inherits the absorbed row's identity: the cell still reads and writes the exact preedit, and predictions and frequency follow as they do for the dictionary cell. 漢羅對應 untouched. (#60)
- **Punctuation width no longer locks under 漢羅濫.** The 文/A flag carried two meanings — candidate lead script and punctuation width — and 漢羅濫 forced the first, so the second was stuck at full-width with no key to change it. The two are now separate: under 漢羅濫 the 文/A key (and the desktop `` ` `` shortcut) is back and flips punctuation width only, cells stay split; 羅馬字 stays half-width with the key hidden; 漢羅對應 unchanged. Under TPS punctuation is always full-width on every page and 文/A is hidden. (#62)

#### Changes

- **The 文/A key hides under 羅馬字 and TPS.** With nothing to flip it sat visible but inert; the space bar takes its width and the expanded-candidate panel drops its swap button. Under 漢羅濫 it stays as the punctuation-width toggle (see Bug Fixes). (#58, #62)
- **Labels**: 漢羅並排 → 漢羅對應; the display-language endonym 台漢 → 漢字.

### iOS

#### Changes

- **Default keyboard font is the system font.** New installs and anyone who never picked a font get the system font instead of 芫荽 (open-huninn); a stored font choice is kept. (#63)
- The 候選詞顯示 menu in the toolbar settings overlay no longer paints its label in accent blue. (#59)
- The vendored ISEmojiView bundle drops 30 loose PNGs that duplicated its asset catalog (196 KB → 68 KB). (#15)

### Android

#### Bug Fixes

- **The first key after an app or keyboard switch is no longer lost.** Two host callbacks — a selection report of `-1/-1` and a same-editor restart — were treated as authoritative even when older than the keyboard's own last composing write, so the in-flight candidate fetch was dropped and the second key replaced the first. The keyboard now re-reads the editor before discarding composing state and re-asserts the composing span when the preedit is still there. (#61)

#### Improvements

- **The candidate strip no longer lags every keystroke.** Candidate fetches ran on the UI thread — two engine calls per key, ~40 ms each on a one-letter prefix — behind a fixed 50 ms debounce that dated from the Kotlin autocomplete era. The fetch now runs on a worker thread with no delay, for Taigi and English alike; results are published only if the strip was not cleared meanwhile. The key-preview popup keeps its composition alive across presses instead of rebuilding the theme on every key, and cursor-caps queries to the editor are skipped whenever they cannot change shift state. Emulator release build: P99 frame 101 ms → 18 ms. (#48–#54)
- **Word-initial candidate lookup is bounded.** The first letter of a word scanned the entire index family (~140k entries) before capping; the scan now widens in length bands and stops at the first that fills the cap. `tl:t` 38 ms → 1 ms with identical output. Shared engine change; Android was the platform where it showed. (#51)

### Dictionary

#### Changes

- `taigi-converter` bumped for the `au` coda fix and for `normalizeToTl` reversing the traditional-POJ vowels `ṳ` / `o̤` / `e͘`. Those vowels had leaked into the TL column verbatim (`('書','chṳ')`, `('鷄','ko̤e')`), so 81 untypeable `(漢字, TL)` rows now normalize into their existing entries and 16 rows gain a real TL reading; net 130,689 → 130,624 entries. (#622)
- 教典 source capture refreshed (`kautian.ods`); readings and entries regenerated from it.
- Dictionary artifacts regenerated for the release; the four platforms now ship one shared `dictionaries/` directory instead of per-platform copies. (#14)
