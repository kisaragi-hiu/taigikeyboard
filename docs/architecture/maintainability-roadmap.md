# Maintainability Audit Follow-up — Roadmap

> **Type**: Planning (PR table over the 2026-09-30 audit)
> **Keywords**: `refactor`, `dead code`, `parity`, `desktop-core`, `userdata`, `commit resolution`, `test selection`, `naming`, `docs drift`
> **Status**: in progress — R1–R3(b) PR-B merged (#274–#285)
> **Source**: `docs/reports/2026-09-30-audit-all.md` (frozen snapshot on `30a79c16`; §2 findings, §4 draft rounds, Appendices A–F)
> **Session memory**: project memory `project_maintainability_audit_2026_09_30.md` (Claude auto-memory)

---

## Mandate

- USER 2026-09-30: "ok, go, follow your recommendations; after creating a PR, merge and continue until done" — the report's §5 recommendations are adopted as decisions, and each PR merges once its gates are green.
- USER 2026-09-30: "finish all rounds; affecting user settings is fine, just don't lose data". A round may delete, reset or normalise a stored **setting** without a migration (named as a behaviour change in its PR). User **data** — the engine `userdata` stores (custom dictionary, frequency, association, learned phrases), user-created themes, copied-in fonts, `.taigi` backups — must survive every round; a round that touches one carries a migration or a proof it is untouched, plus a test.
- Not covered by the mandate: release / version / tag decisions; `.pbxproj` edits (USER only); rounds that rest on an unconfirmed bug.

## PR table

| Round | Content | PRs | Status |
|---|---|---|---|
| R1 | Dead surface: dead wire ops, platform dead code, retired settings (desktop), Android legacy compat | #274 #275 #276 #277 | Merged |
| R1 PR4 | Legacy appearance keys on iOS + Android (`keyHeightScale`, `keyFontSizeScale`, `candidateTextSizeScale`, `keyCornerRadius`, `keyBorderWidth`, `colorSettings`) — delete; the default theme reads stock values. User themes untouched | 1 | Pending |
| R2 | Parity bugfixes: (a) Android Dictionary tab toggle + TPS search, (b) Android sentence-end pre-check, (c) `en` placeholders | #278 #280 #279 | Merged |
| R2d | iOS `CandidateCellHelper.tpsFallback` on hanji-less TPS cells | — | **Blocked** — unconfirmed, needs a USER on-device check |
| R3(a) | Settings windows reach user data through engine ops; retire `DeriveCustomQueryKey` | #281 #282 | Merged |
| R3(b) | Settings-page model: PR-A presentation + launch parser, PR-B `SettingsWriter`, PR-C custom-dictionary listing state, PR-D remaining twin label / roster helpers | #284 #285 · PR-C · PR-D | PR-C next |
| R3(c) | Win/Linux key-intent executor + `Runtime` → `desktop-core` | 1–2 | Pending |
| R4 | Engine user-data façade: `dispatch/src/user_data.rs` page logic → `userdata`; single-impl store traits; `cfg(not(user-data))` arms; one `CustomSearchKey` | 2 | Pending |
| R5 | Engine-owned commit resolution (`CommitContinuous` returns document text + auto-space verdict, records usage) | 3 (engine; mobile; macOS + desktop) | Pending |
| R6 | Config normalisation in the engine (flat settings snapshot, real `input_mode = "tps"`, engine-resolved sources) | 2–3 | Pending |
| R7 | Test redundancy (engine layers, platform restatements, test-local copies, shared `engine/test-support`) | 3 | Pending |
| R8 | Test selection: one integration binary per crate, `tools/test-select`, CI path filters, macOS `swift test` job | 2–3 | Pending |
| R9 | Naming batch A (identifiers, files, non-iOS folders) | 3–4 | Pending |
| R10 | Naming batch B (proto names; field numbers unchanged) | 1–2 | Pending |
| R11 | Lexicon / composing boundary (`SortKey` → `ranking`, one key-family module, visibility) | 2 | Pending |
| R12 | `Phase::Composing` removal — ≤20-line spike first | 2 | Pending |
| A | Docs drift (Appendix D, 57 rows) | admin lane | Pending |

Every PR runs its round type's pre-gate (`~/.claude/rules/round-workflow.md`): refactor = behaviour-freeze list + `refactor-reviewer`; bugfix = root cause + Codex agreement; feature = plan + Codex design pass.

## Not scheduled (USER decisions)

- **macOS over `desktop-core`** (report §5.2): ~4,000 Swift lines duplicate `desktop/crates`; the report recommends deciding after R3 lands.
- **Naming batch C** — persisted names (setting keys, DB columns, backup JSON, FST prefix, JNI symbol, package names): frozen per §5.6.
- **iOS top-level folder renames** (§5.8): `.pbxproj` edits are USER-only.

## Decisions recorded while running

- R3(b) PR-B: the three atomic writers (`SettingsFileStore::save`, Windows / Linux CSV export) stay separate — the Linux export creates its temp file exclusively so it follows no pre-planted symlink, and no unification keeps that.
- R3(b) PR-C: only state and rules move; which component owns the job slot differs per shell (Windows per page, Linux window-wide so an outcome survives a page rebuild) and stays there.
