# TaigiKeyboard — Roadmap

> **Type**: Planning (forward-looking)
> **Keywords**: `roadmap`, `planning`, `released versions`, `release trains`
> **Status**: Active
> **Last updated**: 2026-10-02 (macOS over desktop-core planned). 2026-10-01: maintainability audit follow-up complete. 2026-09-30: mobile v3.6.10 / v3.6.11 rows; bigram LM closed. 2026-09-26: Active items all merged — collapsed into Closed phases; design bodies frozen in `docs/reports/2026-09-26-shipped-roadmap-design-notes.md`

---

## Summary

- **Forward-looking work items only.** Shipped detail lives in `docs/releases/<version>/plan.md` + `changelog/mobile-<version>.md`.
- **Active**: macOS over desktop-core — planned, awaiting the maintainer's approval before any implementation PR. Pending dogfood is tracked in `docs/architecture/dogfood-checklist.md`.
- **Open candidates**: every unfinished, parked or brainstorm item across the roadmaps and reports is listed once in § Open candidates (unscheduled), with a link to its design; the one design-locked item is the converted-romanization commit.
- **Release scope / timing / tag is the maintainer's call.**

---

## Active / In-flight items

| Item | Status | Where |
|---|---|---|
| **macOS over desktop-core** — the macOS input method links the shared Rust `taigi-desktop-core` for its key path instead of re-implementing it in Swift (about 4,300 of the ~8,070 duplicate Swift lines measured; candidate-window geometry, the settings backend, global shortcuts and the update flow stay Swift) | P0–P6 merged 2026-10-02 (#342–#347); P7 next | [`architecture/macos-desktop-core-roadmap.md`](architecture/macos-desktop-core-roadmap.md) · inventory [`reports/2026-10-02-macos-desktop-core-inventory.md`](reports/2026-10-02-macos-desktop-core-inventory.md) |
| **Desktop TPS mode** — 方音符號 as a third input mode on macOS, Windows and Linux: Dachen positions with a Shift layer, its own switch shortcut, keypad candidate picking, an on-screen key panel | P0 plan 2026-10-03; P1 (engine `TpsKey`) next | [`architecture/desktop-tps-roadmap.md`](architecture/desktop-tps-roadmap.md) |
| **Learning Records page** — view, correct the count of, and delete one learned row (word frequency, phrases) on all five platforms | Complete 2026-10-03: P0 `5bab5b9b`; P1 #366, P2 #369, P3+P4 #370, P5+P6 #371 merged; device dogfood pending | [`architecture/learning-records-page-roadmap.md`](architecture/learning-records-page-roadmap.md) |

Everything else scoped through 2026-10-01 has merged (see Closed phases). Pending dogfood: `docs/architecture/dogfood-checklist.md`.

---

## Released versions index

Newest first. Two trains since 2026-09: mobile `mobile-x.y.z` (iOS + Android) and desktop `desktop-x.y.z` (macOS + Windows + Linux; earlier desktop tags `macos-v*` / `windows-v*` live in the website repo). Links: release notes (`changelog/`) + detailed plan archive where one exists. Authoritative ship-date list: memory `project_released_versions.md`.

| Version | Ship date | Release notes | Detailed plan archive |
|---|---|---|---|
| mobile v3.6.11 | 2026-09-29 (`mobile-3.6.11` @ `cb410a17`) | [`changelog/mobile-v3.6.11.md`](../changelog/mobile-v3.6.11.md) | — (Android 3.6.10 upgraders get the custom dictionary + frequency back; `association.bin` v2 word pairs; collapsible theme preview) |
| mobile v3.6.10 | 2026-09-28 (`mobile-3.6.10` @ `1da2ebaa`, re-cut; first cut `ec5fe549` 2026-09-27) | [`changelog/mobile-v3.6.10.md`](../changelog/mobile-v3.6.10.md) | — (one-handed mode, photo theme background, learned phrases, typed separators, No Hyphens, user data in the engine; mobile skips v3.6.9) |
| desktop v3.6.10 | 2026-09-24 (`desktop-3.6.10` @ `f6f00b47`, Linux-only patch; macOS / Windows stay on 3.6.9) | [`changelog/desktop-v3.6.10.md`](../changelog/desktop-v3.6.10.md) | — (Fcitx5 selection keys, vertical-window arrow keys, Linux Fonts pane removed) |
| desktop v3.6.9 | 2026-09-23 (`desktop-3.6.9` @ `6523379e`, re-cut; first Linux packages uploaded 2026-09-24) | [`changelog/desktop-v3.6.9.md`](../changelog/desktop-v3.6.9.md) | [`docs/reports/2026-09-26-shipped-roadmap-design-notes.md`](reports/2026-09-26-shipped-roadmap-design-notes.md) — learned phrases, Telex tone 1 / 4, input-source menu |
| mobile v3.6.8 | 2026-09-12, re-cut 2026-09-14 (`mobile-3.6.8` @ `4022956b`, first cut `3a399505`) | [`changelog/mobile-v3.6.8.md`](../changelog/mobile-v3.6.8.md) | — (Candidate Display picker, Show Typed Text default on, POJ tone placement + `o͘ⁿ`, auto-space follows commit, Android strip lag; mobile skips v3.6.6/v3.6.7) |
| desktop v3.6.8 | 2026-09-11 (`desktop-3.6.8` @ `440171d2`, same-version overwrite ×2) | [`changelog/desktop-v3.6.8.md`](../changelog/desktop-v3.6.8.md) | [`docs/reports/desktop-3.6.x-design-notes.md`](reports/desktop-3.6.x-design-notes.md) — Telex, candidate-window toggle, symbol picker, composing caret, ⇧+slot, custom + installed fonts, Shortcuts blocks |
| desktop v3.6.7 | 2026-09-04 (macOS first 2026-09-03; assets overwritten several times through 2026-09-05; tags live in the website repo) | [`changelog/desktop-v3.6.7.md`](../changelog/desktop-v3.6.7.md) | — (first Windows release; POJ `au` tone placement; Candidate Display picker on desktop) |
| v3.6.6 | 2026-08-28 (macOS-only package; no tag in this repo) | [`changelog/desktop-v3.6.6.md`](../changelog/desktop-v3.6.6.md) | — (letter-key candidate selection, Caps Lock ABC, in-app update download; iOS KeyboardKit 10.9.0) |
| v3.6.5 | 2026-08-28 (`v3.6.5` @ `84304d82`) | [`changelog/mobile-v3.6.5.md`](../changelog/mobile-v3.6.5.md) | — (macOS first release; TPS + candidate-accuracy round; §40 NextWord contract; Android 11 floor) |
| v3.6.4 | 2026-08-07 (`v3.6.4` @ `ed38499f`) | [`changelog/mobile-v3.6.4.md`](../changelog/mobile-v3.6.4.md) | — (App UI i18n — five display languages) |
| v3.6.3 | 2026-06-20 (`v3.6.3` @ `acea9a8f`) | [`changelog/mobile-v3.6.3.md`](../changelog/mobile-v3.6.3.md) | — (TPS fixes: explicit tone, tone 9, `ir`; single-initial input; Android autocorrect / vibration / rich-editor backspace) |
| v3.6.2 | 2026-06-12 (`v3.6.2` @ `c550e100`) | [`changelog/mobile-v3.6.2.md`](../changelog/mobile-v3.6.2.md) | — (keyboard theme picker + custom theme editor; Show Romanization toggle) |
| v3.6.1 | 2026-06-06 (`v3.6.1` @ `d1259966`) | [`changelog/mobile-v3.6.1.md`](../changelog/mobile-v3.6.1.md) | — (cross-mode user-data consistency R1–R7, Hanji-with-romanization literal candidate, backup exclusion; contracts in `architecture/behavioral-invariants.md`) |
| v3.6.0 | 2026-05-31 (`b782205c`) | [`changelog/mobile-v3.6.0.md`](../changelog/mobile-v3.6.0.md) | — (kautian subcoll + dev supplement + source-toggle filtering + explicit-tone fix) |
| v3.5.9 | 2026-05-29 (`3c8bec16`) | [`changelog/mobile-v3.5.9.md`](../changelog/mobile-v3.5.9.md) | — (TPS tri-index + Tier-A/B refactor; design memo `project_v359_d_tps_triindex_plan.md`) |
| v3.5.8 | 2026-05-20 (`61df3028`) | [`changelog/mobile-v3.5.8.md`](../changelog/mobile-v3.5.8.md) | [`docs/releases/v3.5.8/plan.md`](releases/v3.5.8/plan.md) — Phase 0-9 + whole-sentence lattice + walker S1-S9 + continuous-compound-hyphen fix |
| v3.5.7 | 2026-05-08 | [`changelog/mobile-v3.5.7.md`](../changelog/mobile-v3.5.7.md) | — |
| v3.5.6 | 2026-04-27 | [`changelog/mobile-v3.5.6.md`](../changelog/mobile-v3.5.6.md) | — |
| v3.5.5 | 2026-04-12 | [`changelog/mobile-v3.5.5.md`](../changelog/mobile-v3.5.5.md) | — |
| v3.5.3 | 2026-03-22 | [`changelog/mobile-v3.5.3.md`](../changelog/mobile-v3.5.3.md) | — |
| v3.5.2 | 2026-03-08 | [`changelog/mobile-v3.5.2.md`](../changelog/mobile-v3.5.2.md) | — |
| v3.5.1 | 2026-02-25 | [`changelog/mobile-v3.5.1.md`](../changelog/mobile-v3.5.1.md) | — |
| v3.5.0 | 2026-02-12 | [`changelog/mobile-v3.5.0.md`](../changelog/mobile-v3.5.0.md) | — |
| v3.4.x | 2025-2026 | [`changelog/mobile-v3.4.*.md`](../changelog/) | — |
| v3.3.x | 2025 | [`changelog/mobile-v3.3.*.md`](../changelog/) | — |

Detailed plan archives are added retroactively only when source material exists; older versions remain release-notes-only.

---

## Open candidates (unscheduled)

Forward-looking candidates only, NOT items already shipped. None is assigned to a release; scope and timing are the maintainer's call. (v3.5.8-era items that read like candidates but shipped — `whole-sentence lattice + walker`, `continuous compound-hyphen`, `Phase 9 user-freq plumb` — live in [`docs/releases/v3.5.8/plan.md`](releases/v3.5.8/plan.md).)

### Other open items

One line each; the linked section holds the design, the measurements and the open questions. Device dogfood lives in [`architecture/dogfood-checklist.md`](architecture/dogfood-checklist.md) (every pending item marked PASS 2026-10-02).

**Engine gaps and small decisions**

| Item | Status | Where |
|---|---|---|
| `lexicon::classification::is_hanji` stops at CJK Extension E; the Python pipeline tests through Extension G | measure callers first, then align | [`architecture/bigram-lm-roadmap.md`](architecture/bigram-lm-roadmap.md) § Open USER decisions #6 |
| Stale `.proto` comments: `lexicon.proto` still documents `DEV` as always-on (the toggle shipped); `composing.proto` cites a nonexistent `continuous-input-ranking.md` §10.11 and (`:426`) describes the iOS commit as `clearMarkedText` + `insertText` | needs a proto regen round (iOS + Android generated trees) | [`architecture/macos-roadmap.md`](architecture/macos-roadmap.md) § Open items this track produced |
| v2 `.taigi` restore folds POJ→TL over canonical-TL readings (macOS folds v1 only) | verify it still applies now the engine owns the codec | same § |
| Fedora × IBus e2e skipped (daemon never lists the test component) | own root-cause round | [`architecture/e2e-testing-roadmap.md`](architecture/e2e-testing-roadmap.md) § PR table PR4 |
| iOS #352: Flutter hosts (Cashew) keep the raw preedit on commit. Root cause proven on device: proxy calls of one turn are merged and applied as inserts → last `setMarkedText` → `unmarkText`, so any same-turn mix of a marked commit and another write breaks (Space, punctuation, auto-space). Fix: one host write per event vs. defer post-commit writes | parked until the current refactor ends (USER 2026-10-03); PR #357 draft, not mergeable as is (Space regresses Notes); fix option not chosen | [PR #357](https://github.com/taigikeyboard/taigikeyboard/pull/357) comments |
| macOS Custom Dictionary: no confirmation before a destructive command (Windows and Linux confirm) | USER 2026-10-02: add it — own round | [`reports/2026-10-02-macos-desktop-core-inventory.md`](reports/2026-10-02-macos-desktop-core-inventory.md) S11 |

**Refactors (USER decisions)**

| Item | Status | Where |
|---|---|---|
| iOS top-level folder renames (`.pbxproj` = USER-only) | not scheduled | [`architecture/maintainability-roadmap.md`](architecture/maintainability-roadmap.md) § Not scheduled |
| Shared Swift package for macOS + iOS (open-source round 10) | not scheduled; USER adds the local package in Xcode | [`reports/2026-09-24-open-source-readiness-and-layout.md`](reports/2026-09-24-open-source-readiness-and-layout.md) § rounds |
| Residual platform twins: auto-space punctuation set ×4 (iOS / Android / macOS `AutoSpacePunctuation`, desktop `policies/auto_space.rs`), external-lookup digit-tone fold ×3 (`ExternalLookupURLBuilder` iOS / Android + desktop `engine/external_lookup.rs`; macOS copy deleted in macOS-over-desktop-core P1), source-bitmask decode ×3 (`LexiconBitmask` ×2, desktop `engine/lexicon.rs`), no-op `SuggestionCaseTransformer` ×2 (delete) | after macOS over `desktop-core` lands (removes the macOS copies); re-verify callers first — files confirmed on main 2026-10-02, callers not; one PR | [`reports/2026-09-30-audit-all.md`](reports/2026-09-30-audit-all.md) Appendix B |

**Project and legal (USER)**

| Item | Status | Where |
|---|---|---|
| SignPath Foundation code-signing application | rejected 2026-10-02; USER will reapply later — keep `CODE_SIGNING_POLICY.md` + `windows-build.yml` provenance build | [`go-public-checklist.md`](go-public-checklist.md), [`CODE_SIGNING_POLICY.md`](CODE_SIGNING_POLICY.md) |

**Not to re-propose** (USER-closed): `zh-TW` README, xcconfig signing, Android Gradle proto plugin, dictionary-source licensing follow-up, romanization spelling correction, corpus expansion P1b incl. the TAT application, a full TL / POJ / en / ja proofreading pass of the app UI (fix reported typos per incident instead), `$` sentence-start opener (its `association.bin` data is removed in its own PR), smart-suggestion techniques outside the bigram model (3, 5, 6, 9, 10, 11 incl. the Android `IME_FLAG_NO_PERSONALIZED_LEARNING` gap), Windows candidate-window paint latency and UIA exposure (USER 2026-10-02: "remove"); converted-romanization commit on Enter (`suann2ting3` → `suán-tìng`), TL mode (臺羅模式) tone key commits without a candidate window, and mobile predictions after Space (USER 2026-10-02: "remove"); bigram P6 walker term / P7 hanji-only sources / D7 two-word context; e2e drivers for macOS, Windows, Android, iOS; invariant-label PR2; naming batch C (persisted names stay frozen).

## Per-round gates (process invariants, project-wide)

Apply to every change regardless of release:

- Cross-platform parity-correction rounds merge both platforms in lockstep.
- iOS `pbxproj` is user-only (`docs/contributing/ios-guidelines.md`); Android Gradle is editable.
- Engine slices require S0 golden-diff EMPTY acceptance.

---

## Closed phases / shipped audits

- **Maintainability audit follow-up** — COMPLETE 2026-10-01: R1–R12 + docs drift MERGED (#274–#331). PR table: [`architecture/maintainability-roadmap.md`](architecture/maintainability-roadmap.md); source audit: [`reports/2026-09-30-audit-all.md`](reports/2026-09-30-audit-all.md).
- **Bigram language model** — CLOSED 2026-09-30 (USER, after the Android dogfood): P0–P5 MERGED (#267, #268, #270–#272) + the punctuation-context fix #273; P6 not opened, P7 not adopted. `association.bin` v2 word keys shipped in mobile v3.6.11. Design + status: [`architecture/bigram-lm-roadmap.md`](architecture/bigram-lm-roadmap.md).
- **User data in the engine** — P0–P9d MERGED 2026-09-26 (#219–#237): the four user-data SQLite stores are engine-owned (`engine/userdata`). Design + PR table: [`architecture/user-data-engine-roadmap.md`](architecture/user-data-engine-roadmap.md). Device dogfood pending (iOS first look OK).
- **Mobile custom-theme color roles** — P0–P7 MERGED 2026-09-27 (#252, #255, #257, #258; P3 dropped, premise false). Tiers + rollout rules: [`ui/theme.md`](ui/theme.md) § Custom Theme Color Roles.
- **Identical desktop menus; Linux update check added then removed** — phases 1–5 MERGED 2026-09-25 (#175–#178, site #20); Linux half reversed the same day (#193, no update check on Linux). S77. Design: [`reports/2026-09-26-shipped-roadmap-design-notes.md`](reports/2026-09-26-shipped-roadmap-design-notes.md) § Linux update check.
- **Learned phrases** — #109–#113 MERGED 2026-09-20; own store PR-A–D #125–#128 MERGED 2026-09-21; store engine-owned since user-data P3c. S62. Design: [`reports/2026-09-26-shipped-roadmap-design-notes.md`](reports/2026-09-26-shipped-roadmap-design-notes.md) § Learned phrases.
- **Mobile custom theme — one background surface, gradient direction, photo background** — A–D #90–#93 MERGED 2026-09-20 (+ follow-up E). S54 / S55. Design: [`reports/2026-09-26-shipped-roadmap-design-notes.md`](reports/2026-09-26-shipped-roadmap-design-notes.md) § Mobile custom theme; current state [`ui/theme.md`](ui/theme.md).
- **Desktop Telex keys for tone 1 / 4** — #98 `17850f17` MERGED 2026-09-19. S57. Design: [`reports/2026-09-26-shipped-roadmap-design-notes.md`](reports/2026-09-26-shipped-roadmap-design-notes.md) § Desktop Telex keys.
- **Desktop input-source menu — global shortcut rows** — #100 `f763d8bf` MERGED 2026-09-20. S59. Notes: [`reports/2026-09-26-shipped-roadmap-design-notes.md`](reports/2026-09-26-shipped-roadmap-design-notes.md) § Desktop input-source menu.
- **Desktop 3.6.8 items** — custom fonts #16, installed typefaces #45 (S42 / S43 PASS), Telex keys + candidate-window toggle #17–#22, symbol picker #26–#28, composing caret #29–#31, ⇧ + slot key #35, Shortcuts pane #36; shipped in desktop v3.6.8. Dogfood S30–S38. Design: [`reports/desktop-3.6.x-design-notes.md`](reports/desktop-3.6.x-design-notes.md).
- **kautian subcollections** (accent + Surname Appendix toggles + pronunciation-difference word-level extension) — 5 phases MERGED, shipped **v3.6.0** (#354-#358).
- **v3.6.1 user-data key consistency across input modes** — CLOSED / shipped: rounds R1–R7 MERGED 2026-06-03/04 (#382–#388; association recall + canonical-TL commit + custom words cross-mode + Android cap parity + `(hanji, tl)` frequency key + SQLite hygiene + backup exclusion). Dogfood items S11–S16. Triple index kept. The user-data stores later moved into the engine (see User data in the engine above); the canonical-TL key contract lives in `architecture/behavioral-invariants.md`.
- **App UI i18n — multi-language** (Hanji / English / Japanese / Tâi-lô / Pe̍h-ōe-jī + Automatic) — SHIPPED; all five display languages in the production picker. No standing proofreading pass (USER 2026-10-02): reported typos are fixed per incident. Outcome: `docs/contributing/i18n.md`, `behavioral-invariants.md` §37–39, `system-overview.md` §3 (`make i18n`).
- **Keyboard theme picker** (swipe gallery + custom theme) — SHIPPED v3.6.2: iOS #400-411, Android port #412-#418. Current-state reference: [`docs/ui/theme.md`](ui/theme.md).
- **Android UI modernization** (Compose M3 chrome/overlay) — DONE 2026-05-30 (#362 / #364 / #365). 3 leaf overlays (Symbol/Layout/Candidate) View→Compose M3 over `KeyboardChromeColors`; keys stay custom-draw; `InputView`/window kept View (IME-dismiss bug zone). Memory `project_android_compose_modernization.md`.
- **v3.5.9 D = TPS tri-index** — SHIPPED, tagged `3c8bec16` 2026-05-29. `tps:` FST family parallel to `tl:` / `poj:`; mode-axis (Input + Key + FST) now three-layer symmetric. Retired `is_tps` short-circuit (`dispatch.rs`/`continuous.rs`), `tps_or_mapped_to_er` runtime branch (`search.rs`), `tps_to_tl` canonicalize chain (`classification.rs`). 6 PR (#334-#340, C-0/C-1/C-3a/C-3b/C-4/C-5).
- Roadmap Item 1 (Project Structure & File Naming Cleanup) — CLOSED 2026-05-06 (#212-#215).
- Roadmap Item 4 (Android UI Compose migration) — CLOSED 2026-05-08 (#227-#231).
- v3.5.8 continuous input — SHIPPED 2026-05-20 (`61df3028`). See [`docs/releases/v3.5.8/plan.md`](releases/v3.5.8/plan.md) for full plan + Phase status + design rationale + dogfood matrix.

<!-- New active items go in Active / In-flight items. New deferred items go in Out of scope / deferred. Shipped versions get a row in Released versions index + an entry in Closed phases. -->
