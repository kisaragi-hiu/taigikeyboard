# Desktop TPS mode — roadmap

TPS (方音符號, the i18n `en` label "TPS") as a third input mode on macOS, Windows and Linux, typed on a physical keyboard, with an on-screen key panel. iOS and Android have typed TPS since v3.5.x; the engine needs no change.

Status: **P0 (this document)**. No release is assigned; scope and timing are the maintainer's call.

## Maintainer decisions (2026-10-03)

| # | Decision |
|---|---|
| U1 | Do not disturb the macOS-over-desktop-core refactor (`macos-desktop-core-roadmap.md`); other sessions work in parallel; this work lives in its own worktree. |
| U2 | TPS is not a romanization. Its switch shortcut is its own action and does not join the TL ↔ POJ toggle. |
| U3 | An on-screen key panel the user can look at or click. |
| U4 | Physical layout = the system Zhuyin (Dachen) positions, TPS-only glyphs on a Shift layer. |
| U5 | Candidate picking under TPS = arrows / Tab + Enter, plus the numeric keypad `1`–`9`. |
| U6 | Panel scope: macOS and Windows show it and take clicks; Linux shows it only. |
| U7 | Review: Codex sandwich plus a Claude cloud session per PR. |

## Today (grounded in code)

| Fact | Where |
|---|---|
| Desktop `InputMode` is `{Tl, Poj}`; the comment excludes TPS on purpose | `desktop/crates/taigi-desktop-core/src/settings/engine_settings.rs:8-43`; `macos/Sources/TaigiInputMethodCore/Settings/EngineSettings.swift:3-9` |
| A stored `"inputMode": "tps"` reads as TL; the stored string survives | `settings/document.rs:102-108`, test `:376-386` |
| The engine accepts `input_mode = "tps"`, forces Hanji-first, never renders hyphenless | `engine/protos/proto/envelope.proto:134-135`; `engine/protos/src/lib.rs:44-62` |
| The raw buffer under TPS holds TPS glyphs; the preedit comes back as glyphs with the space markers removed | `engine/composing/src/derived.rs:18-23,41` |
| Per key, mobile calls the phonetics op `TpsInputAdjust{incoming, raw_input}` → optional `ReplaceLast` → `Append` | `engine/protos/proto/phonetics.proto:109-112,170-173`; `engine/phonetics/src/tps_adjust.rs:340-350`; iOS `Actions/ActionHandler+KeyActions.swift:25-42`; Android `ime/text/keyboard/TextInputKeyHandler.kt:503-518` |
| Space: appended as a soft separator when the last raw character is neither a tone mark nor a space; otherwise the raw buffer is committed (§31, §41) | iOS `ActionHandler+KeyActions.swift:168-188`; Android `TextInputKeyHandler.kt:435-457` |
| Desktop has no `ReplaceLast` wrapper | `desktop/crates/taigi-desktop-core/src/engine/composing.rs:6` |
| The desktop classifier types only ASCII letters and `-` (plus tone digits) | `keys/intent.rs:279-281,391-393` |
| Slot keys are bare letters (Standard) or digits (Telex), taken before any append | `keys/intent.rs:192-200`; `keys/slot_key_set.rs:36,59` |
| Space is bound to Output the Other Script; under TPS `COMMIT_SCRIPT_OTHER` writes raw TL | `keys/action.rs:100`; `engine/composing/src/commit_text.rs:138-144` |
| A candidate row's `roman` is TL; mobile shows Hanji only, or `tlDisplayToTPS(roman)` for a Hanji-less row | iOS `Autocomplete/Views/CandidateCellHelper.swift:26-38`; Android `ime/text/candidates/CandidateStripState.kt:89-92` |
| `raw_preedit_writes_romanization` is an exhaustive match that names TPS | `policies/auto_space.rs:39-47` |
| No physical-key → TPS table exists in the repository | — (only reference: `references/rime-moetaigi/rime-moetaigi/moetaigi-tsuim.schema.yaml:70,87`, Dachen positions) |
| `KeyEventSnapshot` cannot tell a keypad digit from a number-row digit | `keys/snapshot.rs:80-100`; `keys/slot_key_set.rs:140-141` |
| Windows and Linux settings pickers iterate `InputMode::ALL`; macOS hard-codes two tags | `windows/crates/taigi-windows-settings/src/winui/pages/general.rs:40-47`; `linux/crates/taigikeyboard-settings/src/pages/general.rs:19-25`; `macos/.../Settings/GeneralSettingsView.swift:92-95` |
| `tpsMode` (方音符號) exists in i18n, scoped to `android`, `ios` | `i18n/settings.json:44-48` |
| Wildcard matches that would read TPS as TL | `keys/telex_guide_rows.rs:83-86`; `windows/crates/taigi-windows-tsf/src/session.rs:861-864,882-885`; `linux/crates/taigi-linux-core/src/chrome.rs:82-85,99-102,161-164` |
| Global shortcuts in use: `Ctrl+Alt` (`⌃⌘` on macOS) + `C` `H` `,` `/` `S`, and a bare `` ` `` | `keys/shortcut_actions.rs:157-170`; `macos/.../Settings/ShortcutActions.swift:31-115` |
| A click on any desktop candidate window selects, never commits | Windows `ui/candidate_window.rs:1411-1419`; Linux `session.rs:456-478`; macOS `Candidates/CandidateItemView.swift:53-56` |
| A Windows window procedure never requests an edit session (W3) | `windows/crates/taigi-windows-tsf/src/ui/window.rs:9-10`; `windows-roadmap.md:132-137` |
| The Linux IME owns no window; everything is a framework lookup table | `linux/crates/taigi-linux-core/src/chrome.rs:1-8`; `linux-roadmap.md:253-256,505-506` |

## Design

### D1 — `InputMode::Tps`

One new variant, wire and stored spelling `"tps"`, label `SettingsTpsMode` (`tpsMode` gains the three desktop platforms). The four exhaustive matches are compile-forced decisions; the six wildcard matches above become exhaustive in the same PR. A document that already stores `"tps"` (a restored mobile backup) starts reading as TPS — the test at `document.rs:376-386` changes its unknown value to one that stays unknown.

### D2 — Physical layout (U4)

A character-keyed table in the core, new file `keys/tps_layout.rs`: `(unshifted character, Shift held) → glyph`, read from `characters_ignoring_modifiers` lowercased plus the Shift modifier, so Caps Lock does not select the Shift layer. US QWERTY characters; the same assumption the slot keys make today.

Base rows — the Dachen positions as `rime-moetaigi` adapts them (`moetaigi-tsuim.schema.yaml:87`):

| Key | Glyph | Shift | | Key | Glyph | Shift |
|---|---|---|---|---|---|---|
| `1` | ㄅ | ㆠ | | `8` | ㄚ | ㆩ |
| `q` | ㄆ | | | `i` | ㆦ | ㆧ |
| `a` | ㄇ | | | `k` | ㄜ | ㄛ † |
| `2` | ㄉ | | | `o` | ㆤ | ㆥ |
| `w` | ㄊ | | | `9` | ㄞ | ㆮ |
| `s` | ㄋ | | | `l` | ㄠ | ㆯ |
| `x` | ㄌ | | | `,` | ㆰ | ㆱ |
| `e` | ㄍ | ㆣ | | `0` | ㄢ | |
| `d` | ㄎ | ㄫ | | `;` | ㄤ | ㆲ |
| `c` | ㄏ | | | `u` | ㄧ | ㆪ |
| `r` | ㄐ | ㆢ | | `j` | ㄨ | ㆫ |
| `f` | ㄑ | | | `m` | ㆬ | |
| `v` | ㄒ | | | `p` | ㄣ | |
| `y` | ㄗ | ㆡ | | `/` | ㄥ | |
| `h` | ㄘ | | | `-` | ㆭ | |
| `n` | ㄙ | | | `.` | ㆨ † | ㄝ † |

Stop codas `b` ㆴ, `t` ㆵ, `g` ㆻ, `z` ㆷ. Tones `4` ˋ (2), `3` ˪ (3), `6` ˊ (5), `=` ˇ (6) †, `5` ˫ (7), `7` ˙ (8), Shift+`6` ˆ (9) †; tone 1 and the unmarked tone 4 are Space.

† = not in `rime-moetaigi`, which has no key for ㄛ ㄝ ㆨ ˇ ˆ; the mobile layout types all five (`ios/Sources/TaigiKeyboard/Layout/TaigiLayouts.swift:83-98`). P1 confirms these five positions with the `phonetics-specialist` agent against `docs/phonetics/taigi-phonetics-reference.md` before the table is written.

The explicit coda keys and the engine's fold are complementary: typing ㄅ after a vowel still folds to ㆴ where the syllable is valid (§32, §33), and §35 lookup reads every member of a key's family, so a user who never learns `b t g z m p / -` types the same words.

### D3 — Classifier under TPS

One new branch in `ComposingKeyIntent::intent`, taken when `input_mode == Tps`, in place of tiers 3 and 7:

| Key | Idle | Composing |
|---|---|---|
| A layout key | `TpsInput(glyph)` — begins the composition | `TpsInput(glyph)` |
| Space | pass through | separator when the last raw character is neither a tone mark nor a space; else `CommitLiteral` then no space |
| Keypad `1`–`9`, list showing | — | pick that slot |
| Number-row digit, letter | layout key | layout key — never a slot |
| Enter / Shift+Enter / Tab / arrows / `[` `]` / Esc / Backspace | unchanged | unchanged |
| Ctrl + `,` `.` `;` | types the full-width mark | commits, then types the full-width mark |
| Any other printable | pass through | `CommitThenInsert` |

`TpsInput` executes as `TpsInputAdjust(glyph, raw[..caret])` → `ReplaceLast` if the answer carries one → `Append`, all in `composing/manager.rs`, so the three platforms share it. The tone-mark test for Space reads the core's own copy of the seven marks, asserted equal to the engine's `IsTpsToneMark` in a test — no round trip per Space.

The keypad needs one new snapshot field, `is_keypad: bool`, filled by each platform's key translation (Windows `VK_NUMPAD1…9`, Linux `KP_1…9`, macOS the numeric-pad flag) and carried in `desktop_shell.proto` `KeyEvent`.

Under TPS these do nothing or do not apply: Telex keys, the letter and digit slot sets, Output the Other Script (Space is the separator; the action's row in the Shortcuts pane stays, its chord simply never fires under TPS), Toggle Hanji / Romanization (the engine forces Hanji-first), No Hyphens. Full-width punctuation is on, as on mobile (`ios/.../Settings/SharedSettings.swift:737-741`).

### D4 — What the list and the commit show

Candidate cell: Hanji when the row has one; otherwise the row's `roman` through the phonetics op `TlDisplayToTps`. Candidate Display has no effect under TPS, as in the engine's commit (`commit_text.rs:83-91`). Commits use `CommitScript::Lead`; the raw commit writes the glyphs as typed. `raw_preedit_writes_romanization(Tps) = false`.

### D5 — Shortcuts (U2)

| Action | Windows / Linux | macOS | Does |
|---|---|---|---|
| `ToggleTps` (new) — Switch TPS | `Ctrl+Alt+P` | `⌃⌘P` | TPS ↔ the romanization last used (`Tl` on a document that never chose) |
| `ShowTpsKeyboard` (new) — Show TPS Keyboard | `Ctrl+Alt+K` | `⌃⌘K` | Shows / hides the panel (D6) |
| `ToggleRomanization` (existing) | `Ctrl+Alt+C` | `⌃⌘C` | Unchanged between TL and POJ. Under TPS it leaves TPS for the *other* romanization than the one last used — it never enters TPS |

`Ctrl+Alt+T` is avoided (GNOME's terminal). Both new actions are rebindable and pass through the existing conflict rules. The last-used romanization is one new stored key, `lastRomanizationMode`.

### D6 — On-screen key panel (U3, U6)

Shared in the core, new file `keys/tps_keyboard_rows.rs`: four rows of key caps built from the D2 table — the physical key's label, its glyph, its Shift glyph — so the panel cannot drift from the layout.

| Platform | Window | Shown | Click |
|---|---|---|---|
| macOS | A new non-activating `NSPanel`, mouse events on (the HUD panels ignore the mouse, `Panels/HUDPanel.swift:37`) | Fixed screen placement — activation may not query the caret (`TaigiInputController.swift:235-240`) | A new core session request carrying the glyph, validated by owner token + generation + live client (`macos-roadmap.md:319`) |
| Windows | A new `WindowHandler` on `PopupWindow` (`ui/window.rs:174-235`) | Centred like the Telex guide (`ui/telex_guide.rs:217-232`); stays up across keys; hides on focus loss | `SendInput` of the key's real virtual key, which re-enters through the key sink — W3 holds. **Spike first** (≤ 20 lines, on the Windows box); if a host refuses it, Windows ships show-only and this row is amended |
| Linux | A window of the GTK settings app (`taigikeyboard-settings`), opened by the shortcut and the panel menu | An ordinary window | None — clicking it takes focus, which ends the session (`taigikeyboard-ibus/src/engine.rs:302-326`) |

The panel is shown only under TPS, and persists until the shortcut hides it; whether it is shown is one stored key, `tpsKeyboardShown`.

## Phases

| Phase | Type | Scope | Size | Status |
|---|---|---|---|---|
| P0 | docs | This roadmap, the `roadmap.md` row | — | In progress |
| P1 | feat (desktop-core, Windows, Linux) | D1, D2, D3, D4: the variant, the layout table, the classifier branch, `ReplaceLast` + `TpsInputAdjust` + `TlDisplayToTps` wrappers, `is_keypad` on Windows and Linux, the wildcard matches, i18n scope. Windows and Linux type TPS from the settings picker | ~500 | Pending |
| P2 | feat (desktop-core, Windows, Linux) | D5: `ToggleTps`, `lastRomanizationMode`, the Windows preserved key, menu and mode-flash labels | ~300 | Pending |
| P3 | feat (macOS) | The Swift enum and picker, `is_keypad` through `desktop_shell.proto`, `ToggleTps` in `ShortcutActions.swift`, the mode flash | ~350 | Pending |
| P4 | feat (all three) | D6 show-only: the rows in the core, the three windows, `ShowTpsKeyboard`, `tpsKeyboardShown` | ~500 | Pending |
| P5 | feat (macOS, Windows) | D6 click: the macOS session request; the Windows `SendInput` path after its spike | ~350 | Pending |

Order: P1 before everything; P4's Windows spike result is known before P5 is scoped.

### Not disturbing the refactor (U1)

P15 of `macos-desktop-core-roadmap.md` rewrites the `*.swift` cites in Rust comments; the files it edits most are the ones P1 edits (`keys/chord.rs`, `keys/intent.rs`, `settings/keys.rs`, `settings/engine_settings.rs`). So:

- P0 and the two new files (`keys/tps_layout.rs`, `keys/tps_keyboard_rows.rs`) touch nothing P15 touches.
- Edits to existing desktop-core files start after P15 merges, on a rebase.
- The macOS behaviour-freeze contract is unaffected for TL and POJ: every property it lists is unchanged unless the stored mode is `"tps"`, which no macOS build could select before P3.

## Best practices alignment

| Mainstream practice | Source | This plan |
|---|---|---|
| Zhuyin-family layouts keep the Dachen positions and put the language's extra glyphs on Shift | `references/rime-moetaigi/rime-moetaigi/moetaigi-tsuim.schema.yaml:70,87` | D2 |
| Space collides with tone 1, so candidate picking moves off the main block | same file, `:254-268` (`Num_Lock: toggle_selection`) | D3, U5 |
| Shared behaviour is written once in Rust | `AGENTS.md` § Design principles; `docs/contributing/rust-migration-policy.md` | D2–D4, D6 rows in the core |
| Align on intended behaviour, not API calls | `docs/contributing/cross-platform-alignment.md` | D3 mirrors the mobile key rules; D6 differs per platform and says why |
| No redundant fallback | `AGENTS.md` § Design principles | The classifier has one TPS branch, not a romanization path that catches what TPS misses |
| A mouse path into the composition needs owner + generation validation | `macos-roadmap.md:319` | D6 macOS |
| Never request an edit session from a window procedure | `windows-roadmap.md:132-137` | D6 Windows uses the key sink |

Deliberately not adopted:

- **A second layout following the mobile 10 × 5 grid** — offered, not chosen (U4); one table, one panel.
- **Shift + digit as slot keys** — the Shift layer of the number row types glyphs.
- **An IME-owned GTK window on Linux, or Fcitx5's virtual-keyboard interface** — `linux-roadmap.md:505-506`; a click path there is outside this plan (U6).
- **An asynchronous edit session from the Windows panel** — breaks W3.
- **A physical-position (scan code) layout table** — only Windows carries full key codes today; the character table needs no new plumbing beyond `is_keypad`.

## Dogfood

One `Sn` per phase is added to `dogfood-checklist.md` when its PR opens, with sentences from `corpus/taigi-typing`.
