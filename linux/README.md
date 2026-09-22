# Linux input method

TaigiKeyboard for Linux: an IBus engine written in pure Rust over D-Bus (no
libibus), plus a GTK 4 / libadwaita settings window, over the desktop-shared
crates in `../desktop` and the engine in `../engine`. Design record and phase
table: `docs/architecture/linux-roadmap.md`.

## Layout

| Crate | Kind | Role |
|---|---|---|
| `crates/taigi-linux-platform` | lib, pure, host-testable | XDG paths + install prefix, IBus key event → `KeyEventSnapshot`, settings launcher, session locale. |
| `crates/taigikeyboard-ibus` | bin `ibus-engine-taigikeyboard` | The engine: bus discovery, `org.freedesktop.IBus.Factory` + `Engine` objects (zbus), the key path over the shared `ComposingManager`, preedit / commit / lookup-table signals, hand-serialised IBus wire types. |
| `crates/taigikeyboard-settings` (PR5–PR7) | bin `taigikeyboard-settings` | The settings window: 一般 / 外觀 / 快捷鍵 / 詞庫來源 / 自訂詞庫 / 關於 (+ unlisted 辭典搜尋). |
| `data/taigikeyboard.xml.in` | component XML | What ibus-daemon reads to know the engine exists (`make component` renders the prefix). |

Behaviour oracle is the macOS input method (`../macos`); the Rust it runs
on is the Windows port (`../windows`, `../desktop`). Deltas are named in the
roadmap's divergence table.

## Working without a Linux machine

```sh
make check          # from linux/: i18n check + native tests + clippy + cross build + fmt
```

Prerequisites on macOS: `brew install gtk4 libadwaita pkgconf zig cargo-zigbuild`,
`rustup target add x86_64-unknown-linux-gnu` (the toolchain file does it on
first use). `zbus` and gtk4-rs both build natively on the Mac, so every crate
is clippy-checked here; `make check-target` cross-builds the engine binary
for the shipping target through `cargo zigbuild`. These gates prove the code
compiles; they do not prove behaviour. The CI job
(`.github/workflows/linux-build.yml`) adds a real Ubuntu build, the test run
and an `ibus-daemon` smoke; the dogfood run-book in the roadmap owns the rest.

## On a Linux machine

```sh
make build                       # cargo build --release
sudo make install PREFIX=/usr    # binaries, component XML, dictionaries
ibus restart                     # or: ibus write-cache; ibus restart
```

Then add 台語齒盤 (language `nan`) in the desktop's input-source settings.
For a development tree, `TAIGIKEYBOARD_DATA_DIR=<repo root>` points the
engine at the repository's own `dictionaries/` without installing them;
`RUST_LOG=debug` on the engine process logs every key's intent.

User data: `~/.config/taigikeyboard/settings.json`,
`~/.local/share/taigikeyboard/{user_frequency,user_association,custom_dictionary,learned_phrases}.db`.
`make uninstall` leaves both directories alone.
