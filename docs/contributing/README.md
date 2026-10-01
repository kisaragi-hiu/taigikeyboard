# Contributing guides

Per-platform style guides and cross-cutting rules. Read the one that covers the files you change; [`AGENTS.md`](../../AGENTS.md) lists which to read before what. Repository-wide basics (licensing, build, commits) are in [`CONTRIBUTING.md`](../../CONTRIBUTING.md).

| Guide | Covers |
|---|---|
| [`known-pitfalls.md`](known-pitfalls.md) | Rules distilled from past incidents — read once before your first change |
| [`cross-platform-alignment.md`](cross-platform-alignment.md) | Refactor behaviour-freeze, parity tiers, shared-core candidates, `CROSS-PLATFORM INVARIANT` comments |
| [`phonetics.md`](phonetics.md) | TL / POJ / TPS schema and canonical-form work — authoritative sources only |
| [`i18n.md`](i18n.md) | Shared `i18n/*.json` keys |
| [`doc-lookup.md`](doc-lookup.md) | Verifying a framework / OS API against current docs before calling it |
| [`security-rules.md`](security-rules.md) | Logging, SQL binding, exported components — all platforms |
| [`rust-best-practices.md`](rust-best-practices.md) | Engine workspace, errors, crates, tests |
| [`rust-ffi-safety.md`](rust-ffi-safety.md) | Swift / JNI / dispatch FFI boundary |
| [`rust-migration-policy.md`](rust-migration-policy.md) | Moving platform logic into the engine |
| [`ios-guidelines.md`](ios-guidelines.md) | Day-to-day iOS rules |
| [`ios-architecture.md`](ios-architecture.md) | iOS structure and layering |
| [`ios-settings-injection.md`](ios-settings-injection.md) | `EngineSettingsProvider` wiring |
| [`ios-shared-core-candidates.md`](ios-shared-core-candidates.md) | iOS files eligible for cross-platform extraction |
| [`ui-style-guide.md`](ui-style-guide.md) | App / settings UI, both mobile platforms |
| [`android-guidelines.md`](android-guidelines.md) | Android architecture, Kotlin, DI, DataStore, Gradle |
| [`android-ime-patterns.md`](android-ime-patterns.md) | Compose, IME patterns, testing |
| [`windows-guidelines.md`](windows-guidelines.md) | Windows TSF |
| [`linux-guidelines.md`](linux-guidelines.md) | Linux Fcitx5 / IBus and `desktop/` |
