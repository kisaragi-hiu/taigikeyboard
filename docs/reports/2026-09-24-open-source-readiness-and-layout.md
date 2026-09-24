# Open-source readiness, best practices and repository layout — audit

Status: **audit only, nothing decided, no round open** (USER 2026-09-24: 「我也想做最佳實踐評估,也review專案資料夾結構,比起新功能,我可能會先針對專案重構,確保遵循open source慣例,其他人可以更容易使用這個repo」).
Companion to `2026-09-24-mobile-smart-suggestions-brainstorm.md` (engine / mobile code-level audit lives there). No release scope implied.

Sources: two read-only passes on `cb1cec63` (new-contributor / community-health audit; folder-structure / build-layout review). Items marked **verified** were re-checked by hand.

---

## 1. Summary

- Community files (CoC, SECURITY, CONTRIBUTING, NOTICE, Dependabot, SHA-pinned CI, fork PRs run without secrets) are above average; GitHub community profile 87 % (issue templates missing).
- The weak side is **onboarding**: the only build doc is `CLAUDE.md`, `make build` is macOS-only all-or-nothing, toolchain pins live on the maintainer's machine.
- **Privacy leaks in the public tree** (verified): a former work email written out in `docs/go-public-checklist.md:76`; GCP project ID + personal Gmail label layout in `.claude/skills/bug-triage/SKILL.md:14,19`; Discord guild / channel IDs in `.claude/skills/discord-triage/`. Redacting fixes the tree; history keeps them unless rewritten (USER decision).
- Layout: the costly problems are **CI coverage and docs**, not directory placement. Moving platforms under `apps/` costs pbxproj + CI paths + 100+ doc refs for little gain; khiin-rs (closest analogue) keeps platforms top-level too.

## 2. Findings — community health and onboarding

| Sev | Location | Finding | Fix | Effort |
|---|---|---|---|---|
| High | `docs/go-public-checklist.md:76`; `.claude/skills/bug-triage/`, `.claude/skills/discord-triage/` | Personal identifiers published (verified) | Redact; move maintainer-ops skills (bug-triage, discord-triage, arguably release-*) to the private dotfiles repo; gitignore `discord-triage/state.json` | S |
| High | `CONTRIBUTING.md:40-43` → `CLAUDE.md:45-66` | Only build doc is `CLAUDE.md`: hardcoded simulator UUID, `~/.claude/rules` links, no prerequisites, no `--recurse-submodules` | `docs/BUILDING.md` (prereqs, bootstrap, per-platform build + test), linked from README | M |
| High | `Makefile:23-37` | `make build` = default goal, always runs xcframework / SwiftPM steps → fails on Linux / Windows; Android contributors cannot get `jniLibs` via make (`.so` gitignored) | Split `protos` / `android-libs` / `ios-libs` / `macos-libs`; `.DEFAULT_GOAL := help`; group help as Contributor / Maintainer | M |
| High | `engine/scripts/gen-platform-protos.sh:30-62`, `android/app/build.gradle.kts:253` | protoc must be exactly 36.0; only hint is `brew install protobuf` (latest). No `mise.toml` / `.tool-versions` in repo (verified) | Commit `mise.toml` pinning protoc 36.0, swiftformat, gitleaks, node, python (match CI pins) | S |
| Med | `CONTRIBUTING.md:4,35`, `SECURITY.md:23`, `README.md:5-8,25`, `Makefile:268`, GitHub topics / labels | Linux missing or undercounted ("four platforms"); README lists `.deb` only | Wording pass; `linux` label + topic | S |
| Med | `README.md:65` vs `dictionary/LICENSE` | README says two sources are NonCommercial; LICENSE says only `taijit` (`kungge` NC covers images) | Align README | S |
| Med | `AGENTS.md` (symlink → `CLAUDE.md`), ~40 `~/.claude/rules` refs in `.claude/rules`, `docs/`, `Makefile:43`, source comments | Agent file = maintainer's private workflow (USER quotes, Codex sandwich, memory); unreadable rules; symlink breaks with `core.symlinks=false` on Windows | McBopomofo / vChewing pattern: neutral canonical `AGENTS.md` + thin `CLAUDE.md` pointer; style guides → `docs/contributing/`; drop `~/.claude` refs from source | M |
| Med | `.github/ISSUE_TEMPLATE/` missing; `.github/pull_request_template.md:11-13` | No issue forms; PR template asks for "round metadata / memory entry", test plan lists iOS / Android / engine only | `bug_report.yml` (platform, version, keystrokes, expected vs actual), `feature_request.yml`, `config.yml` (Discord, advisories); per-platform PR checklist | S |
| Med | `ios/TaigiKeyboard.xcodeproj/project.pbxproj` (`DEVELOPMENT_TEAM` ×8) | Contributor must dirty the pbxproj to sign | xcconfig + untracked `Local.xcconfig` override (maintainer-owned edit) | M |
| Med | `engine/rust-toolchain.toml:5-18` | armv7 missing from `targets` though `abiFilters` + `build-android-libs.sh` build it; comment stale | Add `armv7-linux-androideabi`, fix comment; document `RUSTUP_TOOLCHAIN=stable` for engine-only work | S |
| Med | `linux/README.md` | No system packages listed (only in `linux-e2e.yml:43-48`) | apt / dnf / pacman lines | S |
| Med | `engine.yml`, `android.yml`, `linux-e2e.yml:39,61,88` | clippy `-D warnings` + `spotlessCheck` not in CI; `linux-e2e.yml` uses tag-pinned actions against the SHA-pin policy | Add jobs; pin by SHA | S |
| Med | `.githooks/pre-commit:3-5` | Stale "no CI net" comment | Fix; non-brew gitleaks hints | S |
| Low | pack 809 MiB | Heavy first clone (legacy `dictionary.db`, old `.a` / `.so`) | Document `git clone --filter=blob:none --recurse-submodules` | S |
| Low | `THIRD_PARTY_LICENSES.md` | Linux / desktop crates (zbus, gtk4-rs, libadwaita-rs) and Fcitx5 (LGPL) linking missing; README "FlorisBoard-derived" implies Apache derivative | Add rows (consider `cargo about`); reword "patterned after" | M |
| Low | `dictionaries/` | No pointer to non-commercial status | `dictionaries/README.md` → `dictionary/LICENSE` | S |
| Low | `SECURITY.md:5` | Email only; GitHub private vulnerability reporting is enabled | Add the link | S |
| Low | `skills-lock.json` | Lists personal skills not in repo | Remove / gitignore | S |
| Low | `.gitmodules` corpus | `--recursive` pulls author-restricted text no build uses | `update = none`, opt-in init documented | S |
| Low | top level | English-only README for a Taiwanese audience (trime `README_tc`, vChewing `README-CHS`) | `README.zh-TW.md` | M |
| Low | root | No root `.editorconfig`; `.markdownlint.json` unenforced | Add both or drop config | S |

### Dictionary licensing position (from `docs/go-public-checklist.md` §4)

Code Apache-2.0; fonts OFL-1.1; the merged dictionary is **non-commercial** because `taijit` (CC BY-NC-SA 3.0 TW) cannot be separated from the one index. Open: four sources with no identified licence (`khpoo`, `lkk`, `khiin` frequency data, `char_freq_merged.txt`); ShareAlike compatibility of CC BY-SA 4.0 + CC BY-NC-SA 3.0 TW in one index not analysed; `kautian` / `kungge` rest on a 著作權法 §10-1 reading, not a grant. A fork may redistribute the code freely; a commercial fork cannot ship the built dictionary.

### Fresh-clone walkthrough (blockers in order)

1. ~810 MB clone; no `--recurse-submodules` hint → `taigi-converter/` empty.
2. README → `docs/README.md` has no build section; CONTRIBUTING → `CLAUDE.md`.
3. Bare `make` runs `build` → `protoc` / `protoc-gen-swift` not found.
4. `brew install protobuf` → rejected by the 36.0 gate; version not stated anywhere.
5. Linux / Windows: `make build` can never pass; Android needs `cargo-ndk`, NDK r25+, armv7 target found by hand.
6. Engine-only `cargo test` downloads Apple std libs; `corpus_total_freq_matches_dictionary_csv` red since 2026-08-29 (CI skips, `make test` does not).
7. Linux: missing GTK 4 / libadwaita / Fcitx5 headers / cmake, none listed.
8. iOS: signing fails on the maintainer team; fix dirties pbxproj.
9. `make dict`: Node unstated; `requirements.txt` unpinned (verified).
10. PR: template asks for private round metadata; no issue forms; lint gaps unflagged by CI.

## 3. Findings — layout and build

| Dir | Files | Size | Purpose |
|---|---|---|---|
| `engine/` | 166 | 2.5M | Rust workspace, 11 crates, protos, xcframework / `.so` scripts |
| `desktop/` | 75 | 1.0M | Rust workspace, 2 crates shared by Windows + Linux |
| `windows/` | 98 | 1.2M | Rust workspace, 4 crates, Inno Setup |
| `linux/` | 69 | 620K | Rust workspace, 5 crates, Fcitx5 C++ addon, packaging |
| `ios/` | 492 | 12M | Xcode project (user-only pbxproj) |
| `macos/` | 226 | 2.9M | SwiftPM package, no pbxproj |
| `android/` | 765 | 15M | single `:app`; 279 of 476 Java files are generated protos |
| `dictionary/` | 118 | 102M | Python pipeline, raw sources, `output/dictionary.csv` |
| `dictionaries/` | 4 | 23M | shipped `.bin` / `.fst` |
| `fonts/` `symbols/` `i18n/` `knowledge/` | | | bundled fonts; desktop symbols JSON; UI strings; phonetics spec |
| `tools/` `scripts/` `e2e/` | | | dev tools; release shell scripts; e2e data (drivers in `tools/e2e/`) |
| `taigi-emojis/` `corpus/` `taigi-converter/` | | | emoji generator; test sentences (submodule); converter (submodule) |

Scripts live in six places: `scripts/`, `tools/`, `engine/scripts`, `dictionary/tools`, `{macos,windows}/scripts`, `android/tools`.

| Sev | Path | Issue | Fix | Churn | pbxproj |
|---|---|---|---|---|---|
| High | `.github/dependabot.yml`, `security.yml` | Only `/engine` + `/windows` Cargo covered (verified); `desktop/` + `linux/` lockfiles (93 + 203 packages) get no Dependabot / `cargo audit`; `make fmt` / `make lint` engine-only | Cover all four workspaces | S | no |
| High | `README.md`, `CONTRIBUTING.md` | Architecture table lists 9 of ~22 top-level dirs | Full "Repository layout" table | S | no |
| High | `dictionary/build/common.py:33` | `build_ts = int(time.time())` embedded in `dictionary.bin` / `association.bin` headers (verified) — why both differ every build; engine never reads it | `SOURCE_DATE_EPOCH` or hash of `dictionary.csv` → reproducible bins | S | no |
| Med | `dictionary/` vs `dictionaries/` | One-letter difference between pipeline and shipped output | Shipped output → `assets/dictionary/` | M | yes |
| Med | 4 workspaces / 4 `Cargo.lock` / 4 `target/` | Engine compiled 4×; shared deps synced by hand; MSRV 1.86 (engine) vs 1.95 elsewhere, 1.86 untested | Root workspace (spike first) or shared `build.target-dir` | L | no |
| Med | `android/.../engine/proto/` (279 files, 3.1 MB); 6 byte-identical `.pb.swift` iOS / macOS | Generated code committed, no `linguist-generated`, no proto freshness CI (i18n has one) | `.gitattributes` marks + freshness job; optionally protobuf Gradle plugin | S / M | no |
| Med | iOS vs macOS Swift | 20 same-named diverged files (`RustEngineBridge+*`, `ComposingManager`, `DictionarySearchService`, …) | Shared local SwiftPM package (azooKey `AzooKeyCore/`, vChewing `Packages/`); macOS first | L | yes (iOS) |
| Med | Python (73 files) | Unpinned deps; `dictionary/tests`, `tools/release_notes_test.py`, `tools/windows/tests` not in CI | `pyproject.toml` + uv (like `taigi-emojis/`); run in `checks.yml` | S | no |
| Low | `engine/Makefile.toml` | Duplicates root Makefile; stale "no CI" comment | Delete | S | no |
| Low | `scripts/` vs `tools/`; `e2e/` vs `tools/e2e/` | Split homes | `scripts/` → `tools/release/`; e2e drivers → `e2e/` | S | no |
| Low | `knowledge/` | Doc outside `docs/` (27 referencing files) | `docs/phonetics/` | M | no |
| Low | `dictionary/` internals | `build/` package shadows PyPA `build`; duplicate basenames `common/X.py` vs `common/stages/X.py`; two `supplementary/` | Rename `build/` → `compile/` | M | no |
| Low | crate names | `protos`, `dispatch` next to `taigi-*`; `swift-ffi` + `android-jni` share `[lib] name = "rust_taigi"` (breaks `cargo doc --workspace`) | `taigi-` prefix if workspaces merge | M | no |

### Proposed target tree (platforms stay top-level)

```
Cargo.toml / Cargo.lock              # phase 7, after spike
engine/  desktop/
ios/ macos/ android/ windows/ linux/
apple/TaigiEngineKit/                # phase 8: shared SwiftPM package
assets/{dictionary,fonts,symbols}/   # was dictionaries/, fonts/, symbols/
dictionary/                          # pipeline + sources only
i18n/  taigi-emojis/  corpus/  taigi-converter/
e2e/{scenarios,drivers,analyzer,budgets.json}
tools/{release,i18n,desktop,windows}/
docs/{…,phonetics/}
```

### Leave as is

Android `com.siansiansu.taigikeyboard` (Play Store identity); single `:app` module; committed `.pb.swift` (generation needs pbxproj edits — rely on a freshness check); committed raw source CSVs + `dictionary/output/dictionary.csv` (`version_snapshot.py` diffs against old tags); history size (rewrite out of scope); per-workspace toolchain files while workspaces stay separate; root `i18n/` + `changelog/`.

## 4. Draft sequence (unscheduled; one PR each)

| # | Round | Content | Size | User-gated part |
|---|---|---|---|---|
| 0 | Privacy scrub | redact email + skill identifiers; move maintainer-ops skills out; gitignore runtime state | S | whether to move skills; whether to rewrite history |
| 1 | Onboarding docs | `docs/BUILDING.md`, README layout table + Linux wording + licence fix, `mise.toml`, `linux/README` packages, armv7 toolchain fix, delete `engine/Makefile.toml`, stale comments | ~300 LOC | — |
| 2 | `.github` | issue forms, neutral PR template, SECURITY advisory link | ~150 | — |
| 3 | Makefile split | per-platform lib targets, `.DEFAULT_GOAL := help`, grouped help | ~150 | — |
| 4 | CI coverage | Dependabot + audit for `desktop/` + `linux/`, fmt / lint over four workspaces, clippy + spotless jobs, SHA-pin `linux-e2e.yml`, Python pins + tests in CI | ~200 | — |
| 5 | Generated code + reproducible dictionary | `linguist-generated`, proto freshness job, deterministic `build_ts` + one regenerated artifact commit | ~100 + artifacts | — |
| 6 | `AGENTS.md` inversion | neutral `AGENTS.md`, thin `CLAUDE.md`, style guides → `docs/contributing/`, drop `~/.claude` refs | M | how much maintainer process stays public |
| 7 | `git mv` pass | `scripts/` → `tools/release/`, e2e unify, `knowledge/` → `docs/phonetics/` | mechanical | — |
| 8 | Assets move | `dictionaries/` `fonts/` `symbols/` → `assets/` | M | USER fixes 2–3 pbxproj paths |
| 9 | Root Cargo workspace | spike, then merge or shared `target-dir` | L | — |
| 10 | Shared Swift package | behaviour-freeze refactor, macOS first | L | USER adds local package in Xcode |
| — | `zh-TW` README, xcconfig signing, Android Gradle proto plugin | optional | | xcconfig touches pbxproj |

The engine-level refactors (dead ops, `platformFallbackFilters`, prediction assembly) are in the companion report §3–4 and can interleave.
