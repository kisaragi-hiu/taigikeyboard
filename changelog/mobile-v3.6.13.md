## v3.6.13

Mobile release for iOS and Android. Headline: **Learning Records** — a new
page to review, correct or delete what the keyboard has learned, and to add a
learned word to the custom dictionary. Settings are regrouped, dictionary
sources get their own Manage Dictionaries page, and No Hyphens becomes a
three-way Syllable Separator. Most of the range is desktop work (macOS over the
shared desktop core, desktop TPS mode) with no effect on phones.

### Shared (iOS + Android)

#### New

- **Learning Records page.** Dictionary tab → Learning Records opens Word
  Frequency and Phrases: search, sort by most used / most recent, change a
  word's count (1–1 000 000) or delete one row. Add to Custom Dictionary copies
  a word-frequency row (the row stays, it is the word's ranking weight) and
  moves a learned phrase; only rows with Hanji offer it. Delete Learning
  Records now lives on this page and leaves custom words alone. (#366, #371,
  #410, #415, #422, #425, #427)
- **Syllable Separator replaces No Hyphens.** Hyphen (default, `tâi-uân`),
  Space (`tâi uân`) or None (`tâiuân`); the neutral-tone `·` stays on its
  syllable (`hōo ·guá`). Display only — learning and lookup are unchanged. An
  install with No Hyphens on carries over as None. (#423)
- **Nasal mark in POJ capitals is a picker.** Choose `ᴺ` (`SIÂᴺ`, default) or
  `ⁿ` (`SIÂⁿ`); the previous toggle value carries over. (#406)
- **Manage Dictionaries page.** Dictionary-source toggles and the dictionary
  search moved from the Dictionary tab root to their own page. The Dictionary
  tab now lists Manage Dictionaries, Custom Dictionary, Learning Records and
  Backup and Restore. (#416, #419)

#### Changed

- **Settings regrouped, one row style.** Settings tab and the keyboard's
  settings panel share one order: Display Language, Input Mode, Typing,
  Keyboard (Font joins it), Pe̍h-ōe-jī, Phonetic Symbols, Key-Press Feedback,
  then Device Info and Reset. Setting rows lose their leading icons. (#430,
  #432)
- **Reset Settings resets settings only.** It no longer wipes learning
  records; delete them from the Learning Records page. (#433)
- **Show Typed Text First is off by default.** A value you set yourself is
  kept. (#337)
- **Clause punctuation ends the next-word context.** `，、；：` and `, ; :`
  break predictions and learning like `。！？`, so no word pair is learned
  across a clause. (#341)
- **No example words on a fresh install.** The custom dictionary starts empty;
  existing installs keep the two examples already added. (#426)
- **Backup format version 3.** `.taigi` backups keep readings verbatim (TL
  `eng` / `ek` finals no longer folded); older backups restore as before. (#389)
- Wording: Learning Records labels and failure notices in Taigi revised; the
  Learning Records count is called Quantity. (#414, #421, #424)

#### Bug Fixes

- **The separator form you pick stays first.** Picking `gín-á` for 囡仔 no
  longer leaves `gín--á` in front (and the reverse); applies to every word
  with both forms, such as 出來 and 頭家. (#418)
- **Custom word with a spaced reading ranks first.** A custom entry such as
  `tshì-giām sû` / 試驗辭 now wins over the dictionary word, as the hyphenated
  form did. (#411)
- **Next-word learning keeps TL `eng` / `ek`.** A picked word such as
  `tsiúnn-keng-kok` is no longer learned as `tsiúnn-king-kok`. (#389)
- **Rare Hanji are recognised.** Dictionary search and continuous input
  handle CJK Extension F / G and compatibility ideographs. (#388)
- **A word ending in `$` predicts nothing.** It used to offer 30 sentence
  openers. (#340)

### iOS

#### Bug Fixes

- **Flutter apps no longer keep the typed letters.** Picking a candidate in
  apps such as Cashew wrote `taigi台語`; each key event now writes to the host
  once. (issue #352, #375)
- **Emoji keyboard top row no longer blurred on iOS 26 and later.** (#379)
- **Emoji long-press on the top rows opens below the emoji**, fully visible,
  skin-tone bar included. (#380)
- **Learning Records page:** a one-syllable row offers its swipe action
  instead of popping the page; runtime warnings on the page fixed. (#427,
  #428, #429)

#### New

- **`·` on the `-` long-press** of the QWERTY TL / POJ and PhahTaigi layouts
  and the numbers page. (#407)

### Android

#### New

- **`·` on the `-` long-press** of the QWERTY TL / POJ, PhahTaigi and symbol
  layouts. (#407)

#### Bug Fixes

- **Fast typing at sentence start capitalizes one letter.** Auto-capitalization
  produced `TÂI` instead of `Tâi` when keys came about 100 ms apart. (#412)

### Dictionary

- `association.bin` drops the `$` sentence-start key (30 entries); every other
  entry unchanged. (#340)
