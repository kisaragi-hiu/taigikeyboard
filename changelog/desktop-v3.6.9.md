# desktop v3.6.9

The settings-and-engine release. The keyboard now learns the phrases you build
segment by segment and offers them whole next time; 漢字 is the default output on
a fresh install, with a 輸出文字 picker to flip it; a 無連字符 switch drops the
dictionary's hyphens from romanization; Telex gains keys for tones 1 and 4;
`Ctrl` with a punctuation key types the other width once; the symbol picker
leads with your recent picks; the input-source menu carries the global shortcuts
and a new 關於 page; and a round of engine fixes lands typed tones, capitals,
abbreviations and user picks where they belong. Every setting row is re-ordered
to follow the typing pipeline, and much of the UI copy is reworded. macOS and
Windows have all of it.

### macOS

#### Input

- **Learned phrases.** Composing a phrase from two or more picks — 記 → 起 → 來 on
  `kikhilai` — teaches the keyboard that whole phrase; the next `kikhilai`
  offers 記起來 as one candidate. Always on; learned rows share the 自訂詞庫 table
  with a 學 badge, cap 2000, and a manual entry for the same pair always wins.
  On first launch an existing 自訂詞庫 database gains the two provenance columns
  and rebuilds its search keys once. (#109, #112, #113)
- **Telex `x` and `v` cover tones 1 / 4 and 2 / 8.** Under the Telex scheme `x`
  writes tone 1 on an open syllable and tone 4 on a stop coda (`p t k h`); `v`
  writes 2 / 8 the same way. The Telex 說明 card lists both rows, and the card
  itself is larger. (#98, #115)
- **⌃ + punctuation types the other width once.** With 漢字 output, `⌃,` inserts a
  half-width `,` inside hanji text; with 羅馬字 output, `⌃,` inserts `，`. One
  shot, the mode is untouched; ⇧ still picks the key (`⌃⇧,` → 《). Listed as a
  read-only row on 快速齒. (#119)
- **Symbol picker: recent picks first, plus `⋯`.** The last nine symbols picked
  lead the list, most recent first, then the table in file order; `⋯` sits after
  `…`, and `·` (輕聲 dot) leads the punctuation row. (#88, #108)
- **Symbol picker no longer leaks keys in Chromium apps.** In Chrome, Discord,
  Slack and VS Code the arrows both walked the list and moved the page caret, and
  Return reached the page. The picker now marks a placeholder so the host keeps
  its hands off. (#85)
- **Punctuation width follows the stored setting under 漢羅濫.** After 3.6.8 the
  `` ` `` chord went inert and punctuation locked full-width whenever candidates
  were shown 漢羅濫; the chord toggles again and 羅馬字-only output stays
  half-width. (#62)
- **Typed tone digits win.** `teng5sek` puts 程式 first instead of 等式 / 中式;
  `kokbin5tong2` finds 國民黨; `iah8` no longer trails the whole `ia` family
  after the `ia̍h` words. (#67, #71, #97)
- **Capitals survive tone placement.** Caps Lock `SIANN5` writes `SIÂᴺ` rather
  than `Siâⁿ`, and a 自訂詞 stored as `Keng-lâm Su-īⁿ` keeps its capitals when
  reached through `klsi`. (#102, #89)
- **Abbreviations look up the whole buffer.** `ss` reaches 鎖匙; aspirated
  initials count as one unit, so `phthk` reaches a 披頭巾 custom word; words with
  no initial (`âng-enn-á`) abbreviate too. (#82)
- **Your homophone pick leads.** Picking 更新 for `kingsin` puts it first next
  time, even against far commoner dictionary words, and a repeatedly picked
  whole word beats its syllable split. (#69)
- **一个 / tsi̍t-ê shows again.** Classifier 个 words were wrongly filed as
  variants of 的 and hidden while 異用字 was off. (#73)

#### Settings

- **漢字 is the default output.** A fresh install shows 漢字 as the candidate and
  羅馬字 as the annotation; an install that already pressed `` ` `` keeps its
  choice. 一般 gains a 輸出文字 picker (漢字 / 羅馬字) bound to the same setting the
  chord flips, and a 恢復預設設定 row that resets the pane — but keeps 介面語言.
  (#86, #87, #118)
- **無連字符 — romanization without dictionary hyphens.** A switch in 一般. On,
  `tâi-uân` renders `tâiuân` and the 輕聲 `--` becomes `·`: `hōo--guá` →
  `hōo·guá`. Only hyphens the dictionary or a custom entry supplies are dropped;
  what you type stays as typed. Default off. (#103)
- **Rows in typing order.** Sidebar: 一般 · 外觀 · 快速齒 · 辭典管理 · 自訂詞庫 ·
  字型管理. 一般: 輸入文字 · 聲調拍法 · 顯示選字窗 · 當咧拍的字囥第一个 · 輸出文字 ·
  無連字符 · 自動閬一个縫 · 介面語言. 外觀 and 快速齒 follow the same logic. (#121)
- **快速齒 lists the fixed keys.** Read-only rows show 直接選字 (`qwdfzxvy;` or
  `1`–`9`, following 聲調拍法), 徙動選字游標 (`← → ↑ ↓ ⇞ ⇟`) and 刪除當咧拍的字
  (`⎋`). (#120)
- **Return records on any composing row; Tab records.** Setting 輸出另外一種文字 or
  輸出當咧拍的字 to ↩ used to paint the row empty and hand ↩ back to 確定齒; it now
  sticks. A row emptied of its commit key is refilled from the default instead
  of evicting another row, and bare Tab is recordable. (#99)
- **Input-source menu carries the shortcuts.** 切換台羅/白話字 ⌃⌘C and 切換候選詞顯示
  ⌃⌘H appear as rows showing the chord you recorded, then 設定 ⌃⌘S, 檢查更新 and
  關於. (#100, #107)
- **關於台語齒盤.** A page reached from the menu: installed version, an
  introduction in 漢字 with TL / POJ, and link cards for the website, GitHub,
  Discord, email and 贊助齒盤維護運作. (#107, #116)
- **Copy reworded.** 快捷鍵 is now 快速齒 and 鍵 reads 齒 throughout; 顯示當咧拍的字
  became 當咧拍的字囥第一个; 迒模式輸出 became 輸出另外一種文字; 全形/半形 became
  全角/半角; 自動空白 became 自動閬一个縫; the Telex card says 連字符. Tâi-lô and
  POJ strings start each sentence with a capital, and the help texts are
  shorter. (#75, #104, #106, #114)

#### Candidates

- **Vertical window sized by the rows you see.** Width follows the rows the
  viewport has revealed, so one long entry deep in the list no longer widens the
  first page; scrolling into a wider row grows the window in place, and the
  annotation column is measured against the widest annotation rather than
  truncating to `k…`. (#74, #76)
- **No order animation on the candidate window.** The window appears without the
  AppKit fade, so it no longer reads as lagging the keystroke. (#70)

#### Appearance

- **Settings title hides while a pane is scrolled.** Rows no longer overlap the
  transparent title bar. (#68)
- **Menu-bar icon is a 22×16 keycap**, matching Apple's 注 / A / あ input-source
  glyphs on macOS 26. (#105)

#### Dictionary

- The kautian capture and the iTaigi source are refreshed, and abbreviation keys
  are rebuilt per leading spelling unit. (#82)

### Windows

#### Input

- **Learned phrases.** Composing a phrase from two or more picks — 記 → 起 → 來 on
  `kikhilai` — teaches the keyboard that whole phrase; the next `kikhilai`
  offers 記起來 as one candidate. Always on; learned rows share the 自訂詞庫 table
  with a 學 badge, cap 2000, and a manual entry for the same pair always wins.
  On first launch an existing 自訂詞庫 database under `%APPDATA%\TaigiKeyboard`
  gains the two provenance columns and rebuilds its search keys once. (#109,
  #112, #113)
- **Telex `x` and `v` cover tones 1 / 4 and 2 / 8.** Under the Telex scheme `x`
  writes tone 1 on an open syllable and tone 4 on a stop coda (`p t k h`); `v`
  writes 2 / 8 the same way. The Telex 說明 card lists both rows, and the card
  itself is larger. (#98, #115)
- **Ctrl + punctuation types the other width once.** With 漢字 output, `Ctrl+,`
  inserts a half-width `,` inside hanji text; with 羅馬字 output, `Ctrl+,`
  inserts `，`. One shot, the mode is untouched; Shift still picks the key
  (`Ctrl+Shift+,` → 《). Listed as a read-only row on 快速齒. (#119)
- **Symbol picker: recent picks first, plus `⋯`.** The last nine symbols picked
  lead the list, most recent first, then the table in file order; `⋯` sits after
  `…`, and `·` (輕聲 dot) leads the punctuation row. (#88, #108)
- **Punctuation width follows the stored setting under 漢羅濫.** After 3.6.8 the
  `` ` `` chord went inert and punctuation locked full-width whenever candidates
  were shown 漢羅濫; the chord toggles again and 羅馬字-only output stays
  half-width. (#62)
- **Typed tone digits win.** `teng5sek` puts 程式 first instead of 等式 / 中式;
  `kokbin5tong2` finds 國民黨; `iah8` no longer trails the whole `ia` family
  after the `ia̍h` words. (#67, #71, #97)
- **Capitals survive tone placement.** Caps Lock `SIANN5` writes `SIÂᴺ` rather
  than `Siâⁿ`, and a 自訂詞 stored as `Keng-lâm Su-īⁿ` keeps its capitals when
  reached through `klsi`. (#102, #89)
- **Abbreviations look up the whole buffer.** `ss` reaches 鎖匙; aspirated
  initials count as one unit, so `phthk` reaches a 披頭巾 custom word; words with
  no initial (`âng-enn-á`) abbreviate too. (#82)
- **Your homophone pick leads.** Picking 更新 for `kingsin` puts it first next
  time, even against far commoner dictionary words, and a repeatedly picked
  whole word beats its syllable split. (#69)
- **一个 / tsi̍t-ê shows again.** Classifier 个 words were wrongly filed as
  variants of 的 and hidden while 異用字 was off. (#73)

#### Settings

- **漢字 is the default output.** A fresh install shows 漢字 as the candidate and
  羅馬字 as the annotation; an install that already pressed `` ` `` keeps its
  choice. 一般 gains a 輸出文字 picker (漢字 / 羅馬字) bound to the same setting the
  chord flips, and a 恢復預設設定 row that resets the pane — but keeps 介面語言.
  (#86, #87, #118)
- **無連字符 — romanization without dictionary hyphens.** A switch in 一般. On,
  `tâi-uân` renders `tâiuân` and the 輕聲 `--` becomes `·`: `hōo--guá` →
  `hōo·guá`. Only hyphens the dictionary or a custom entry supplies are dropped;
  what you type stays as typed. Default off. (#103)
- **Rows in typing order.** Sidebar: 一般 · 外觀 · 快速齒 · 辭典管理 · 自訂詞庫 ·
  字型管理. 一般: 輸入文字 · 聲調拍法 · 顯示選字窗 · 當咧拍的字囥第一个 · 輸出文字 ·
  無連字符 · 自動閬一个縫 · 介面語言. 外觀 and 快速齒 follow the same logic. (#121)
- **快速齒 lists the fixed keys.** Read-only rows show 直接選字 (`qwdfzxvy;` or
  `1`–`9`, following 聲調拍法), 徙動選字游標 (`← → ↑ ↓ PgUp PgDn`) and
  刪除當咧拍的字 (`Esc`). (#120)
- **Enter records on any composing row; Tab records.** Setting 輸出另外一種文字 or
  輸出當咧拍的字 to Enter used to paint the row empty and hand Enter back to
  確定齒; it now sticks. A row emptied of its commit key is refilled from the
  default instead of evicting another row, and bare Tab is recordable. (#99)
- **Tray menu carries the shortcuts.** 切換台羅/白話字 `Ctrl+Alt+C` and
  切換候選詞顯示 `Ctrl+Alt+H` appear as rows showing the chord you recorded, then
  設定 `Ctrl+Alt+S`, 檢查更新 and 關於; a shortcut row acts on the window that had
  focus before the menu opened. (#100, #107)
- **關於台語齒盤.** A page reached from the tray menu: installed version, an
  introduction in 漢字 with TL / POJ, and link cards for the website, GitHub,
  Discord, email and 贊助齒盤維護運作. (#107, #116)
- **Copy reworded.** 快捷鍵 is now 快速齒 and 鍵 reads 齒 throughout; 顯示當咧拍的字
  became 當咧拍的字囥第一个; 迒模式輸出 became 輸出另外一種文字; 全形/半形 became
  全角/半角; 自動空白 became 自動閬一个縫; the Telex card says 連字符. Tâi-lô and
  POJ strings start each sentence with a capital, and the help and installer
  texts are shorter. (#75, #104, #106, #114)

#### Candidates

- **Vertical window sized by the rows you see.** Width follows the rows the
  viewport has revealed, so one long entry deep in the list no longer widens the
  first page; scrolling into a wider row grows the window in place, and the
  annotation column is measured against the widest annotation rather than
  truncating to `k…`. (#74, #76)
- **No DWM fade on the candidate popup.** The ~200 ms show / hide fade is
  disabled for the candidate window and the mode flash, so they no longer read
  as lagging the keystroke. (#72)

#### Dictionary

- The kautian capture and the iTaigi source are refreshed, and abbreviation keys
  are rebuilt per leading spelling unit. (#82)
