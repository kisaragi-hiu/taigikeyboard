## v3.6.12

Mobile release for iOS and Android. Headline: **next-word suggestions and
continuous-input candidates learn from whole words** — predictions follow the
word you just picked, words you have chosen always come before built-in ones,
and continuous-input candidates are ranked by the word in front of them. Most
of the range is engine and platform refactoring with no visible change.

### Shared (iOS + Android)

#### Improved

- **Word-level next-word suggestions.** Predictions look up the whole word you
  just committed (Hanji + reading) and fall back to its last character only
  when the word has no entry. A prediction you have picked before now always
  ranks above every built-in one. (#270)
- **A continuous composition is learned as one sequence.** Committing a
  multi-word continuous composition teaches every adjacent word pair in it, not
  only the last two; segments you abandoned no longer leak into the context.
  (#271)
- **Continuous candidates ranked by the previous word.** Among same-length
  homophones, the candidate that usually follows the previous word (the last
  word you fixed in the composition, or the last word committed within 10 s)
  is offered first. Segmentation itself is unchanged. (#272)

#### Bug Fixes

- **Punctuation typed outside a composition ends the next-word context.**
  Typing `。` after a word now ends the sentence for predictions, so the next
  word is not suggested or learned as a continuation across sentences. (#273)
- **All dictionaries off now offers no dictionary candidates.** Turning every
  dictionary source off used to bring all of them back. (#298)
- **Phonetic Symbols layout: a Hanji-less candidate writes its Bopomofo.**
  Picking a candidate with no Hanji (for example `tâi`) writes the symbols the
  cell shows (`ㄉㄞˊ`) with no auto space, instead of the romanization. Hanji
  candidates and the TL / POJ layouts are unchanged. (#309)
- **Custom-entry dialog examples.** The romanization field now shows a
  romanization example (`gâu-tsá`) and the Hanji field a Hanji example (`𠢕早`)
  in every UI language; they were swapped. (#279)

#### Changed

- **Keyboard look from before themes is kept as a theme.** A look customized
  with the pre-theme size / corner / border / color settings is carried over
  once into a user theme named "New Theme" and selected when no other theme
  was in use; an unchanged look simply drops the old settings. (#293)

### iOS

#### Bug Fixes

- **Candidate bar no longer blurred on iOS 26 and later.** The system scroll
  edge effect blurred the top of the candidate strip, making the Hanji line
  hard to read. (issue #304, #319)
- **Hanji-less candidate when Hanji is output first.** With output swapped to
  Hanji first, a candidate without Hanji now writes its romanization
  with an auto space; Annotate in Brackets no longer writes it twice
  (`taigi (taigi)`). (#296)

#### Changed

- **Carried-over look needs one app launch.** When the keyboard opens before
  the app without Full Access, the pre-theme look cannot be saved as a theme
  yet: the keyboard shows the default look until the app is opened once. The
  old settings are kept meanwhile. (#293)

### Android

#### Bug Fixes

- **Dictionary tab follows Enable Custom Dictionary and searches Phonetic
  Symbols.** With Enable Custom Dictionary off, custom entries no longer appear
  in the Dictionary tab, and a Bopomofo query on the Phonetic Symbols layout
  finds words. (#278)
- **Words ending in sentence punctuation are learned.** Selecting a candidate
  such as `多謝！` now teaches the next-word pair, as on iOS. (#280)

#### Changed

- **Retired settings with no UI.** Leftover settings from very old installs
  with no screen to change them reset to today's defaults; a hidden launcher
  icon is restored. (#277)
