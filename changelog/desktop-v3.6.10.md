# desktop v3.6.10

A Linux-only patch. The Fcitx5 candidate window shows its selection keys
again, the arrow keys follow a vertical candidate window, and the 字型管理
page is gone from the Linux settings because the candidate window's font is
set in Fcitx5 / IBus. macOS and Windows are unchanged and stay on v3.6.9.

### Linux

#### Candidates

- **The selection keys are back beside each candidate on Fcitx5.** The window
  showed no `q w d f z x v y ;` labels, so letter selection looked broken even
  though the keys still picked the right word. (#161)
- **The arrow keys follow a vertical candidate window.** With 候選窗排列 set to
  直, `↑` / `↓` now move one candidate and `←` / `→` turn the page, as on macOS
  and Windows; the horizontal window is unchanged. On both Fcitx5 and IBus.
  (#164)

#### Settings

- **字型管理 is no longer in the Linux settings.** Picking a typeface there never
  changed the candidate window, which Fcitx5 / IBus draw with their own font:
  set it in Fcitx5 › 附加元件 › 經典使用者介面 › 字體, or in IBus's preferences
  outside GNOME. The four bundled typefaces (粉圓, 芫荽, 源樣明體, 源樣烏體) stay
  installed as system fonts, so they are in those pickers and rare characters
  such as `𧉟` still display. (#167)
