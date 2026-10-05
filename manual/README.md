# Desktop user manual

A visual manual for first-time desktop users (macOS, Windows, Linux), a cheatsheet picture, and the blog post that announces them. Sources live here, like `launch-film/`; the website repository (`taigikeyboard/taigikeyboard.github.io`) receives copies.

| Path | What it is |
|---|---|
| `index.html`, `manual.css` | The manual page. Plain HTML, no build step, no script beyond picking the visitor's platform. |
| `shots/` | Screenshots of the real candidate window, panels and settings window. Generated — never edit by hand. |
| `cheatsheet.html` | Source of the cheatsheet picture; `?os=pc` switches it to Windows / Linux key names. |
| `cheatsheet/` | The rendered pictures (3200 × 2000). |
| `blog/` | Posts written for the website's future blog, with Jekyll front matter. |
| `render.sh` | Renders `cheatsheet.html` to `cheatsheet/*.png` with headless Chrome. |

## Preview

```sh
open manual/index.html
```

## Regenerating the pictures

The screenshots come from `macos/Tests/TaigiInputMethodCoreTests/ManualScreenshotTests.swift`, which types into the real controller and engine, lets the real windows come up, and captures the screen. Run it after a UI change, on a Mac whose terminal has Screen Recording permission (a Retina display gives 2x pictures):

```sh
make build                                   # only if engine/ or desktop/ changed
TAIGI_MANUAL_SHOTS="$PWD/manual/shots" swift test --package-path macos --filter ManualScreenshotTests
manual/render.sh                             # the cheatsheet embeds three of the screenshots
```

Windows flash on screen for about half a minute while it runs. A new picture is a new `Scene` in that file plus an `<img>` in `index.html`; update the `width` / `height` attributes when a scene's size changes.

## Publishing to the website

Jekyll copies a directory without front matter as-is, so the manual is served at `https://taigikeyboard.tw/manual/` by copying four things into the website repository's `manual/`:

```sh
SITE=../taigikeyboard.github.io
mkdir -p "$SITE/manual"
cp -R manual/index.html manual/manual.css manual/assets manual/shots manual/cheatsheet "$SITE/manual/"
```

Then link the page from the website's `_includes/navbar.html`.

The website has no blog yet. When it gets one, the files in `blog/` go into its `_posts/` unchanged; they link to `/manual/` and to pictures under `/manual/shots/`.

## Writing rules

- Text is Taigi in Hanji, matching the website. UI labels are quoted from the `hanji` values in `i18n/*.json` — copy them, do not reword.
- Every key is written twice, once per platform family: `<span class="mac">` and `<span class="pc">`. Defaults come from `desktop/crates/taigi-desktop-core/src/keys/` (`action.rs`, `shortcut_actions.rs`, `shortcut_labels.rs`).
