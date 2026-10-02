# macOS ↔ desktop-core Duplication Inventory

> **Type**: Report (frozen snapshot on `e330cd30`, 2026-10-02)
> **Keywords**: `macOS`, `desktop-core`, `duplication`, `inventory`, `parity`, `FFI`
> **Status**: Historical — the plan built on it is `docs/architecture/macos-desktop-core-roadmap.md`
> **Method**: five read-only passes (keys, composing, candidates, settings / storage / bridge, update), each comparing the Swift file with its Rust counterpart branch by branch. Nothing was built or run. Rows marked ✔ were re-read by hand afterwards; "not verified" means reasoned from the source only.

Replaces the "~4,000 Swift lines" estimate in `docs/reports/2026-09-30-audit-all.md` §2 / §5.2 with measured numbers.

Path prefixes: `M/` = `macos/Sources/TaigiInputMethodCore/` · `T/` = `macos/Tests/TaigiInputMethodCoreTests/` · `C/` = `desktop/crates/taigi-desktop-core/src/` · `ST/` = `desktop/crates/taigi-desktop-storage/src/` · `DU/` = `desktop/crates/taigi-desktop-update/src/` · `L/` = `linux/crates/` · `WIN/` = `windows/crates/`.

---

## 1. Measured totals

`wc -l` on the tree: `macos/Sources` holds 32,336 lines in 121 Swift files, of which 11,947 are generated (`Engine/Generated/*.pb.swift` 10,776, `Strings/Generated/*` 1,171). **Hand-written Swift: 20,389 lines.** Tests: 16,355 lines in 74 files. `desktop/crates`: 21,832 lines.

"Duplicate" below = lines whose logic has a counterpart in `desktop/crates` (estimate from the line ranges read, comments included). "Stays Swift" = IMKit / AppKit / SwiftUI / Security.framework / OS integration.

| Group | Swift total | Duplicate | Stays Swift | Clean to move? |
|---|---|---|---|---|
| Keys + shortcuts | 3,131 | ~1,228 | ~1,903 | Classifier, bindings, chords, slot keys, tone scheme: yes, after the core takes a platform parameter (§3 K1–K3). Global-shortcut tier: different design on macOS |
| Composing, policies, next-word, controller | 3,102 | ~1,580 (~1,807 with the per-key session preamble) | ~1,295–1,522 | Yes — the intent switch is a faithful port |
| Engine bridge (hand-written `RustEngineBridge*`) | 1,336 | ~1,246 | ~90 | 521 follow the composing group; the rest follow the settings UI |
| Settings model, strings, symbols, lexicon services, storage, bootstrap | 6,403 | ~2,332 | ~4,071 | Engine-settings snapshot and source toggles: yes. Storage backend: a persisted-format change. Page models: stateful, UI-bound |
| Candidate window | 5,397 | ~1,328 net of rows counted above | ~3,810 | Labels, grid, horizontal paging: yes. Metrics: three pixel divergences. Vertical / expandable models: tied to `NSScrollView` |
| Update check / verify / install | 1,731 | ~132 | ~1,599 | No — three blockers (§7) |
| **All groups** | **20,389** (groups overlap by ~700 lines) | **~8,070** | | |

About 790 of the duplicate lines are already dead on macOS (§5 S21) and can be deleted with no Rust work.

---

## 2. Composing, policies, next-word, controller

| Swift file (LOC) | Duplicate / stays | Rust counterpart | Status |
|---|---|---|---|
| `M/Composing/ComposingManager.swift` (447) | 447 / 0 | `C/composing/manager.rs:27-378` | Identical (C2 not verified) |
| `M/Composing/ComposingSessionCoordinator.swift` (176) | ~80 / ~96 | `C/composing/coordinator.rs:49-117`, `C/runtime.rs:149-160` | Divergent: the shortcut-target registry (`:18-28,58-73,116-148`) has no Rust counterpart and stays |
| `M/Composing/CandidateOutcomes.swift` (60) | 60 / 0 | `C/composing/outcomes.rs:12-83` | Identical |
| `M/Composing/ClientEffectExecutor.swift` (92) | ~14 / ~78 | trait `C/composing/manager.rs:23-25`; shell example `L/taigi-linux-core/src/executor.rs:106-128` | Stays: `setMarkedText` / `insertText` |
| `M/Composing/ComposingEffectExecutor.swift` (14) | 14 / 0 | `C/composing/manager.rs:23-25` | Identical |
| `M/Engine/ComposingTransition.swift` (121) | 121 / 0 | `C/engine/transition.rs:16-209` | Identical |
| `M/NextWord/NextWordPort.swift` (76) | 76 / 0 | `C/composing/next_word.rs:16-72`, `clock.rs:6-22` | Identical (Rust injects a `Clock`; Swift calls `Date()` at `:73-75`) |
| `M/Policies/AutoSpacePolicy.swift` (105) | 105 / 0 | `C/policies/auto_space.rs:31-107` | Identical |
| `M/Policies/AutoSpacePunctuation.swift` (32) | 32 / 0 | `C/policies/auto_space.rs:12-24` | Identical (19 characters, same order) |
| `M/Policies/FullWidthPunctuation.swift` (67) | 67 / 0 | `C/policies/full_width.rs:11-66` | Identical (24 of 24 entries) |
| `M/Controller/TaigiInputController.swift` (1,582) | 394 / 1,188 (621 / 961 with the session preamble) | `C/composing/intent_executor.rs:52-367` | Intent switch identical; see below |
| `M/Candidates/PresentedCandidate.swift` (75), `CandidateSource.swift` (68), `CandidateScript.swift` (27) | 170 / 0 | `C/composing/presentation.rs:18-182`, `cell_content.rs:13-26` | Identical |
| `M/Candidates/CandidatePresenter.swift` (160) | 0 / 160 | `C/composing/intent_executor.rs:36-47` (the surface it answers) | Stays: AppKit panel seam |

**Controller sections.**

- *Duplicate of desktop-core*: the intent switch `:787-923` ↔ `intent_executor.rs:60-172` (13 arms, one to one); `commitPresented` / `commit` `:943-1003` ↔ `:275-297`; `commitAsTyped` `:1005-1023` ↔ `:76-86`; `refreshCandidates` `:1025-1058` ↔ `:218-231`; refetch `:1092-1110` and re-render `:663-677` ↔ `:239-254`; `insertSymbol` `:1299-1318` ↔ `:179-189`; width / gate / auto space `:1342-1395` ↔ `:325-367`; the policy half of the swap ↔ `:304-319`.
- *Has a counterpart only in the Rust shells, not in desktop-core*: `performShortcutAction` `:589-661` ↔ `L/taigi-linux-core/src/chrome.rs:134-230`; the `handle` preamble `:703-786` ↔ `L/taigi-linux-core/src/session.rs:148-337,476-520`; `openSymbolPicker` `:1155-1179` ↔ `chrome.rs:292-329`; picker keys and picks `:1253-1297` ↔ `session.rs:216-271`, `chrome.rs:355-401`.
- *Stays (IMKit / AppKit)*: properties `:1-205`; IMK overrides `:206-353`; menus `:355-587`; `windowContent` `:679-701`; `flash` `:925-941`; `presentCandidates` `:1060-1090`; KVO and dismiss `:1112-1139`; picker chord / placeholder / dismiss `:1143-1153,1181-1251,1320-1340`; `armAutoSpaceSwap` `:1397-1410`; the swap's client check `:1442-1448`; `caretRect` `:1460-1492`; `endSession` / `finishComposition` `:1494-1541`; `onMainActor` `:1543-1582`.

**The shell surface desktop-core asks for.** `ComposingEffectExecutor::execute(&Effect)` (`C/composing/manager.rs:23-25`; nine `Effect` variants at `C/engine/transition.rs:16-46`, of which a shell acts on three: `UpdatePreedit { text, caret_utf16 }`, `ClearPreeditWithoutCommit`, `CommitTextReplacingPreedit`) and `IntentSurface` with nine methods (`C/composing/intent_executor.rs:18-48`): `insert_external`, `swap_preceding_space -> bool`, `arm_swap`, `has_write_failed`, `list_changed`, `list_closed`, `selected_index`, `index_for_key_slot`, `navigate`. Per key the input is a `KeyEventSnapshot` (six fields, `C/keys/snapshot.rs:80-100`) → `ComposingKeyIntent::intent` (`C/keys/intent.rs:136-141`) → `perform_intent(...) -> bool`. State: one `OnceLock<Mutex<ComposingSessionCoordinator>>` per process (`C/runtime.rs:55`), one `ContextToken` per client (`C/composing/coordinator.rs:49`). No timer; the shell is never asked for surrounding text. The Linux shell records effects under the lock and replays them after it (`L/taigi-linux-core/src/executor.rs:1-5`).

**Engine op sequence.** Both sides send the same nine composing ops and three next-word ops with the same config (`M/Engine/RustEngineBridge+Composing.swift:44-274` ↔ `C/engine/composing.rs:40-248`); the one difference is C1.

### Divergences

| # | Class | Input → macOS / desktop-core | Where |
|---|---|---|---|
| C1 ✔ | genuine | Every request: macOS sends `platform_id = macos`; desktop-core sends Windows unless the target is Linux | `M/Engine/RustEngineBridge.swift:181`; `C/engine/bridge.rs:87-91` |
| C2 | genuine, not verified | "Is this a letter": `Character.isLetter` / `char::is_alphabetic`. No test pins either | `M/Composing/ComposingManager.swift:116-118`; `C/composing/manager.rs:112-116` |
| C3 ✔ | genuine | One key event carrying `a.` while composing: macOS appends `a.` to the composition (reads the first character only) / desktop-core commits, then inserts `a.` (every scalar must qualify — a deliberate rule, see the comment at `intent.rs:262-267`) | `M/Keys/ComposingKeyIntent.swift:358-374`; `C/keys/intent.rs:245-273` |
| C4 | genuine, same end state | Refetch after a display-mode switch: macOS does not re-read Show Candidate Window / desktop-core clears the list when it is off | controller `:1100-1110`; `intent_executor.rs:223-226` |
| C5 | shell | Selection after a refetch: macOS keeps the absolute index / the Linux shell resets it | `M/Candidates/CandidatePresenter.swift:109-112`; `L/taigi-linux-core/src/chrome.rs:256-259` |
| C6 | genuine | Engine bring-up: macOS installs the lexicon and opens the user data at launch / desktop-core on the first consumed key (`prepare_for_first_key` is `pub`, so a shell may call it at launch) | `M/Bootstrap/AppDelegate.swift:42,47`; `C/runtime.rs:149-160` |
| C7 ✔ | platform | Caret chord: ⌥←/→ / Ctrl+←/→ (a crate constant today) | `M/Keys/ComposingKeyIntent.swift:199,243-247`; `C/keys/intent.rs:33,148-152` |
| C8 | platform | Auto-space swap check: macOS re-reads `selectedRange` and the previous character / Linux has a capability bit / Windows an `ITfRange` | controller `:1442-1447`; `L/.../executor.rs:92-103`; `WIN/taigi-windows-tsf/src/composition.rs:337-376` |
| C9 | platform | Arming the swap: macOS only on a collapsed selection past position 0 / the core calls `arm_swap()` unconditionally and the shell decides | controller `:1406-1410`; `intent_executor.rs:333-336` |
| C10 | platform | Commit write: one `insertText` / `ClearPreedit` then `Commit` | `M/Composing/ClientEffectExecutor.swift:52-57`; `L/.../executor.rs:114-117` |
| C11 | platform | Lifecycle end: macOS commits the raw text without an auto space / Linux `end_session` cancels | controller `:1520-1541`; `L/.../session.rs:344-369` |
| C12 | platform | Ownership: macOS claims in `activateServer` and answers `false` for a non-owner / Linux claims per key | controller `:238,716-718`; `L/.../session.rs:494-500` |
| C13 | platform | Global shortcuts: Carbon hotkeys / matched in the key path. The Rust snapshot has no repeat flag; the macOS picker chord reads one | `M/Settings/ShortcutActions.swift:258,264`; `L/.../session.rs:157,187-196`; `C/keys/snapshot.rs:80-100`; controller `:748` |
| C14 | platform | No caret rectangle: macOS drops the list; `list_changed` lets a surface do so | controller `:1065-1078`; `intent_executor.rs:35-36` |
| C15 | platform | The symbol-picker placeholder (one marked space) exists only on macOS | controller `:1225-1231` |
| C16 | platform | §34 literal cell: macOS gives it no slot key / Linux does | controller `:695-699`; `L/.../session.rs:558-561` |
| C17 | one side | Shift-tap English mode (`C/keys/language_mode.rs`, `shift_tap.rs`): Windows only; macOS removed it (controller `:208-217`) | |
| C18 | one side | Password-field gate: Linux shell only (`L/.../session.rs:197-211`); macOS behaviour not verified | |

### Swift tests with no Rust equivalent

Unit tests of types the plan deletes (port before deleting): `T/ComposingManagerLearningTests.swift:50,183`; `T/RustEngineBridgeComposingTests.swift:115`; `T/ComposingEffectDecodingTests.swift:14,58` (nine effects; Rust pins three at `C/engine/transition.rs:249-272`); `T/RustEngineBridgeAppConfigTests.swift:20` (Rust asserts Windows / Linux at `C/engine/bridge.rs:142-148`); `T/PresentedCandidateTests.swift:43,79`; `T/CandidateCellContentTests.swift:86`.

Controller-level scenarios with no desktop-core executor test (port, or keep the Swift suite running against the linked core): `T/AutoSpaceControllerTests.swift:41,113,304,349,385,407,428,451`; `T/FullWidthPunctuationControllerTests.swift:110,186,209,225,247`; `T/TaigiInputControllerCandidateTests.swift:1070,1102,1124,1145,1161`; shortcut actions (9), symbol picker (30), Telex guide (10) — covered only by five Linux-shell tests (`L/taigi-linux-core/src/chrome.rs:437-548`).

Shell tests that stay Swift: `ClientEffectExecutorTests` (5), `TaigiInputControllerTests` (8), `ActivateServerClientQueryTests` (1), `TaigiInputControllerMenuTests` (19), `T/AutoSpaceControllerTests.swift:168,183,197,225,242`.

---

## 3. Keys + shortcuts

| Swift (duplicate range) | LOC | Duplicate / stays | Rust counterpart | Status | Non-test callers |
|---|---|---|---|---|---|
| `M/Keys/ComposingAction.swift:24-189` | 190 | 150 / 40 | `C/keys/action.rs:14-165` | Identical | 26 |
| `M/Keys/ComposingKeyBindings.swift:17-114` (`CandidateSlotKeySet`), `:133-253` (`ComposingKeyBindings`) | 253 | 235 / 18 | `C/keys/slot_key_set.rs:18-103`, `C/keys/bindings.rs:20-191` | Identical rules; key codes K3 | 8 + 10 |
| `M/Keys/ComposingKeyChord.swift:17-258` | 258 | 210 / 48 | `C/keys/chord.rs:18-287` | Divergent K2 K3 K4 K6 | 41 |
| `M/Keys/ComposingKeyIntent.swift:11-114` (snapshot), `:122-483` (classifier) | 483 | 305 / 178 | `C/keys/snapshot.rs:12-165`, `C/keys/intent.rs:79-390` | Divergent K1 C3 K7 K8 | 11 + 12 |
| `M/Keys/SymbolPicker.swift:9-58` | 58 | 35 / 23 | `C/keys/symbol_picker.rs:15-66` | Identical | 1 |
| `M/Keys/ToneInputScheme.swift:20-55` | 55 | 28 / 27 | `C/keys/tone_input_scheme.rs:26-85` | Identical | 6 |
| `M/Settings/ShortcutActions.swift:7-196` (roster, defaults), `:205-302` (`ShortcutHotkeys`), `:309-566` (`ShortcutConflicts`) | 566 | 150 / 416 | `C/keys/shortcut_actions.rs:24-208,265-389`; hotkeys: none | Roster divergent K1 K5; conflict policy identical over a different data model | 15 + 4 + 6 |
| `M/Settings/ShortcutKeyRecorder.swift:28-79,404-468,479-519` | 519 | 45 / 474 | `C/keys/recorder.rs:94-152`, `shortcut_actions.rs:224-260`, `chord.rs:259-286` | Recorder decision identical; global policy K5 | 9 |
| `M/Settings/ShortcutDefaultMigration.swift` | 83 | 0 / 83 | none | macOS only | 1 |
| `M/Settings/ShortcutSettingsView.swift:154-206,252-279` | 311 | 25 / 286 | `C/keys/shortcut_labels.rs:13-72`, `recorder.rs:47-60` | Identical rules, platform spelling | 1 |
| `M/Panels/InputSourceMenuRenderer.swift` + controller `:372-485` | 74 | 0 / 74 | `C/keys/input_method_menu.rs:16-93` | Rows identical; chord column differs | 2 |
| `M/Panels/TelexGuidePanel.swift:11-31,76-93` | 191 | 45 / 146 | `C/keys/telex_guide_rows.rs:12-95` | Identical | 7 |
| `M/Panels/ModeFlashPanel.swift` | 90 | 0 / 90 | none | macOS only | 1 |

Code-only (non-blank, non-comment) the group is 1,433 lines, ~524 of them duplicate. `C/keys/shift_tap.rs` (304) and `language_mode.rs` (82) have no macOS counterpart.

### Divergences

| # | Class | Input → macOS / desktop-core | Where |
|---|---|---|---|
| K1 ✔ | genuine, blocks a 1:1 link | Modifier roles. ⌥← while composing: macOS moves the caret / the core (⌥ as `alt`) answers `CommitThenPassThrough`. Mapping ⌥ onto `control` instead breaks the width flip, which is ⌃ on macOS and `CONTROL` in the core. No modifier mapping satisfies both constants | `M/Keys/ComposingKeyIntent.swift:199,418`; `C/keys/intent.rs:33,40` |
| K2 ✔ | genuine, persisted | Stored chord letters: macOS writes `d/c/o/s` / the core reads `w/c/a/s` and loads an unknown letter as a cleared row. A stored `o\|000D` is ⌥↩ on macOS and nothing in the core | `M/Keys/ComposingKeyChord.swift:208-240`; `C/keys/chord.rs:240-247`, `bindings.rs:84-86` |
| K3 | platform | Key-code space: Carbon `kVK_ANSI_*` / Windows VK. Linux translates in its shell (`L/taigi-linux-platform/src/key_translation.rs:195-202`) | `M/Keys/ComposingKeyChord.swift:132-140`; `C/keys/chord.rs:53-62` |
| K4 | genuine | Function-key chords: macOS reserves the six navigation keys, Backspace, Delete, Escape — ⌃Home is a valid chord (`T/CrossTierShortcutConflictTests.swift:113-127`) / the core reserves all of U+F700–F8FF | `M/Keys/ComposingKeyChord.swift:34-53`; `C/keys/chord.rs:155-161` |
| K5 | one side / genuine | The global-shortcut tier: macOS stores a Carbon key code in the `KeyboardShortcuts` registry, fires from a Carbon hotkey on key-up, checks the live system hotkey table when recording, and migrates old defaults / the core stores a character chord under `globalShortcut.<raw>` and matches on the press in the key path. Recording policy differs (⌃⌥↩: accepted / `NotAGlobalKey`; ⇧⌘S: `belongsToHost` / accepted) | `M/Settings/ShortcutKeyRecorder.swift:8-20,34-74`, `ShortcutActions.swift:214,237-243`; `C/keys/shortcut_actions.rs:163-198,224-260` |
| K6 | genuine, edge | Case fold of a chord key: `lowercased()` / `to_ascii_lowercase`. ⌃⇧Ñ stores `ñ` on macOS and `Ñ` in the core | `M/Keys/ComposingKeyChord.swift:176`; `C/keys/chord.rs:149` |
| K7 | genuine, edge, not executed | Non-text scalars: `CharacterSet.controlCharacters` (Cc + Cf) / `is_control` (Cc) | `C/keys/intent.rs:387-389` |
| K8 | genuine, edge | A plain Escape is tested on the first character / on the whole string | |
| K9 | platform | Labels: `⌃⌥⇧⌘ ↩ ⇥ ⎋` / `Win+Ctrl+Alt+Shift+ Enter Tab Esc` | |

**Key model.** `characters`, `charactersIgnoringModifiers`, the named-special flag, the six navigation keys, ⌥-composed characters and the keypad-Enter / back-tab fold map onto `KeyEventSnapshot` without loss. Fn, the numeric-pad flag and Caps Lock are not representable and not needed (Swift already drops them, `M/Keys/ComposingKeyChord.swift:158-160`). Not verified: what JIS keys and forward Delete deliver.

**Swift tests with no Rust equivalent.** macOS values the core does not pin: `T/ComposingKeyIntentTests.swift:580,613,625` (⌥ caret chord); `T/ComposingKeyBindingsTests.swift:124,138` (raw letters); `T/ShortcutActionsTests.swift:79`, `T/ShortcutSettingsTests.swift:72,95`; `T/ShortcutKeyRecorderTests.swift:194,204,214,234,248`; `T/CrossTierShortcutConflictTests.swift:113`. Partial (Rust asserts fewer cases): `T/ComposingKeyBindingsTests.swift:400,419,434,438,489`; `T/ComposingKeyIntentTests.swift:190`; `T/CandidateSlotKeyTests.swift:196`; `T/ToneInputSchemeTests.swift:24`; `T/ShortcutActionsTests.swift:185`; `T/ShortcutSettingsTests.swift:28`. Shell rules that stay Swift tests: `T/ComposingKeyBindingsTests.swift:114,477`; `T/CandidateSlotKeyTests.swift:85-86,185-192`; `T/ComposingKeyIntentTests.swift:392,410,603`.

---

## 4. Candidate window

Group: 5,397 lines in 31 files; ~1,580 duplicate (29%), ~3,810 stay. Windows renders through the core model (the shell measures with DirectWrite, the core computes pages, rectangles and selection — `WIN/taigi-windows-tsf/src/ui/candidate_window.rs:308-317,450-498`). Linux does not use the layout module: the framework panel owns the UI and the core sends a flat table of nine (`L/taigi-linux-core/src/selection.rs:1-11`).

| Swift file (LOC) | Rust counterpart | Status | Duplicate / stays |
|---|---|---|---|
| `CandidateMetrics` (457) | `C/candidates/metrics.rs:37-64,99-195,283-388`, `C/settings/choices.rs:97-165` | Divergent W1–W3 | 290 / 165 |
| `CandidateIndexLabel` (75) | `C/candidates/index_label.rs:9-61` | Identical | 75 / 0 |
| `CandidatePanelPositioning` (70) | `C/candidates/positioning.rs:46-74` | Identical, y axis mirrored | 70 / 0 (+ ~15 adapter) |
| `ExpandedGridLayout` (102) | `C/candidates/grid.rs:18-114` | Identical | 102 / 0 |
| `HorizontalPageLayout` (206) | `C/candidates/horizontal.rs:29-187` | Identical | 206 / 0 |
| `CandidateCellContent` (73), `CandidateCellArrangement` (16), `CandidateLayout` (26) | `C/composing/cell_content.rs:33-70`, `metrics.rs:10-17`, `choices.rs:24-54` | Identical | 81 / 34 |
| `CandidateFontChoice` (112), `CandidateFontSelection` (70) | `choices.rs:172-309` | Roster identical; face identity platform-forced | 45 / 137 |
| `ExpandableCandidatePanel` (1,004) | `C/candidates/expandable.rs:87-136,168-225,328-533` | Decide-half identical; unfold and scroll ownership differ | 190 / 814 |
| `VerticalCandidatePanel` (576) | `C/candidates/vertical.rs:24-63,124-231,313-430` | Decide-half identical; scroll ownership differs | 175 / 401 |
| `HorizontalCandidatePanel` (177), `CandidateBasePanel` (472), `CandidatePresenter` (160) | `horizontal.rs:192-321`, `index_label.rs:25-47` | Identical slices | 98 / 711 |
| `Settings/StoredFontSelection` (70), `Settings/AppearanceMode` (42) | `C/settings/font_selection.rs:22-84`, `choices.rs:60-89` | Identical rules | 82 / 30 |
| Nine view files (`CandidateItemView`, `CandidatePanel`, `CandidateBackdrop`, `RegisteredFace`, `CandidateAccentColor`, `FontRegistryObserver`, `CandidateSeparatorView`, `CandidateWindowStyle`, `ScreenLookup`) + chevron / page-arrow views | none in the core | One side | 0 / 1,519 |

| # | Class | macOS / desktop-core | Where |
|---|---|---|---|
| W1 | genuine | Chrome ratio 0.7 / hard-coded 0.6. At Standard: padding 7 / 9, inset 6, row 26 on macOS; 6 / 8, 5, 25 in the core. All five size steps differ | `CandidateMetrics.swift:239`; `metrics.rs:64,474` |
| W2 | genuine | A stacked list with no annotation: macOS flips to the inline arrangement / the core swaps only the item height | `CandidateMetrics.swift:298-301`; `metrics.rs:185-195` |
| W3 | genuine | Index slot: widest label + 2 / floored at the index font size first | `CandidateMetrics.swift:345-348`; `metrics.rs:152-156` |
| W4 | platform | `CGFloat` / `f32` (no span mismatch for totals 100–1,399), points / DIPs, y-up / y-down, `NSScroller` read / passed in | |
| W5 | one side | Scroll offset: `NSClipView` bounds are the truth on macOS / the core owns `scroll_y`. Unfold choreography and row materialisation exist only in Swift; `UnfoldPlan` has no production consumer | `VerticalCandidatePanel.swift:473-519`; `vertical.rs:380-388`; `expandable.rs:263-287` |

The core gets text widths through `TextMeasurer` (`metrics.rs:30-35`), used only by `CandidateMetrics::resolve`; the layout models take pre-measured `&[f32]`. A macOS consumer needs no callback — four scalars per metrics value and two arrays per list — but the stateful models need a handle each, and a scroll event becomes a call plus a re-read.

Swift tests with no Rust equivalent: `CandidateMetricsTests:121,293` and `testBaseWidth_growsWithTheSize`; 10 panel-driven cases in `CandidateIndexLabelTests`; 4 in `CandidateSemanticNavigationTests`; 8 in `CandidateStackedCellTests`; all 4 `CandidateLiteralCellTests`; 7 of 9 `VerticalCandidateRowMaterialisationTests`; both `CandidateWindowStyleTests`; 3 view cases in `CandidateCellContentTests`; 4 Core Text cases in `CandidateFontSelectionTests`; 7 in `CandidateFontChoiceTests`. Fully covered by Rust: positioning (7), grid (5), page layout (13), elastic width (3).

---

## 5. Settings, storage, strings, symbols, engine bridge, lexicon services

| Swift file (LOC) | Rust counterpart | Status | Duplicate / stays |
|---|---|---|---|
| `M/Settings/SettingsStore.swift` (803) | `C/settings/keys.rs:30-291`, `document.rs:77-315` | Divergent S1–S5 | ~650 / ~153 (~200 / ~603 if `UserDefaults` stays) |
| `M/Settings/EngineSettings.swift` (195), `EngineSettingsProvider.swift` (17) | `C/settings/engine_settings.rs:10-213`, `C/settings/mod.rs:60-64` | Identical (nine fields, defaults, derivations) | ~182 / ~30 |
| `M/Settings/DictionarySourceToggles.swift` (112) | `engine_settings.rs:221-324` | Identical (13 + 11 toggles) | 112 / 0 |
| `M/Settings/RetiredSettingsCleanup.swift` (141) | none | macOS only | 0 / 141 |
| Settings UI, 12 files (1,581) | `C/settings/choices.rs:316-405`, `presentation.rs:11-26` | UI; 43 `@AppStorage` bindings read `UserDefaults` directly | ~43 / ~1,538 |
| `M/Storage/CustomFontLibrary.swift` (609) | `ST/font_library.rs:20-294` | Rules identical, S13–S16; Core Text half platform-forced | ~140 / ~469 |
| `M/Storage/UserDataDirectory.swift` (66), `CustomDictionaryRow.swift` (36) | `ST/directory.rs:28-42` (reads `%APPDATA%` only) | Platform-forced | 0 / 102 |
| `M/Strings/DisplayLanguage.swift` (97), `DisplayLanguageStore.swift` (128), `StringResolver.swift` (46) | `C/strings/mod.rs:22-209` | Fallback chain identical; S17; formatter platform-forced | ~131 / ~140 |
| `M/Symbols/SymbolTable.swift` (109), `RecentSymbols.swift` (39) | `C/symbols.rs:11-200` | Identical (same JSON asset; cap 9, most recent first) | ~139 / ~9 |
| `M/Engine/RustEngineBridge.swift` (201) | `C/engine/bridge.rs:11-125` | Divergent C1 | ~125 / ~76 |
| `…+Composing.swift` (388), `…+NextWord.swift` (133) | `C/engine/composing.rs:40-276`, `nextword.rs:27-104` | Identical | 521 / 0 (callers are the composing group) |
| `…+Lexicon.swift` (285), `…+Phonetics.swift` (66) | `C/engine/lexicon.rs:22-382`, `phonetics.rs:15-77` | Identical | 351 / 0 |
| `…+UserData.swift` (137) | `C/engine/user_data.rs:31-315` | Divergent S8 | 137 / 0 |
| `M/Engine/DictionaryArtifacts.swift` (49), `DictionarySource.swift` (63) | `C/dictionary_artifacts.rs:10-61`, `lexicon.rs:31-75` | Identical | 112 / 0 |
| `M/Lexicon/DictionarySearchService.swift` (214), `DictionarySearchPage.swift` (180), `ExternalLookupURLBuilder.swift` (107) | `C/engine/dictionary_search.rs:14-128`, `external_lookup.rs:8-100` | Identical; **dead on macOS** (S21) | 501 / 0 |
| `M/Lexicon/UserDataClient.swift` (159) | `C/engine/user_data.rs:56-296` | Divergent S9 S10 | ~110 / ~49 |
| `M/Lexicon/CustomDictionaryPage.swift` (486) | `C/settings/custom_dictionary.rs:17-330` | Divergent S11 | ~150 / ~336 |
| `M/Lexicon/DictionaryTogglesView.swift` (138), `UserDataPageChrome.swift` (396), `UserDataFilePanels.swift` (83) | `C/settings/dictionary_sources.rs:11-83`, `presentation.rs:69-105` | Row order, delays, export name identical | ~67 / ~550 |
| Bootstrap (491), `Logging/DebugLogger.swift` (58) | `C/runtime.rs:165-188` ↔ `AppDelegate.swift:146-170` | macOS only | ~25 / ~524 |

`AppConfig` matches field by field (`RustEngineBridge.swift:171-200` ↔ `bridge.rs:113-125`) except `platform_id`. Forty-four named setting keys plus `composingShortcut.<action>` have the same spelling and default on both sides (`SettingsStore.swift:42-334` ↔ `keys.rs:30-222`); the reset rosters are the same sets (General 8, Appearance 4, Sources 24).

### Divergences

| # | Class | macOS / desktop-core | Where |
|---|---|---|---|
| S1 | platform | Settings backend: `UserDefaults.standard` with KVO / `settings.json` with a lock file, atomic rename and an mtime reload. No importer exists | `SettingsStore.swift:347-353,725-803`; `ST/settings_file.rs:42-311` |
| S2–S4 | platform / one side | Update bookkeeping keys: `updateNextCheckDate` (`Date`) / `updateNextCheckMs` (`i64`); `updatePendingManifest` `Data?` / string; `hasOfferedUpdateNotifications` macOS only | `SettingsStore.swift:203-223`; `keys.rs:10-11,143-154` |
| S5 | one side | Global shortcut rows live in the document on desktop and in the `KeyboardShortcuts` library on macOS; composing chords use the same key spelling on both | `C/keys/shortcut_actions.rs:163-165`; `M/Keys/ComposingAction.swift:162-164` = `C/keys/action.rs:149-151` |
| S6 | genuine | `SettingsPane.dictionarySearch` exists on desktop; macOS has no such pane and sweeps a stored value | `choices.rs:327,401`; `SettingsSplitView.swift:13-26` |
| S8 ✔ | genuine | User-data journal: macOS opens `DELETE` / desktop-core hard-codes `WAL`. The engine refuses a second open at another journal | `M/Engine/RustEngineBridge+UserData.swift:21`; `C/engine/user_data.rs:35`; `engine/userdata/src/requests.rs:25-27` |
| S9 | genuine | New custom entry: macOS mints the UUID / the engine mints it | `CustomDictionaryRow.swift:28`; `custom_dictionary.rs:91-96` |
| S10 | genuine | CSV import of an unreadable file: Foundation error / `could not read the file: …`. 5 MB cap on both | `UserDataClient.swift:116,120`; `user_data.rs:287-292` |
| S11 | genuine | Custom Dictionary page: **macOS asks for no confirmation before a destructive command; desktop confirms both**. Filter as typed / trimmed. Reload after a failed write: no / yes. Off-page selection kept / cleared | `UserDataPageChrome.swift:208-210`; `custom_dictionary.rs:28-56,82-89,249,269-274` |
| S13–S16 | genuine | Font library: name suffix unbounded / 100 attempts; exists-check then copy / `create_new` plus bounded read; `localizedStandardCompare` / byte sort; **path-component guard on the stored name: desktop only** | `CustomFontLibrary.swift:226,279-280,300,393-397,601-607`; `ST/font_library.rs:38,146,179-193,205-207` |
| S17 | genuine | Automatic language: `hasPrefix("ja" / "en")` / exact subtag | `DisplayLanguage.swift:74-82`; `strings/mod.rs:79-90` |
| S19 | one side | i18n key sets: 185 Swift, 181 Rust; 13 Swift-only (menu, notification offer), 9 Rust-only (installer, reset) — one generator, two scopes | `tools/i18n/i18n_lib.py:1280-1310` |
| S21 ✔ | one side | **Dictionary Search has no production caller on macOS**: nothing under `macos/Sources` constructs `DictionarySearchPage` or `DictionarySearchService`, and `SettingsDetailView` routes no search pane. Dead with it: the URL builder, every phonetics op, the lexicon search / filter ops, `LexiconBitmask`, `customDictionarySearch` — about 790 lines kept alive only by tests | `SettingsSplitView.swift:168-183` |
| S22 | genuine, dead | External lookup: `Character.isNumber` / `is_ascii_digit`; `URLQueryItem` / a stricter encoder | `ExternalLookupURLBuilder.swift:83`; `external_lookup.rs:48-64,83` |
| S24 | genuine | Dictionary stamp: `CFBundleVersion` / the crate version (a `RuntimeParts` field, so a shell passes its own) | `AppDelegate.swift:155-156`; `C/runtime.rs:36-37` |
| S26 | platform | Bool decoding: `as? Bool` (accepts a number) / JSON boolean only | `SettingsStore.swift:744-746`; `document.rs:77-82` |

### Engine linkage (grounded in code)

- macOS links `engine/swift-ffi` as `librust_taigi.a`: `crate-type = ["staticlib"]`, `default = ["user-data"]` (`engine/swift-ffi/Cargo.toml:19-33`), built for both darwin triples and merged with `lipo` (`engine/scripts/build-macos-xcframework.sh:38,59-87`), consumed as the SwiftPM binary target `RustTaigi` (`macos/Package.swift:27-42`).
- The bridge exports `process_request_bytes`, `install_logger_sink`, `set_log_level`, `panic_for_test`, `e2e_trace_open` and the `SwiftLoggerSink.log` callback (`engine/swift-ffi/src/lib.rs:41-55`).
- The engine is a set of process-wide statics: lexicon `STATE` (`engine/lexicon/src/handle.rs:46`), composing `HANDLE` (`engine/composing/src/handle.rs:59`), next-word `HANDLE` (`engine/nextword/src/handle.rs:42`), user-data `HANDLE` (`engine/userdata/src/handle.rs:31,43`), the logger statics (`engine/swift-ffi/src/lib.rs:158-160`).
- desktop-core calls `dispatch::process_request` in-process (`C/engine/bridge.rs:42`) and declares `dispatch` without default features (`desktop/Cargo.toml:38`); each shell turns `user-data` on along its own edge.
- **A second static library that also contains `dispatch` would give the process two engines** — two lexicon states, two composing sessions, two writers on the same `.db` files at different journal modes — or fail the link on duplicate `sqlite3_*` symbols (reasoned, not link-tested). The engine and desktop-core have to arrive in one archive.

Swift tests with no Rust equivalent: all 11 `RetiredSettingsCleanupTests`; 13 of 14 `DictionarySearchServiceTests` (`C/engine/dictionary_search.rs` has no test); 8 `RustEngineBridgeDictionaryFiltersTests`; 7 `UserDataPageChromeTests`; 5 `SettingsStoreTests` (`:327,476,487,512,524`); 5 `DisplayLanguageStoreTests`; 5 Core Text cases in `CustomFontLibraryTests`; 3 each in `DisplayLanguageTests`, `ExternalLookupURLBuilderTests`, `LexiconBitmaskTests`; 2 `StringResolverTests`; one each in `DictionarySourceSettingsTests:31`, `RustEngineBridgeAppConfigTests:20`, `UserDataClientTests:24`, `EngineFfiSmokeTests`.

---

## 6. Rules that live in a Rust shell, not in desktop-core

A macOS link reaches only what `desktop/crates` owns. These are written once per Rust shell today (Windows and Linux each carry a copy) and once more in Swift:

- The per-key preamble: ownership, picker state, Telex-guide dismissal, the chord latch (`L/taigi-linux-core/src/session.rs:148-337`; `WIN/taigi-windows-tsf/src/session.rs`, 1,460 lines).
- What each global action does (`L/taigi-linux-core/src/chrome.rs:134-230`).
- Opening the symbol picker and picking from it (`chrome.rs:292-401`).
- Candidate-window rules in the Windows shell: the width budget, chevron and page-arrow widths, in-place re-render, the per-cell label loop (`WIN/taigi-windows-tsf/src/ui/candidate_window.rs:45-47,66-70,239-262,406-445,713-725`).
- The install state machine and the package verifier of the update flow (`WIN/taigi-windows-update/src/installation.rs`, `verify.rs`).

---

## 7. Update check / verify / install

Group: 1,731 lines. With the shared crate as it is, `URLSession` and `UserDefaults` kept, about 132 lines (8%) are duplicate; 897 lines of trust, notification and UI stay under every scenario.

Three blockers:

1. **Schema** — the Rust manifest yields a package only when `packageURL` and `packageSHA256` are both valid (`DU/manifest.rs:83-89,101`); the macOS manifest carries no digest (`macos/updates/README.md`), so decoding it in Rust would turn the in-app install into "open the download page".
2. **Storage** — the bookkeeping functions take `&SettingsDocument` (`DU/checker.rs:45-107`, `C/settings/update_schedule.rs:13-33`); macOS keeps these values in `UserDefaults`.
3. **Linkage** — `DU/manifest.rs:75` uses `ureq::http::Uri` and `ureq` is an unconditional dependency, so the pure logic cannot be linked without a TLS stack; the crate's own rule is that the input-method process does not load it, and the macOS check runs inside that process (`M/Bootstrap/AppDelegate.swift:119-128`).

Replacing `URLSession` with `ureq` would also change macOS behaviour: `ureq` reads a proxy from the environment only, so a Mac behind a system proxy or a PAC file would stop reaching the manifest; the idle timeout becomes a connect timeout; a downloaded file loses its quarantine attribute.

What must stay Swift whatever is shared: `UpdatePackageVerifier` (Team ID pin of the running copy, `pkgutil --check-signature`, the leaf certificate's CN and OU, the `Distribution` bundle id and version — `M/Updates/UpdatePackageVerifier.swift:64-188`), the quarantine attribute, `NSWorkspace.open`, `UNUserNotificationCenter`. The Windows rule that lets a digest stand in when the running copy is unsigned (`WIN/taigi-windows-update/src/verify.rs:115-118`) must not reach macOS.

Genuine divergences in the shared half:

1. A non-string `packageURL` drops the field on macOS (`M/Updates/UpdateChecker.swift:114-121`) and fails the whole manifest in Rust (`DU/manifest.rs:50-55,94`).
2. URL check: `https` scheme only / scheme plus a non-empty host (`UpdateChecker.swift:52,120`; `DU/manifest.rs:74-80`).
3. Timeouts: 5 s / 30 s are idle timeouts on macOS and connect timeouts in Rust (`M/Updates/UpdateHTTP.swift:10-17`; `DU/transport.rs:61-69`).
4. The 200 MiB cap is checked after the download on macOS and while streaming in Rust (`M/Updates/UpdateInstallation.swift:257-264`; `DU/transport.rs:116-131`).
5. macOS records an announcement only after the system accepted it; Rust claims first (`UpdateChecker.swift:356-360`; `DU/checker.rs:97-107`).
6. macOS clears a stale pending manifest at launch and withdraws its notification; Rust only filters it (`UpdateChecker.swift:245-256`; `DU/checker.rs:45-58`).
7. Package version match: string equality / dotted-version equality (`M/Updates/UpdatePackageVerifier.swift:160-161`; `WIN/taigi-windows-update/src/verify.rs:182-188`).
8. Fetch errors: one `malformed` / four `FetchError` kinds — the user sees "failed" either way.
9. An `http` hop inside a redirect chain: both check only the final URL.

---

## 8. Other findings

- `macos/RustEngine/README.md` says "single slice `macos-arm64`" and "committed copies"; the build is universal and the artefacts are gitignored (`.gitignore:101-103`).
- `M/Settings/EngineSettings.swift:136-140` says iOS / Android default Show Typed Text First on; the value is `false` (`:188`).
- `macos/App/Info.plist:66-69` carries `CFBundleVersion` 30612 beside `CFBundleShortVersionString` 3.6.13.
- 275 comments in 90 Rust files under `desktop/`, `windows/`, `linux/` cite a `*.swift` file as the rule's origin.
