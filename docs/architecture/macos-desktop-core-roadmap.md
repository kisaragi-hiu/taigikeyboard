# macOS over desktop-core — Roadmap

> **Type**: Planning (multi-PR refactor)
> **Keywords**: `macOS`, `desktop-core`, `refactor`, `FFI`, `behaviour oracle`, `parity`
> **Status**: in progress — scope and design decisions taken 2026-10-02 (§ Decisions); P1 merged #342, P2 merged #343, P3 merged #344, P4 merged #345, P5 merged #346, P6 merged #347, P7 merged #349, P8 next
> **Source**: `docs/reports/2026-09-30-audit-all.md` §2 ("macOS re-implements what `desktop/crates` already has") and §5.2; measured inventory `docs/reports/2026-10-02-macos-desktop-core-inventory.md` (frozen on `e330cd30`)
> **Session memory**: project memory `project_macos_desktop_core.md` (Claude auto-memory)

---

## Mandate

USER 2026-10-02: the macOS input method stops re-implementing desktop behaviour in Swift and links the shared Rust `taigi-desktop-core`, so a desktop feature is written once for macOS, Windows and Linux and their parity stops being a manual chore.

Carried over from the maintainability mandate (`maintainability-roadmap.md` § Mandate): a stored **setting** may change; user **data** — the engine `userdata` stores, copied-in fonts — survives every phase. Release, version and tag decisions are the USER's and no phase is assigned to a release.

## Summary

- **Measured, not estimated**: macOS has 20,389 hand-written Swift lines; about 8,070 of them have a counterpart in `desktop/crates`. The audit's "~4,000" is replaced by the table below (inventory report §1).
- **This plan moves the key path** — key classification, bindings and chords, the composing manager, the intent executor, auto space, full-width punctuation, next-word, the candidate list model, the engine-settings snapshot and the engine bring-up: about **4,300 Swift lines deleted**, plus the generated `*.pb.swift` files nothing references afterwards. The intent switch in `TaigiInputController` is already a one-to-one port of `intent_executor.rs`, which is what makes this the low-risk half.
- **It leaves in Swift** what is IMKit / AppKit / SwiftUI, and four duplicate areas where the boundary would cost more than it saves or would change behaviour: candidate-window geometry, the settings storage backend, the global-shortcut tier, the update flow (§ Outside this plan).
- **desktop-core becomes the behaviour oracle** for the three desktops, replacing "macOS is the behaviour oracle" (`docs/contributing/windows-guidelines.md:82`). A platform difference becomes a named `DesktopPlatform` value with a test per platform.

## Today (grounded in code)

- **Engine link**: macOS links `engine/swift-ffi` as the static library `librust_taigi.a` with `user-data` on (`engine/swift-ffi/Cargo.toml:19-33`), built universal by `engine/scripts/build-macos-xcframework.sh:38,59-87` and consumed as the SwiftPM binary target `RustTaigi` (`macos/Package.swift:27-42`). Swift talks to the engine through one function, `process_request_bytes`, with protobuf envelopes (`engine/swift-ffi/src/lib.rs:41-70`; `macos/Sources/TaigiInputMethodCore/Engine/RustEngineBridge*.swift`).
- **Duplicate logic**: everything between the key event and that function is Swift on macOS and Rust on Windows / Linux — `Keys/`, `Composing/`, `Policies/`, `NextWord/`, the controller's intent switch (`Controller/TaigiInputController.swift:787-923` ↔ `desktop/crates/taigi-desktop-core/src/composing/intent_executor.rs:60-172`).
- **What desktop-core asks of a shell**: a `KeyEventSnapshot` in; `perform_intent` out through the `IntentSurface` trait (`intent_executor.rs:18-59`); a `SettingsProvider` and the directories through `RuntimeParts` (`runtime.rs:24-40`). One coordinator, one manager and one current `ContextToken` per process (`runtime.rs:55`, `composing/coordinator.rs:49-66,82-117`).
- **What desktop-core does not own**: the per-key preamble, the bodies of the global actions, the symbol picker — each Rust shell has its own copy (`linux/crates/taigi-linux-core/src/session.rs:148-337`, `chrome.rs:134-401`; inventory report §6). A macOS link does not reach them.
- **Why a 1:1 link fails today**: desktop-core hard-codes Windows / Linux choices — the caret chord and the width flip are both `CONTROL` (`keys/intent.rs:33,40`; macOS uses ⌥ and ⌃), stored chords spell modifiers `w/c/a/s` (`keys/chord.rs:240-247`; macOS writes `d/c/o/s`), every request reports the platform as Windows unless the target is Linux (`engine/bridge.rs:87-91`), and the user data opens write-ahead logged (`engine/user_data.rs:35`; macOS opens `DELETE`).
- **The engine is process-wide statics** (`engine/composing/src/handle.rs:59`, `engine/lexicon/src/handle.rs:46`, `engine/userdata/src/handle.rs:31,43`).

## Inventory and scope

Per-file rows with `file:line` on both sides, every divergence and the tests without a Rust equivalent: inventory report §2–§7.

| Group | Swift | Duplicate | In this plan | Why / why not |
|---|---|---|---|---|
| Composing, policies, next-word, controller switch, candidate list model | 3,102 | ~1,580 | **yes** (~1,580) | Faithful port; highest drift cost |
| Keys: classifier, bindings, chords, slot keys, tone scheme, symbol-picker classifier | 1,297 | ~963 | **yes** (~963) | Same; needs the platform rules first |
| Engine bridge: composing, next-word, lexicon, phonetics | 872 | ~872 | **yes** (~872) | Callers are the groups above or dead code |
| Engine settings snapshot, source toggles, dictionary artefacts and stamp | 436 | ~406 | **yes** (~406) | The core builds `AppConfig` and installs the lexicon |
| Dictionary Search (service, page, URL builder) — dead on macOS | 501 | 501 | **yes** (delete) | No production caller (report S21) |
| Shortcuts tier (roster, recorder policy, conflicts, hotkeys, migration) and the menu / Telex-guide / mode-flash panels | 1,834 | ~265 | no | Different design on macOS (`KeyboardShortcuts` + Carbon hotkeys); behaviour change + migration |
| Candidate-window geometry | 5,397 | ~1,328 | no | 71% is AppKit; three pixel divergences; scroll ownership |
| Settings storage, page models, user-data client, strings, symbols, font library, bootstrap | ~5,800 | ~1,700 | no | Persisted-format change, or stateful UI models over the boundary |
| Update check / verify / install | 1,731 | ~132 | no | Three blockers; trust decisions stay in Swift |

In plan: about 4,300 hand-written lines out. The `composing` / `nextword` / `lexicon` / `phonetics` / `case` `*.pb.swift` files (7,757 generated lines) stop being generated for macOS **one by one, as each loses its last reference** — the retained user-data UI keeps `envelope` and `user_data`. Every phase re-measures with `wc -l` and records the number in the PR table.

## Design

### D1. One static library (grounded in code)

The engine and desktop-core reach the app in **one** archive: two Rust static libraries that each contain `dispatch` risk either a link failure on duplicate symbols or two copies of the engine's static state over the same `.db` files, depending on what the linker keeps — neither is acceptable.

A new crate `macos/crates/taigi-macos-ffi` (own workspace `macos/Cargo.toml`, the `linux/` and `windows/` shape) depends on `taigi-desktop-core`, on `dispatch` with `user-data`, and on `swift-ffi` — which gains `rlib` beside `staticlib`, the `taigi-linux-ffi` precedent (`linux/crates/taigi-linux-ffi/Cargo.toml:11-13`). `build-macos-xcframework.sh` builds this crate instead of `swift-ffi`; the xcframework and its module keep the name `RustTaigi`, so `Package.swift` does not change (it finds the library through the xcframework, whatever the archive is called). iOS keeps building `engine/swift-ffi` alone and its staging is verified unchanged.

Spiked in P2 (`known-pitfalls.md` § Spike a platform capability; result in § Decisions). **P2 opened with a ≤20-line spike** that proved, for both slices under the new workspace's release profile: the `swift-ffi` bridge exports are in the dependent archive (the generator emits plain `#[export_name]` C exports, so they should be), a Swift target links against them, and two bridge modules stage with `SwiftBridgeCore.swift` once. Had it failed: `taigi-macos-ffi` declares all five engine entry points and the logger callback in its own bridge over helpers moved to a non-bridge module, and does **not** depend on the exporting `swift-ffi` — redeclaring the same export names beside it would collide.

### D2. Boundary mechanism — a second bytes seam

Chosen (USER 2026-10-02, § Decisions): the mechanism macOS already uses for the engine. One more bridged function, `desktop_request_bytes(&[u8]) -> Vec<u8>`, carrying protobuf messages defined in `macos/crates/taigi-macos-ffi/proto/desktop_shell.proto` (prost in Rust; `gen-macos-protos.sh` emits the Swift).

- No handle crosses the boundary: the runtime is a process singleton inside the crate, a session is addressed by a token — the engine adapters' shape (`docs/contributing/rust-ffi-safety.md` §1.1).
- **No new `unsafe`**: `&[u8]` in, `Vec<u8>` out is what `process_request_bytes` does today.
- Both sides get generated types from one schema — no hand-synchronised constant tables.

What the choice costs, and how the plan pays it:

- **A token is identity, not ownership.** Nothing frees a session when Swift forgets it. The contract (D3) names activation, release, a never-reused token and stale-token rejection, and P9 tests them.
- **The safety rules do not come for free**: `catch_unwind`, the request size cap, the error envelope and the malformed-input tests (`rust-ffi-safety.md` §1.2, §1.4, §1.5, §6 T1 / T4 / T5) are implemented and tested for the new envelope from P2 (the Version request, Codex pre-review), and P8 / P9 extend the tests to each request they add.
- **Every key encodes a request and copies the candidate list** into the reply. Held keys and long lists are part of the device pass.
- **The local schema mirrors native enums**; every effect and action translation has a test, and an unknown request variant is refused.

Alternatives considered, both workable: a hand-written C ABI with opaque handles like `taigi-linux-ffi` (pointer / free contracts and accessor functions on both sides; its consumer is C++, which has no protobuf runtime — Swift already links SwiftProtobuf), or swift-bridge opaque types (automatic release, but a new ownership and error surface, and generated destructors to square with the panic rule).

### D3. Boundary shape — one request in, one reply out

| Request | Carries | Reply |
|---|---|---|
| `Configure` | data directory, dictionaries directory (each optional), dictionary stamp, system locale | the settings whitelist: name, kind, default (D5) |
| `Settings` | the key-path settings as typed entries (D5) | — |
| `Prepare` | — | lexicon install stats |
| `Activate` | token | — (claims the coordinator; `activateServer`) |
| `Key` | token, key code, `characters`, `charactersIgnoringModifiers`, modifier flags, panel state | `handled` + effects |
| `CommitComposition` / `Cancel` | token, panel state | effects |
| `CommitForSymbolPicker` | token, panel state | effects — the highlighted cell, else the composition as typed with its auto space (P7: replaces `PickCandidate`, which no macOS path sends — a click only selects, `CandidateItemView.swift:54`; the Linux shell's `commit_for_picker`, `session.rs:392-417`) |
| `InsertSymbol` | token, symbol, panel state | effects (the Swift picker writes through the core's `insert_symbol`, `intent_executor.rs:179-189`) |
| `Represent` | token, refetch flag, panel state | the list unchanged, repainted in place, or closed (after a Swift global action switched a mode; `represent_list`, `intent_executor.rs:239-254`) |
| `Release` | token | — |

Effects (replayed by Swift in order, after the call returns): `SetMarkedText { text, caret_utf16 }`, `ClearMarkedText`, `InsertText`, `SwapPrecedingSpace { replacement }`, `ArmSwap`, `CandidatesChanged { cells, leads_with_literal_roman, marked_text_length_utf16 }` (the length anchors the caret walk), `CandidatesClosed`, `Navigate { direction }`. A cell carries what the core already computes for it (content, script, literal flag). Every reply also carries `is_composing` (P7: the Swift protocol asks it after each key and around the picker's commit; the core back end answers from the last reply, and `owns` from its own `Activate` / `Release` record).

- **Record, then replay.** The Rust surface that implements `IntentSurface` only records; Swift talks to the IMK client after the call returns, so the coordinator lock is never held across a client call and a re-entrant client callback cannot deadlock on it — the Linux shell's rule (`linux/crates/taigi-linux-core/src/executor.rs:1-5`). The order among client calls is preserved. What changes is that the engine calls of one key now all happen before its first client call (before P7: append → `setMarkedText` → fetch, `TaigiInputController.swift:788-790` @ `96930b1c`); no client can observe that.
- **Panel state travels in** with every request that can act on the list: the selected index, the slot → index map, whether a list is on screen, whether a swap is available. `IntentSurface::selected_index` / `index_for_key_slot` answer from those values; each arm of `perform_intent` reads them once, before it produces any effect (`intent_executor.rs:144-161,304-313`). The swap's client check (`TaigiInputController.canSwapAutoSpace`, P7) runs in Swift before the call; Swift keeps its replacement range and its re-arm arithmetic.
- **The panel is the truth about what is shown.** macOS drops a list it has no caret rectangle for (`TaigiInputController.swift:1065-1078`). When a request says "no list on screen" and the session still holds one, the session clears it before it classifies anything, so Space and the slot keys never reach candidates the user cannot see. Selection after a refetch stays the panel's (macOS keeps the absolute index; the Linux reset at `linux/.../executor.rs:166-167` is a Linux-shell policy).
- **`has_write_failed` answers `false`.** The executor asks it after writing the auto space (`intent_executor.rs:325-335`); IMKit's `insertText` reports no failure (`Composing/ClientEffectExecutor.swift:27-57`), so macOS has no failure to report today either. This is a macOS assumption, not a property of the trait.
- **Tokens.** Swift mints one per controller from a counter and never reuses it. `Activate` claims; `Release` is sent from `inputControllerWillClose` / `deinit`. A request for a token that is not the current owner is answered `IGNORED` with no effects — the non-owner rule the controller has today (`TaigiInputController.swift:716-718`).
- **Stale replies.** Swift replays an effect only while the controller still owns the session; a client callback that deactivates the controller mid-replay drops the remaining effects. P10 tests exactly that.
- **Lifecycle has its own requests**: `deactivateServer` / `commitComposition` need `ComposingManager::commit_composition`, not `perform_intent(Commit)`, which would add an auto space (`intent_executor.rs:76-86`).
- **Key translation lives in Rust** (`taigi-macos-ffi/src/key_translation.rs`): `NSEvent` fields → `KeyEventSnapshot`, Carbon key codes → the core's key space — the `taigi-linux-platform` precedent (`linux/crates/taigi-linux-platform/src/key_translation.rs:195-202`), table-tested without an event.
- **What stays in the controller**: IMK overrides, menus, panels, the caret rectangle, the Carbon hotkey handlers and what each global action does, the symbol picker's window, chord and placeholder.
- **The Swift side (P7)**: protocol `ComposingBackend` (`macos/Sources/TaigiInputMethodCore/Composing/ComposingBackend.swift`) — `owns`, `isComposing`, `activate`, `release`, `key`, `commitComposition`, `commitForSymbolPicker`, `insertSymbol`, `represent`. Each request carries the controller's settings store (what P8 encodes as the snapshot) and the panel state, whose selection, slot and swap answers are closures: the legacy back end asks them where the old key path did, the core back end before it encodes. `Cancel` is not on it — only the core back end sends it, after `FAIL_INTERNAL` (D4). `TAIGI_COMPOSING_BACKEND` picks the back end per process (unset or `legacy` today; an unknown name stops the process).

### D4. Threading and errors

- Every call is made on the main thread — IMKit delivers `handle(_:client:)` there and the controller is main-actor bound (`TaigiInputController.swift:1543-1582`). The Rust runtime still locks per call (`runtime.rs:128-132`), so a stray call from another thread is serialised, not undefined.
- The engine finishes opening the user data on a thread of its own, as today (`runtime.rs:153-155`).
- Malformed bytes answer `FAIL_PARSE`; an oversized request `FAIL_INVARIANT`; a panic `FAIL_INTERNAL`. No Rust error type crosses.
- **After `FAIL_INTERNAL` the key is consumed, not handed to the host**: the engine may already have advanced, and a host that processed the key again would type it twice. Swift clears the marked text, closes the list, sends `Cancel`, and logs.

### D5. Settings stay in `UserDefaults`

The 43 `@AppStorage` bindings and the KVO observer stay. Swift sends the **key-path settings only** — a fixed whitelist, each as a typed entry (boolean or text), an absent key meaning "default" — and `taigi-macos-ffi` writes them into a `SettingsDocument` behind a `SettingsProvider` (`settings/mod.rs:60-64`). Not the raw defaults domain: the document's JSON form nests values under `values` (`settings/document.rs:44-51`), so a flat dictionary would parse into an empty document, and the domain also holds `Date` and `Data` values the document has no type for.

- The push is synchronous and ordered: Swift rebuilds the snapshot from `UserDefaults` and sends it before **every** request after `Configure` (as built in P6 — no KVO, no dirty flag: nothing can go stale between a write and the next request, so a mode toggled by a global action is in force for the `Represent` that follows). Each push replaces the whole snapshot; a refused one (unknown, repeated or value-less name, wrong kind) keeps the previous snapshot. **From P8** (USER 2026-10-02) the snapshot travels inside each request — an optional field of `DesktopRequest`, applied before dispatch and refusing the whole call when invalid, as the engine envelope carries `config_snapshot` — so a key crosses the seam once and its settings and its request apply in one step; the separate `Settings` request and the Swift pre-push go.
- The macOS spellings: the chord letters `d/o` (report K2) are read by the core in the `MacOS` grammar; a number where a boolean is expected (report S26) is normalised by the Swift side, which reads every whitelisted name with the expression `SettingsStore` reads it with (`object(forKey:) as? Bool`, `string(forKey:)`) — so no Rust replica of Foundation's number bridging exists — and `taigi-macos-ffi` checks the wire kind.
- `settings_store` is `None`. Nothing on the key path writes settings (`perform_intent` and `ComposingManager` only read; `runtime.rs:94-108` is called from shell chrome). What `None` switches off — launch shortcut reconciliation (`runtime.rs:193-199`), mode toggles, the recent-symbol list — stays in Swift, where it is today.
- The whitelist is the core's (`taigi-desktop-core/src/settings/key_path.rs`, next to the readers it lists), answered by `Configure` with each name's kind and default; Swift keeps no second list. A Swift test compares each whitelisted default with `SettingsStore`'s and the name sets both ways.
- The system locale is the one `Configure` carried — a launch snapshot; no macOS caller reads the core's strings today.

### D6. `DesktopPlatform` in desktop-core

A copyable value `{ Windows, Linux, MacOS }` — not `cfg(target_os)`: the desktop crates' tests run on a Mac host and on Linux CI, and each platform's rules must be testable on both.

- **Pure rules take it as an argument** (or a small rules value derived from it): `ComposingKeyIntent::intent` (the width-flip / document-text helpers take none — the width flip is ⌃ / Ctrl on all three, `ComposingKeyIntent.swift:418`), the chord codec — `from_raw`, `translate_raw`, `raw_value`, `make`, `matches` must agree on one spelling and one normalisation (`keys/chord.rs:72,134,145,194,226`) — and the Shortcuts labels. None of them has a runtime in hand (`keys/intent.rs:136-141`).
- **`RuntimeParts` carries it** for the runtime's own consumers; `app_config` and `user_data::open` receive it, since `EngineSettings` alone does not reach them (`engine/bridge.rs:113-125`, `engine/user_data.rs:31-45`).
- It selects: the engine `platform_id`, the user-data journal, the caret-chord modifier, the chord-letter spelling, the reserved chord keys, the case fold of a chord key, the Shortcuts labels.
- Windows and Linux pass their own value and keep every rule they have today. Call sites that change: `windows/crates/taigi-windows-tsf/src/session.rs:194`, `runtime.rs:69`; `linux/crates/taigi-linux-core/src/session.rs:284`, `runtime.rs:115`; both settings apps' `user_data::open` (`windows/crates/taigi-windows-settings/src/user_data.rs:22`, `linux/crates/taigikeyboard-settings/src/user_data.rs:27`) and Shortcuts pages (`windows/.../winui/pages/shortcuts.rs:76,102`, `linux/.../pages/shortcuts.rs:55,80`); inside the core `keys/bindings.rs:85`, `keys/shortcut_actions.rs:196`, `keys/shortcut_labels.rs:23,38`, `engine/composing.rs` (eight `app_config` calls), `engine/nextword.rs:93`.

### D7. Build, bundle, CI

- **Archive**: `make build` / `make macos-libs` build `taigi-macos-ffi` for both darwin triples with `MACOSX_DEPLOYMENT_TARGET=14.0` (matching `LSMinimumSystemVersion`, `macos/App/Info.plist:78-79`) and `lipo` them — the script's existing shape. The **new workspace's** release profile governs the archive, so it restates the engine's (`lto`, `codegen-units = 1`, `strip`, the unstripped `build-override`, `panic = "unwind"`).
- **Staging** (P2): `engine/scripts/lib/swift-bridge-artifacts.sh` finds each bridge's build-script output by package name and stages extra `(out_dir, bridge_name)` pairs into the one `RustTaigi` module, `SwiftBridgeCore.swift` once after checking every copy is identical. The iOS path through the same helper was re-run (release and `--dev`) and its staged files compared unchanged.
- **Features**: `e2e-trace` and `panic-injector` are forwarded through the new crate; the release script keeps `e2e_trace_assert_absent` (`build-macos-xcframework.sh:85`). The injector's symbol is present in every build by design — only its body is gated (`engine/swift-ffi/src/lib.rs:118-132`).
- **Bundle**: `bundle-app.sh` keeps its architecture, deployment-target and symbol assertions and, from P6 (the first production caller), asserts both entry points in the executable; `build-macos-xcframework.sh` asserts both in every archive slice (P2).
- **Caller's view**: `make -C macos build / test / install` are unchanged; `macos/Makefile` still never rebuilds Rust.
- **Stale-artifact gate** (`AGENTS.md` § Build & Test): a diff touching `desktop/` or `macos/crates/` needs `make build` before a macOS build or test.
- **Tooling**: `macos/rust-toolchain.toml` (both darwin targets); `tools/test_select.py` maps `desktop/**` and `macos/crates/**` to **two** macOS gates — the native Rust tests and, after a rebuild, the Swift suite; `.github/workflows/macos.yml` adds the paths, caches the new workspace and runs the native tests, clippy, fmt and the MSRV check; `.github/workflows/security.yml` and the advisory checks cover the new lockfile; `make lint` and `tools/release_notes.py` (which implements `make version-desktop`) learn the workspace.

### D8. The oracle rule

`windows-guidelines.md` § "macOS is the behaviour oracle" becomes: **desktop-core and its tests are the behaviour oracle for the three desktops.** A rule lives there once; a platform difference is a `DesktopPlatform` branch with a test per platform, or a named shell divergence. The rule is swapped in the cut-over PR. The 275 comments in 90 Rust files that cite a `*.swift` file as a rule's origin are rewritten to state the rule in a separate, last PR.

### D9. Proving behaviour is frozen

No single check proves it; the package is:

1. **One suite, two back ends.** The controller's key path goes behind a Swift protocol (`ComposingBackend`) with the existing code as its legacy implementation; the core-backed implementation is added beside it and the existing controller suites run against each. **Each back end runs in its own `swift test` process**, selected by an environment variable: both would otherwise drive the one process-wide engine with separate generation counters (`Composing/ComposingSessionCoordinator.swift:46`; `composing/coordinator.rs:61-66`) and contaminate each other. The suites' settings fixtures write `.standard` (`AutoSpaceControllerTests.swift:13-23`); the core back end gets the equivalent injection through `Settings`.
2. **Ported unit tests.** Tests of deleted Swift types are ported to desktop-core first (report §2, §3 list them), under the `MacOS` platform value.
3. **Native tests of the boundary**: key translation tables, every effect and request translation, token rules, the envelope errors.
4. **Integration tests the suites do not reach**: launch order and once-only preparation, journal and dictionary stamp, the settings push (change, reset, removal), lifecycle requests, a deactivation during replay.
5. **A device pass** at the cut-over, before any legacy code is deleted: IMK re-entrancy, caret geometry, focus changes, held keys, long lists.

## Parity decisions

**Preserved on every platform, no decision needed** — the core gains the macOS value as a `DesktopPlatform` rule, and Windows and Linux keep theirs: modifier roles (K1), chord letters (K2), key codes (K3), `platform_id` (C1), journal (S8), engine bring-up at launch (C6), dictionary stamp (S24), labels (K9), **reserved chord keys (K4)** and **case fold of a chord key (K6)**. The last two were candidates for one shared rule, but the Windows recorder manufactures private-use scalars for F1–F24, Insert, Home and End (`windows/crates/taigi-windows-platform/src/key_translation.rs:232-252,263-268`), so narrowing the reserved range would change what it accepts.

**Rare inputs where macOS and the core differ in the classifier itself** — USER 2026-10-02: macOS takes the core's rule.

| # | Input | macOS today | desktop-core today | Status |
|---|---|---|---|---|
| E2 (C3) | One key event carrying several characters (`a.`, `a5`) | classified by the first character | every character must qualify — a deliberate rule (`keys/intent.rs:262-267`) | core rule on macOS — P11 |
| E2b (K8) | A plain Escape inside a multi-character event | first character | whole string | **P5: real difference**, predicate only — `"\u{1B}x"` closes the Mac symbol picker, the core answers `CloseAndPassThrough`; the composition's Escape tier reads the first character on both (Cancel). No single key press carries it. Core rule on macOS — P11. Pinned: `symbol_picker.rs` `e2b_an_escape_inside_a_longer_event_does_not_close_the_picker` |
| E4 (K7) | Format characters (Cf) as "not text" | yes | no (Cc only) | **P5: real difference**, isolated (`U+200B`, `U+00AD`, `U+FEFF`) and embedded (`x‍y`, `👩‍💻`): composing, macOS commits then passes the key to the host, the core commits then inserts it; `is_document_text` false / true, so the core hands the Cf text to the manager's next-word gate, which forwards an isolated Cf or an emoji ZWJ sequence (`x‍y` still stops there, on the letter). Same document text when the host inserts the passed-through key. Core rule on macOS — P11, reported to the USER. Pinned: `intent.rs` `e4_a_format_character_is_text_to_the_core` |
| E5 (C2) | "Is a letter / is whitespace" in the manager — it gates the next-word notification | per grapheme (`Character`, first scalar of each) | per scalar (`char`) | **P5: real difference** only for a non-letter, non-whitespace base followed by an Other_Alphabetic mark (`。` + U+0345, `,` + U+093E): forwarded on macOS, skipped by the core. A non-Alphabetic mark (U+0301) and whitespace-led graphemes: no difference. Core rule on macOS — P11, reported to the USER. Pinned: `tests/composing_manager.rs` `e5_a_mark_that_is_alphabetic_keeps_the_character_from_next_word` |
| E6 | Chord gate's typing-key test (`is_typing_key`) on a key whose Unicode fold adds a combining mark (`İ` → `i̇`) | first grapheme — not ASCII, so not a typing key | first scalar `i` — a typing key, so refused bare | **P5: real difference**: bare and ⇧`İ` record on macOS and are refused by the core; a stored `s\|0069,0307` reads as a cleared row in the core. ⌃`İ` binds on both; `Ñ` and the Kelvin sign agree. **USER 2026-10-02: core rule on macOS** (`İ` is a typing key, refused bare) — P11. Pinned: `chord.rs` `e6_a_fold_that_adds_a_combining_mark_is_a_typing_key_on_the_mac` |
| E7 | A stored chord with an empty hex field (`c\|0041,,0042`) | field skipped, chord parsed | whole value refused (`translate_raw`) | **P5: real difference** for hand-edited values only (`raw_value` never writes an empty field): `c\|0041,,0042`, `c\|,005D`, `c\|005D,` parse on macOS, refused by the core; `c\|` refused on both. **USER 2026-10-02: core rule on macOS** (the value refused whole) — P11. Pinned: `chord.rs` `e7_a_stored_chord_with_an_empty_hex_field_does_not_parse` |

Each change lands in its own `parity:` PR with both sides described and a test pinning the result (`cross-platform-alignment.md` §1b), never inside a refactor PR. A divergence of another kind that a ported test uncovers — anything that is not a rare-input classifier rule — stops the phase and goes to the USER.

## Phases

One phase = one PR. Size counts added and changed lines; a deletion-only diff may be larger.

| Phase | Type | Content | Est. | Status |
|---|---|---|---|---|
| P0 | docs | This roadmap, the inventory report, memory | — | this commit |
| P1 | refactor (macOS) | Delete the dead Dictionary Search code and what only it kept alive (report S21), with their tests. Independent of the migration; first because it shrinks the bridge the later phases touch | −874 / +7 src measured (6 files deleted, 4 trimmed), tests −4 files +1 (est. −790) | Merged #342 `766e4921` |
| P2 | build | Spike (D1); `macos/Cargo.toml` + `taigi-macos-ffi` with a version request; one archive; staging helper; feature forwarding; release profile; toolchain file (from P3: the archive build runs in `macos/` and needs its targets); iOS staging verified; Swift smoke test; `rust-ffi-safety.md` §1.1 names the seam | ~400 | Merged #343 `338e59b2` |
| P3 | build | Tooling (D7): test selection, `macos.yml` native job, security workflow, lint, version script, `AGENTS.md` stale-artifact row | ~250 | Merged #344 `9c8004d5` |
| P4 | refactor (desktop-core) | `DesktopPlatform` (D6) through the pure rules, `RuntimeParts`, `app_config`, the journal, the chord codec and the labels; Windows, Linux and both settings apps pass theirs | ~450 | Merged #345 `9859405a` |
| P5 | test (desktop-core) | Port the macOS tests with no Rust equivalent for the key path, under `MacOS`; characterise E4, E5, E6, E7, E2b | ~450 (+559 / −15 measured, tests only) | Merged #346 `824104c6` |
| P6 | refactor (macOS) | Runtime over the boundary: `Configure` / `Settings` / `Prepare`; launch bring-up moves from `AppDelegate` + bridge to the core. Tests: journal, stamp, defaults, `platform_id`, once-only | ~400 (+1,327 / −302 measured incl. tests; Swift src +235 / −177) | Merged #347 `1fbbb408` |
| P7 | refactor (macOS, Swift only) | `ComposingBackend` seam — the complete protocol, with the existing key path as its legacy implementation | ~400 moved (controller 1,582 → 1,280; +711 in two new files) | Merged #349 `7249ff42` |
| P8 | refactor (Rust, additive — not wired) | Session I: `Activate` / `Key` / `CommitComposition` / `Cancel` / `Release`, the recording surface, key translation, envelope errors and size cap; the settings snapshot rides on each request (D5, USER 2026-10-02) instead of its own `Settings` push; native tests | ~500 | Pending |
| P9 | refactor (Rust, additive — not wired) | Session II: `CommitForSymbolPicker`, `InsertSymbol`, `Represent`, list-visibility reconciliation, stale-token rejection; native tests | ~350 | Pending |
| P10 | refactor (macOS) | Core back end in Swift; controller suites per back end in separate processes; the integration tests of D9.4; a cross-check that the Shortcuts pane's Swift chord rules and the core's agree on every action and a table of chords. Cases that hit an open E-item are listed, not silently skipped | ~450 | Pending |
| P11 | parity | E2 / E2b, E6, E7, and E4 / E5 where P5 found a real difference — one PR each as needed | small | Pending |
| P12 | cut-over | Default back end → core; the oracle rule (D8); macOS device pass | small | Pending |
| P13 | refactor (macOS) | Delete the legacy key path, the bridge extensions and settings snapshot it used, their unit tests; drop each generated `*.pb.swift` that lost its last reference (two PRs if the diff is unreadable) | −3,200 src | Pending |
| P14 | refactor (macOS) | The Shortcuts pane reads chord grammar, validation, action defaults and slot keys from the core (durable requests — the pane stays Swift); delete the Swift chord / binding / action logic | ~300, −600 | Pending |
| P15 | comments + docs | The `*.swift` cites in Rust comments (D8); `system-overview.md`, `AGENTS.md` structure line | — | Pending |

Order constraints: P4 and P5 before anything reads a rule from the core; P6 after the typed settings adapter exists (it is part of P6); P7 before P10; P12 only when P10's suites are green on both back ends and P11 is settled; P13 only after the device pass. Between P12 and P14 the Shortcuts pane's Swift rules and the core's coexist, held equal by P10's cross-check.

### Behaviour-freeze contract (every phase from P6)

Observable properties preserved from `HEAD~1`, except an accepted, labelled E-item:

1. The `Bool` each `handle(_:client:)` returns.
2. Every client call — `setMarkedText` (text, selection, underline), `insertText` (text, replacement range) — and their order.
3. The candidate list: cells, annotations, scripts, order, selection, slot labels, shown or hidden.
4. The engine request sequence and the `AppConfig` of each request (`platform_id = macos`).
5. User data: directory, `DELETE` journal, what one pick records.
6. Settings: keys, defaults, storage in `UserDefaults`; no key is rewritten.
7. Shortcuts pane: accepted and refused chords, defaults, conflict resolution.
8. Launch: lexicon installed and user data opened at launch; dictionary stamp `CFBundleVersion`.

For P4 the same list applies to Windows and Linux, with their own values.

Callers that must still work (from the report's caller counts): `TaigiInputController`; the Shortcuts pane (`ShortcutSettingsView`, `ShortcutKeyRecorder`, `ShortcutConflicts`, `ShortcutActions` — `ComposingKeyChord` alone has 41 call sites); `SettingsStore` chord accessors; `TelexGuidePanel`; `InputSourceMenuRenderer`; `AppDelegate` bring-up; `ComposingSessionCoordinator`'s shortcut-target registry. For P4: the call sites listed in D6.

### Gates per PR

- Refactor pre-gate: the freeze list above + every caller; `refactor-reviewer` on the diff.
- Review sandwich on every PR that touches Rust on the boundary (`rust-ffi-safety.md` §5), even with no new `unsafe`.
- Tests: `make -C macos test` per back end; the native tests of `macos/crates`; `make desktop-check`; `make windows-check` and `make linux-check` whenever `desktop/` changes; hosted CI for the Windows build.
- Device: one macOS pass at P12, after the default flips and before P13 deletes anything (USER 2026-10-02) — IMK re-entrancy, caret geometry, focus changes, held keys, long lists. The other phases rest on the tests above.

## Best practices alignment

Rules per phase: P1, P7, P13, P14 — `cross-platform-alignment.md` §1 (behaviour-frozen refactor). P2, P6, P8, P9 — `rust-ffi-safety.md` §1 (seam, panic, size cap, error envelope), §5 (two reviews), §6 (test contract). P4 — §1c (no new platform reads inside shared code). P11 — §1b (a parity correction is isolated, described on both sides, pinned by a test). P5 — `known-pitfalls.md` § Trace before assert. All — no redundant fallback (`AGENTS.md` § Design principles): the legacy back end is a temporary compatibility path, **removed in P13**.

| Mainstream practice | Source | This plan |
|---|---|---|
| One host-neutral core, thin per-OS front ends | librime under Squirrel / Weasel / ibus-rime (`mainstream-ime-comparison.md:221-223`); azooKey-Desktop shares `Core/Sources/` with iOS (`:171-178`) | The whole plan |
| One runtime per process, one engine per text field, one runtime lock | ChiaKey `Frameworks/ChiaKeyCore/Headers/ChiaKeyCore/ChiaKeyCore.h` (`chiakey-reference.md:75-78`) | D3: singleton runtime, token per controller |
| One key in → one reply with commit, composition and candidates | KeyKey `Loaders/Windows-IMM/BaseIMERPC/BIServerRPCInterface.idl` `BISHandleKey` (`mainstream-ime-comparison.md:115`); hazkey `protocol/base.proto` request / response envelope (`:117`) | D3: `Key` request, effect list |
| A single bytes entry point into the core | khiin-rs `khiin/src/engine.rs:57` `send_command_bytes` | D2 |
| Config is host-owned and re-applied by the host; the core never reads a platform preference store | ChiaKeyCore (`chiakey-reference.md:81`) | D5: `UserDefaults` pushed as a snapshot |
| No raw pointer cast back to `&mut`; no panic across the seam | khiin-rs counter-examples `android/rust/src/lib.rs:51-56,64` (`rust-ffi-safety.md` §1.2, §1.3) | D2, D4 |

### Deliberately not adopted

- **A hand-written C ABI with opaque handles** (the `taigi-linux-ffi` shape, ChiaKeyCore's `ChiaKeyCoreC.h`): right for a C++ consumer; for Swift it adds a header, accessor functions and `unsafe` on both sides that the bytes seam does not need.
- **ChiaKeyCore's commit acknowledgement** (`chiakey-reference.md:79`): it guards a host whose insertion can fail; IMKit reports none (D3).
- **An out-of-process engine host** (rakukan, PIME, KeyKey's server, hazkey — `mainstream-ime-comparison.md:112-117`): the engine stays in-process on every platform.
- **`cfg(target_os)` for platform differences**: untestable across hosts (D6).
- **Leaf modules first over throw-away exports** (full-width map, auto space) and **key rules for the Shortcuts pane before the session moves**: either would create a request surface, or a second authority, that exists only for the interim.
- **A `taigi-macos-core` session crate mirroring `taigi-linux-core`**: a third copy of the per-key preamble and chrome. The preamble stays in the controller; hoisting the Windows / Linux copies into desktop-core is a separate refactor of those shells.

### Outside this plan (YAGNI, with the measurement)

- **Candidate-window geometry** (~1,328 duplicate of 5,397): 71% is AppKit; metrics differ in three places that change pixels on one platform (report W1–W3); the vertical and expandable models fight `NSScrollView` for the scroll offset.
- **Settings in `settings.json`** (~450 more lines): a persisted-format change needing an importer and the rewiring of 43 bindings, for no user-visible gain.
- **Global shortcuts as core chords** (~420 more lines): drops `KeyboardShortcuts` and the Carbon hotkeys; changes recording policy and needs a migration (report K5).
- **Settings page models, strings, symbols, font library**: stateful UI models or OS-bound halves; the i18n key sets differ by design (report S19).
- **Update flow** (~132 of 1,731): three blockers and a proxy regression (report §7).

## Found while auditing — not part of this plan

Each needs its own decision; none is scheduled here.

- macOS asks for **no confirmation before a destructive Custom Dictionary command**; Windows and Linux confirm (report S11). USER 2026-10-02: add the confirmation — its own round, unscheduled (`docs/roadmap.md` § Open candidates).
- The font library's stored-name **path-component guard exists only on desktop** (report S16).
- Candidate metrics differ between macOS and Windows at every size step (report W1–W3) — under the old oracle rule that is drift on Windows.

## Decisions

- 2026-10-02 — plan drafted from the five-pass inventory. A Codex analysis-only review of the draft answered GO-WITH-CHANGES; all eight required changes are in this text: the activation / release / stale-owner contract and list-visibility feedback (D3), the typed whitelisted settings snapshot (D5), `DesktopPlatform` as an argument to the pure rules and both settings apps in its phase (D6), K4 / K6 preserved per platform with E4 / E5 characterised before a decision, back ends tested in separate processes plus integration tests (D9), the build surface spelled out and split (D7, P2 / P3), parity corrections in their own PRs and the Shortcuts-pane rules moved after the cut-over (P11, P14), the two-engines claim restated as a risk with a collision-free fallback (D1). It agreed with the bytes seam for a Swift-only consumer.
- 2026-10-02 — P2 spike passed (D1): `taigi-macos-ffi` with `extern crate rust_taigi` (swift-ffi as rlib) built one archive whose arm64 and x86_64 slices export all five engine bridge functions plus `desktop_request_bytes`, with no duplicate global symbol (Apple's `nm` cannot read the `compiler_builtins` bitcode members, in the old engine archive too); both bridges' `SwiftBridgeCore.{h,swift}` are byte-identical; Swift linked the archive and called both seams. The crate's library is `taigi_macos_ffi` (`rust_taigi` is swift-ffi's name), and the xcframework carries `libtaigi_macos_ffi.a` under it. The `taigi-desktop-core` edge is added in P6, with its first caller. Codex pre-review GO-WITH-CHANGES, all applied.
- 2026-10-02 — P4 design (Codex pre-review GO-WITH-CHANGES, all applied): an explicit `DesktopPlatform` argument on every rule that differs, not a field of `ComposingKeyBindings` or `EngineSettings`; each shell's constant lives in its platform crate (`taigi_windows_platform::DESKTOP_PLATFORM`, `taigi_linux_platform::DESKTOP_PLATFORM`); `ComposingManager` and `EngineNextWord` hold the value, `engine::reset` sends no config and takes none; both rosters' default chords are platform-free literals pinned through the gate on all three desktops; the global tier takes the platform only because it calls the shared codec. E6 / E7 found while tracing the Swift codec.
- 2026-10-02 — P6 design (Codex pre-review GO-WITH-CHANGES, all applied): the settings push is rebuilt before every request instead of KVO-tracked (D5); the whitelist is the core's, answered by `Configure`; the Swift side normalises a stored number to a boolean with `SettingsStore`'s own expression; `Configure` is once-only and publishes the runtime whole (a second one is refused before any side effect), so `RuntimeParts.system_locale` became a boxed closure (Windows / Linux wrap their function); each directory stays optional on its own, as at launch before. The lexicon install now goes through the core's `DictionaryArtifacts::locate` (also refuses a directory in a file's place) and logs in the core's words — a diagnostic difference only; a normal bundle sends the same `InstallRequest`. Swift `DictionaryArtifacts` / `lexiconInstall` / `userDataOpen` went with their last caller; the test fixture prepares through the core.
- USER 2026-10-02, after P6 (each the recommended option): E7 takes the core rule on macOS (P11); from P8 the settings snapshot rides on each request (D5); `prepare_for_first_key` keeps its name (renaming touches the Windows / Linux callers; the core docs name the macOS timing); P6 gets no retrospective Codex review (refactor-reviewer PASS, CI green; P7's pre-review reads the same code); the Custom Dictionary confirmation is its own round. The stale-text line of § Found while auditing is closed: `macos/RustEngine/README.md` was already rewritten in P2, the `EngineSettings.swift` literal-roman default comment is fixed. E6 (asked again in plain terms): core rule on macOS too (P11).
- USER 2026-10-02, four decisions, each the recommended option:
  1. **Scope** — the key path only (about 4,300 Swift lines); candidate-window geometry, the settings backend, the global-shortcut tier and the update flow stay Swift.
  2. **Boundary** — the protobuf bytes seam (D2), in place of the C ABI the brief named.
  3. **Rare-input parity** — macOS takes the core's rule (E2, E2b; E4 and E5 once characterised), each in its own `parity:` PR.
  4. **Device dogfood** — once, at the cut-over (P12), before the legacy code is deleted.
