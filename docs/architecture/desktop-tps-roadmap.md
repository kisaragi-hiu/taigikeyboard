# Desktop TPS mode — roadmap

TPS (方音符號, the i18n `en` label "TPS") as a third input mode on macOS, Windows and Linux, typed on a physical keyboard, with an on-screen key panel. iOS and Android have typed TPS since v3.5.x.

Status: P0 on main, P1 merged #367. No release is assigned; scope and timing are the maintainer's call.

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

O1 (what Space does once the syllable is closed) — decided 2026-10-03, the recommended arm: see D3.

## Today (grounded in code)

| Fact | Where |
|---|---|
| Desktop `InputMode` is `{Tl, Poj}`; the comment excludes TPS on purpose | `desktop/crates/taigi-desktop-core/src/settings/engine_settings.rs:8-43`; `macos/Sources/TaigiInputMethodCore/Settings/EngineSettings.swift:3-9` |
| A stored `"inputMode": "tps"` reads as TL; the stored string survives. The macOS FFI importer passes the stored text through unfiltered | `settings/document.rs:102-108`, test `:376-386`; `macos/crates/taigi-macos-ffi/src/settings.rs:52-53,76-93` |
| The engine accepts `input_mode = "tps"`, forces Hanji-first, never renders hyphenless | `engine/protos/proto/envelope.proto:134-135`; `engine/protos/src/lib.rs:44-62` |
| The raw buffer under TPS holds TPS glyphs; the preedit comes back as glyphs with the space markers removed | `engine/composing/src/derived.rs:18-23,41` |
| Per key, mobile calls the phonetics op `TpsInputAdjust{incoming, raw_input}` → optional `ReplaceLast` → `Append` — two composing mutations and a phonetics round trip | `engine/protos/proto/phonetics.proto:109-112,170-173`; `engine/phonetics/src/tps_adjust.rs:340-350`; iOS `Actions/ActionHandler+KeyActions.swift:25-42`; Android `ime/text/keyboard/TextInputKeyHandler.kt:503-518` |
| `ReplaceLast` edits the character before the engine's caret; the desktop mirrors `raw_input` and `display_text` but not the raw caret, and the response carries only a display UTF-16 caret | `engine/composing/src/transition.rs:334-349`; `composing/manager.rs:28-46`; `engine/protos/proto/composing.proto:294-302` |
| The engine already has one intent that edits before its own caret in a single transition: `TelexKey` | `engine/composing/src/transition.rs:127-140` |
| Mobile Space: appended as a soft separator when the last raw character is neither a tone mark nor a space; otherwise the raw buffer is committed and a document space follows (§31, §41) | iOS `ActionHandler+KeyActions.swift:168-190`; Android `TextInputKeyHandler.kt:435-455` |
| The engine's tone-mark set is eight scalars and public | `engine/phonetics/src/tps.rs:233-245` |
| The desktop classifier types only ASCII letters and `-` (plus tone digits) | `keys/intent.rs:279-281,391-393` |
| Slot keys are bare letters (Standard) or digits (Telex), taken before any append; composing bindings (Space among them) are tier 4 | `keys/intent.rs:192-220`; `keys/slot_key_set.rs:36,59`; `keys/action.rs:100` |
| Space is bound to Output the Other Script; under TPS `COMMIT_SCRIPT_OTHER` writes raw TL | `keys/action.rs:100`; `engine/composing/src/commit_text.rs:138-144` |
| A candidate row's `roman` is TL; mobile shows Hanji only, or `tlDisplayToTPS(roman)` for a Hanji-less row | iOS `Autocomplete/Views/CandidateCellHelper.swift:26-38`; Android `ime/text/candidates/CandidateStripState.kt:89-92` |
| `raw_preedit_writes_romanization` matches `Tl \| Poj` exhaustively, so a new variant fails to compile there; its comment names TPS | `policies/auto_space.rs:39-47` |
| No physical-key → TPS table exists in the repository | — (only reference: `references/rime-moetaigi/rime-moetaigi/moetaigi-tsuim.schema.yaml:70,87`, Dachen positions) |
| On Windows and Linux a shifted key's `characters_ignoring_modifiers` keeps Shift (`!`, `^`, `<`), and Caps Lock uppercases letters | `windows/crates/taigi-windows-platform/src/key_translation.rs:175-188`; `linux/crates/taigi-linux-platform/src/key_translation.rs:97-105` |
| A keypad digit and a number-row digit differ only by `key_code`: Windows fills it for every key; Linux and macOS map only the number row and `;`. The macOS shell proto already carries `key_code` | `windows/crates/taigi-windows-platform/src/key_translation.rs:192-193`; `linux/crates/taigi-linux-platform/src/key_translation.rs:195-202`; `macos/crates/taigi-macos-ffi/src/key_translation.rs:98-103`; `macos/crates/taigi-macos-ffi/proto/desktop_shell.proto:225` |
| The classifier takes no input mode; it reads `ComposingKeyBindings`, built from the settings document | `keys/intent.rs:144-150`; `keys/bindings.rs:83-94` |
| `app_config` never sets `tps_or_maps_to_er` | `engine/bridge.rs:115-123` |
| Windows and Linux settings pickers iterate `InputMode::ALL`; macOS hard-codes two tags | `windows/crates/taigi-windows-settings/src/winui/pages/general.rs:40-47`; `linux/crates/taigikeyboard-settings/src/pages/general.rs:19-25`; `macos/.../Settings/GeneralSettingsView.swift:92-95` |
| `tpsMode` (方音符號) exists in i18n, scoped to `android`, `ios` | `i18n/settings.json:44-48` |
| Wildcard matches that would read TPS as TL | `keys/telex_guide_rows.rs:83-86`; `windows/crates/taigi-windows-tsf/src/session.rs:861-864,882-885`; `linux/crates/taigi-linux-core/src/chrome.rs:82-85,99-102,161-164` |
| The input mode is written from five places | Windows `taigi-windows-tsf/src/session.rs:859-865`, `taigi-windows-settings/src/winui/window.rs:578-579`; Linux `chrome.rs:161-164`; macOS `Controller/TaigiInputController.swift:580-582,627-628`, `Settings/GeneralSettingsView.swift:33-34,92` |
| Global shortcuts in use: `Ctrl+Alt` (`⌃⌘` on macOS) + `C` `H` `,` `/` `S`, and a bare `` ` `` | `keys/shortcut_actions.rs:157-170`; `macos/.../Settings/ShortcutActions.swift:31-115` |
| A click on any desktop candidate window selects, never commits | Windows `ui/candidate_window.rs:1411-1419`; Linux `session.rs:456-478`; macOS `Candidates/CandidateItemView.swift:53-56` |
| A Windows window procedure never requests an edit session (W3) | `windows/crates/taigi-windows-tsf/src/ui/window.rs:9-10`; `windows-roadmap.md:132-137` |
| The Linux IME owns no window; everything is a framework lookup table | `linux/crates/taigi-linux-core/src/chrome.rs:1-8`; `linux-roadmap.md:253-256,505-506` |

## Design

### D0 — One engine intent: `TpsKey`

A new composing intent `TpsKey { key }`, the TPS counterpart of `TelexKey`. In one transition the engine runs `tps_adjust` against the pending text before its own caret, applies the replacement the adjuster asks for, and inserts the glyph. `key = " "` is the separator: inserted when the character before the caret is neither a tone mark nor a space; otherwise the transition is a no-op that answers with no effects, which is how the caller knows the Space was not taken. With the caret inside the tail, a tone mark or separator after it refuses the Space too, so a syllable is never parted from its own tone mark.

Why in the engine rather than three wrappers in the desktop core: the desktop has a movable composing caret and only the engine knows where it sits in the raw buffer; one mutation renders one preedit instead of two; and the rule is then written once for five platforms. Mobile keeps its three-call path; moving it onto `TpsKey` is outside this plan.

### D1 — `InputMode::Tps`

One new variant, wire and stored spelling `"tps"`, label `SettingsTpsMode` (`tpsMode` gains the three desktop platforms). The five exhaustive matches (`settings/engine_settings.rs:27-30,38-41`, `policies/auto_space.rs:45-47`, `engine/dictionary_search.rs:100-103`, `engine/lexicon.rs:286-289`) are compile-forced decisions; the wildcard matches above become exhaustive in the same PR. A document that already stores `"tps"` (a restored mobile backup) starts reading as TPS on all three desktops — on macOS too, since its importer does not filter — so the test at `document.rs:376-386` changes its unknown value to one that stays unknown, and the variant lands together with the classifier branch, never before it.

### D2 — Physical layout (U4)

A table in the core, new file `keys/tps_layout.rs`, keyed on the character the key types on a US layout — the same assumption the slot keys make. Letters are read lowercased with the Shift modifier deciding the layer, so Caps Lock does not select it; digits and punctuation are read as typed (`!` `*` `(` `<` `>` `:` `^`), since Windows and Linux hand over the shifted character.

Base rows — the Dachen positions as `rime-moetaigi` adapts them (`moetaigi-tsuim.schema.yaml:87`):

| Key | Glyph | Shift | | Key | Glyph | Shift |
|---|---|---|---|---|---|---|
| `1` | ㄅ | ㆠ | | `8` | ㄚ | ㆩ |
| `q` | ㄆ | | | `i` | ㆦ | ㆧ |
| `a` | ㄇ | | | `k` | ㄛ | ㄜ ‡ |
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

Stop codas `b` ㆴ, `t` ㆵ, `g` ㆻ, `z` ㆷ. Tones `4` ˋ (2), `3` ˪ (3), `6` ˊ (5), `=` ˇ (6) †, `5` ˫ (7), `7` ˙ (8, typed as U+02D9 as on mobile; the engine folds it for lookup, `engine/phonetics/src/tps.rs:262`), Shift+`6` ˆ (9) †; tone 1 and the unmarked tone 4 are Space. `'` types the hyphen † — `-` is taken by ㆭ, and the hyphen is how 輕聲 `--` is written (`engine/phonetics/src/tps.rs:132-134`).

A Shift cell left blank types nothing and is consumed while composing; idle, it passes through.

† = not in `rime-moetaigi`. Mobile types ㄝ ㆨ ˇ from its grid and tone 9 as the digit `9`, which the adjuster turns into ˆ (`ios/Sources/TaigiKeyboard/Layout/TaigiLayouts.swift:83-98`; `engine/phonetics/src/tps_adjust.rs:268-302`); here ˆ has its own key. ㄝ is `ee`, a phoneme of its own, not a spelling of ㆤ `e` (`engine/phonetics/src/tps_ambiguity.rs:23-27`).

‡ = differs from `rime-moetaigi`, which spells TL `o` as ㄜ on `k`. This engine spells `o` as ㄛ and keeps ㄜ for `er` / `or` (`engine/phonetics/src/tps.rs:53-56,67`); a typed ㄛ finds both, a typed ㄜ finds only `er` / `or` words, so ㄛ takes the unshifted key.

ㆳ (U+31B3) gets no key: the engine has no vowel row for it and no dictionary entry holds it.

The table was checked glyph by glyph against `engine/phonetics/src/tps.rs:17-130` and `taigi-converter/src/tables.js:75-98` (2026-10-03): every glyph the engine spells has a key, and every key types a glyph the engine accepts. The palatal keys `r` `f` `v` are optional — ㄗ ㄘ ㄙ ㆡ before ㄧ or ㆪ are rewritten by `tps_adjust.rs:319-332`. The explicit coda keys do not force a final reading: §35 lookup reads every member of a key's family in both directions, and a user who never learns `b t g z m p / -` types the same words through the fold (§32, §33). Only a tone mark or Space pins the boundary.

### D3 — Classifier under TPS

One branch at the top of `ComposingKeyIntent::intent`, taken when the bindings say the mode is TPS (`ComposingKeyBindings::from_document` carries it — the classifier's signature and its four callers do not change), ahead of the slot tier, the composing bindings and the typing tier, so a bound Space (or a chord rebound onto a layout key) cannot intercept:

| Key | Idle | Composing |
|---|---|---|
| A layout key | `TpsKey(glyph)` — begins the composition | `TpsKey(glyph)` |
| Space | pass through | `TpsKey(" ")`; when it answers with no effects the syllable was already closed → O1 |
| Keypad `1`–`9`, no modifier, list showing | — | pick that slot |
| Keypad digit otherwise | pass through | `CommitThenInsert` |
| Enter / Shift+Enter / Tab / arrows / `[` `]` / Esc / Backspace / caret chord | unchanged | unchanged |
| Ctrl + `,` `.` `;` | types the full-width mark — the bare key is a glyph, so under TPS the chord is the punctuation key, not a width flip | commits, then types the full-width mark |
| Any other printable | pass through | `CommitThenInsert` |

Classifying a key never asks the engine — the Windows Test phase returns before the runtime is prepared (`windows/crates/taigi-windows-tsf/src/session.rs:205-242`) — so "Space is consumed while composing" is the whole test-phase answer.

**O1 — Space on a closed syllable.** Mobile commits the glyphs as typed and writes a document space; nobody types TPS that way on mobile, where a candidate is tapped. On a desktop, Space is the key every Zhuyin-family input method confirms with. Recommended: with a list showing, Space confirms the highlighted candidate (Enter's action); with none, it commits the glyphs as typed, no space after. The alternative was mobile's rule verbatim. **Decided 2026-10-03 (maintainer: "follow your recommendation"): the recommended arm** — one arm in the executor, built in P2b.

The keypad is read from `key_code`, which Windows already fills; the Linux and macOS key-code tables gain the nine keypad codes. No snapshot field and no proto change. With Num Lock off the keypad arrives as navigation keys and behaves as those do today.

Under TPS these do nothing, and write no setting: the Toggle Hanji / Romanization and Cycle Candidate Display shortcuts (`windows/crates/taigi-windows-tsf/src/session.rs:888-925` and the Linux `chrome.rs` counterparts), the Telex guide shortcut, Telex keys, the letter and digit slot sets, the Shift + slot script flip, Output the Other Script (its row in the Shortcuts pane stays; its chord never fires under TPS), No Hyphens. Full-width punctuation is on regardless of the stored swap, as on mobile (`ios/.../Settings/SharedSettings.swift:737-741`).

### D4 — What the list and the commit show

One cell per candidate: Hanji when the row has one; otherwise the row's `roman` through the phonetics op `TlDisplayToTps`. Candidate Display has no effect, as in the engine's commit (`commit_text.rs:83-91`). Commits use `CommitScript::Lead`; the raw commit writes the glyphs as typed.

Sites in `desktop/crates/taigi-desktop-core/src/` that assume a romanization and get a TPS arm:

| Site | Under TPS |
|---|---|
| `composing/cell_content.rs:55-66` | Hanji-less cell shows glyphs; Romanization Only does not hide Hanji; no roman annotation |
| `composing/presentation.rs:40-74` | One cell per candidate — no Alternate cell under Combined |
| `composing/presentation.rs:96-103` | A Hanji-less first cell is a candidate, not the typed-literal lead |
| `composing/intent_executor.rs:149-159` | No `Other` commit, no Shift-slot flip |
| `settings/document.rs:267-278` | Full-width punctuation effective value is on |
| `keys/bindings.rs:96-99` | Slot labels are `1`–`9` |
| `policies/auto_space.rs:39-47` | `raw_preedit_writes_romanization(Tps) = false` |
| `engine/bridge.rs:115-123` | `tps_or_maps_to_er = true`, the mobile default (`ios/.../Settings/SharedSettings.swift:88`); the cell's `TlDisplayToTps` call passes the same value, so a cell and its commit agree |

### D5 — Shortcuts and mode changes (U2)

| Action | Windows / Linux | macOS | Does |
|---|---|---|---|
| `ToggleTps` (new) — Switch TPS | `Ctrl+Alt+P` | `⌃⌘P` | TPS ↔ the romanization last used |
| `ShowTpsKeyboard` (new) — Show TPS Keyboard | `Ctrl+Alt+K` | `⌃⌘K` | Shows / hides the panel (D6) |
| `ToggleRomanization` (existing) | `Ctrl+Alt+C` | `⌃⌘C` | Unchanged between TL and POJ. Under TPS it leaves TPS for the *other* romanization than the one last used — it never enters TPS |

`Ctrl+Alt+T` is avoided (GNOME's terminal). Both chords pass `global_rejection` (`keys/shortcut_actions.rs:245-279`) and collide with nothing in the default roster; they are rebindable and go through the existing conflict rules. No list of system shortcuts reserves either, which is not a promise for every desktop environment — the macOS defaults are confirmed on device in P4.

One function in the core, `settings::next_input_mode(current, last_romanization, request)`, answers every mode change — the picker, both shortcuts, reset, restore — and every writer in the five places listed above calls it. The last-used romanization is one new stored key, `lastRomanizationMode`, written only when a romanization is left for TPS; when absent, the mode in use counts, so a POJ user's first round trip returns to POJ. TPS is never stored in it. Both new stored keys join the General reset list (`settings/keys.rs:245-254`); `ShowTpsKeyboard` joins `fires_once_per_press` (`keys/shortcut_actions.rs:97-99`).

### D6 — On-screen key panel (U3, U6)

Shared in the core, new file `keys/tps_keyboard_rows.rs`: four rows of key caps built from the D2 table — the physical key's label, its glyph, its Shift glyph — so the panel cannot drift from the layout.

| Platform | Window | Shown | Click |
|---|---|---|---|
| macOS | A new non-activating `NSPanel`, mouse events on (the HUD panels ignore the mouse, `Panels/HUDPanel.swift:37`) | Fixed screen placement — activation may not query the caret (`TaigiInputController.swift:235-240`) | A new owned session request beside `Key` (`macos/crates/taigi-macos-ffi/src/session.rs:68-103`) that runs the same `TpsKey` path — not `InsertSymbol`, which writes external text. Validates TPS mode, the glyph against the D2 table, owner token, generation, live client (`macos-roadmap.md:319`) |
| Windows | A new `WindowHandler` on `PopupWindow` (`ui/window.rs:174-235`) | Centred like the Telex guide (`ui/telex_guide.rs:217-232`); stays up across keys; hides on focus loss | `SendInput` of the key's real virtual key (down + up), which re-enters through the key sink — W3 holds. **Unverified until its spike passes**; if it does not, Windows ships show-only and this row is amended |
| Linux | A window of the GTK settings app (`taigikeyboard-settings`), opened by the shortcut and the panel menu | An ordinary window | None — clicking it takes focus, which ends the session (`taigikeyboard-ibus/src/engine.rs:302-326`) |

Windows spike acceptance, on the Windows box, before P6 is scoped: a base key and a Shift-layer key each type their glyph; a click with a physical Shift / Ctrl / Alt / Win held; the synthetic Shift does not trip the Shift-tap English toggle (`taigi-windows-platform/src/key_translation.rs:68-93`); the cold first key (Test, then Deliver with `prepare_for_first_key`); focus moving between click and delivery; an English-mode and a read-only context; a click while the symbol picker or the Telex guide is up; a non-US keyboard layout (the injected key's character depends on the live layout); `SendInput`'s return count, and an elevated host (UIPI refuses injection upward).

The panel is shown only under TPS and persists until the shortcut hides it; whether it is shown is one stored key, `tpsKeyboardShown`.

## Phases

| Phase | Type | Scope | Builds / tests | Size | Status |
|---|---|---|---|---|---|
| P0 | docs | This roadmap, the `roadmap.md` row | — | — | In progress |
| P1 | feat (engine) | D0: `TpsKey` in `composing.proto` and `transition.rs`, tests from the mobile key sequences in `behavioral-invariants.md` §31–§33, §41 | engine; `make build` for the mobile artifacts (additive — mobile sends nothing new) | ~250 | Merged #367 `a5174ed9` |
| P2a | feat (desktop-core, not reachable) | D2 table, the `TpsKey` bridge call, the D4 sites behind a mode the document cannot yet produce, keypad key codes on Linux and macOS; tests only | desktop-core, Windows, Linux, the macOS Rust seam, `make -C macos test` | ~400 | Pending |
| P2b | feat (desktop-core, Windows, Linux) | D1 + D3: the variant, the classifier branch, the exhaustive and wildcard matches with their mode labels, i18n scope. Windows and Linux type TPS from the settings picker | as P2a | ~400 | Pending (O1 decided 2026-10-03) |
| P3 | feat (desktop-core, Windows, Linux) | D5: `next_input_mode`, `ToggleTps`, `lastRomanizationMode`, the Windows preserved key, the menu row | as P2a | ~300 | Pending |
| P4 | feat (macOS) | The Swift enum and picker, `ToggleTps` in `ShortcutActions.swift`, the mode flash | macOS | ~350 | Pending |
| P5 | feat (all three) | D6 show-only: the rows in the core, the three windows, `ShowTpsKeyboard`, `tpsKeyboardShown`; the Windows `SendInput` spike | all three | ~500 | Pending |
| P6 | feat (macOS, Windows) | D6 click: the macOS session request; the Windows `SendInput` path if its spike passed | macOS, Windows | ~350 | Pending |

Regression surface for TL and POJ, checked in every phase that touches the core: the classifier callers (Windows `session.rs:194`, Linux `session.rs:283`, macOS FFI `session.rs:93`), the executor callers (Windows `session.rs:1394`, Linux `session.rs:512`, macOS FFI `session.rs:101`), slot labels (Windows `session.rs:1225-1238`, Linux `session.rs:544-582`), cell presentation, the literal-lead skip, the punctuation projection, and the mode writers.

### Not disturbing the refactor (U1)

P15 of `macos-desktop-core-roadmap.md` rewrites the `*.swift` cites in Rust comments; the files it edits most are the ones P2a and P2b edit (`keys/chord.rs`, `keys/intent.rs`, `settings/keys.rs`, `settings/engine_settings.rs`). So:

- P0 and P1 (engine) touch nothing P15 concentrates on and can proceed now.
- P2a onward — every edit to an existing desktop-core, Windows, Linux or macOS file, module registration of the new files included — starts after P15 merges, on a rebase.
- The macOS behaviour-freeze contract is unaffected for TL and POJ: every property it lists is unchanged unless the stored mode is `"tps"`.

## Best practices alignment

| Mainstream practice | Source | This plan |
|---|---|---|
| Zhuyin-family layouts keep the Dachen positions and put the language's extra glyphs on Shift | `references/rime-moetaigi/rime-moetaigi/moetaigi-tsuim.schema.yaml:70,87` | D2 |
| Space collides with tone 1, so candidate picking moves off the main block | same file, `:254-268` (`Num_Lock: toggle_selection`) | D3, U5 |
| A closed composition is confirmed with Space | same file, `:266-268` (`when: composing, accept: space, send: Return`) | O1, recommended arm |
| Shared behaviour is written once in Rust; an edit relative to the caret belongs to the owner of the caret | `AGENTS.md` § Design principles; `engine/composing/src/transition.rs:127-140` (`TelexKey`) | D0 |
| Align on intended behaviour, not API calls | `docs/contributing/cross-platform-alignment.md` | D3 keeps the mobile separator rule; D6 differs per platform and says why |
| No redundant fallback | `AGENTS.md` § Design principles | The classifier has one TPS branch, not a romanization path that catches what TPS misses |
| A mouse path into the composition needs owner + generation validation | `macos-roadmap.md:319` | D6 macOS |
| Never request an edit session from a window procedure | `windows-roadmap.md:132-137` | D6 Windows uses the key sink |

Deliberately not adopted:

- **`TpsInputAdjust` + `ReplaceLast` + `Append` wrappers in the desktop core** — three calls, two preedits, and a raw caret the desktop does not have.
- **A second layout following the mobile 10 × 5 grid** — offered, not chosen (U4); one table, one panel.
- **Shift + digit as slot keys** — the Shift layer of the number row types glyphs.
- **A copy of the tone-mark set in the core** — the engine's separator test is the only one.
- **An IME-owned GTK window on Linux, or Fcitx5's virtual-keyboard interface** — `linux-roadmap.md:505-506`; a click path there is outside this plan (U6).
- **An asynchronous edit session from the Windows panel** — breaks W3.
- **A physical-position (scan code) layout table** — only Windows carries full key codes today; the character table needs nine keypad codes and nothing else.
- **A new `is_keypad` snapshot field** — `key_code` already says it, and a new field touches every snapshot literal and the macOS proto.
- **The Linux panel as a framework lookup table**, as the Telex guide is drawn — the table is shared with the candidates and is taken down by the next key, so it cannot stay up while typing.

## Reviews

- P0 pre-implementation, 2026-10-03: `phonetics-specialist` (ㄛ / ㄜ swapped onto the right layers; tone 8 scalar; ㆳ dropped); Codex GO-WITH-CHANGES — raw caret → D0, shifted characters and Caps Lock → D2, Space and keypad rules → D3, presentation sites → D4, one mode-change function and the POJ default → D5, spike acceptance → D6, the macOS seam in P2a's gates; all applied. Claude cloud session `session_01UteaCLYB9BKdVnyyhzwUEW` GO-WITH-CHANGES — the same caret, Shift-layer and Space findings independently, plus: keypad through `key_code`, the mode through the bindings, `tps_or_maps_to_er`, inert display shortcuts, reset list, P2 split; all applied. Not applied: a key for ㆳ (no engine reading), the Linux lookup-table panel (see above).

## Dogfood

One `Sn` per phase is added to `dogfood-checklist.md` when its PR opens, with sentences from `corpus/taigi-typing`.
