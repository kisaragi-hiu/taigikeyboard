# Desktop-shared crates

The pure Rust the Windows (`../windows`, TSF) and Linux (`../linux`, IBus) input
methods both link, over the shared engine in `../engine`. One workspace, two
crates, no OS handle anywhere — everything here builds and tests natively on
the maintainer's Mac.

| Crate | Role |
|---|---|
| `crates/taigi-desktop-core` | Settings model + revision, engine bridge (prost envelope → `dispatch`), composing orchestration (the macOS `ComposingManager` port), key classifier + shortcuts, candidate geometry, symbol table, generated UI strings. `unsafe_code = forbid`, no C deps. |
| `crates/taigi-desktop-storage` | rusqlite user stores (frequency v2 / association v6 / custom dictionary v4 / learned phrases v1), the `settings.json` file store (atomic replace, revision), CSV codec. |

Behaviour oracle is the macOS input method (`../macos`); ported items cite the
Swift they mirror. Born as `taigi-windows-{core,storage}` in
`docs/architecture/windows-roadmap.md` (W1–W17), moved here for Linux
(`docs/architecture/linux-roadmap.md` L2).

```sh
make check   # i18n check + tests + clippy + fmt --check (from desktop/)
```
