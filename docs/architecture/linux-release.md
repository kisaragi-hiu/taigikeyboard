# Linux release — a `.deb` on the desktop draft, distro-managed after that

> **Type**: Reference (living)
> **Keywords**: `linux`, `release`, `deb`, `dpkg-deb`, `Fcitx5`, `IBus`, `desktop train`
> **Related**: desktop-release.md (the shared flow), linux-roadmap.md (L10 / L11), windows-release.md (the sibling half)

How the Linux half of a desktop version ships. The shared flow — one draft
release `desktop-<version>` for all three desktops, tested and published by a
person — is `desktop-release.md`; this file is only what the Linux artifact
is and how it is built.

## The artifact

`taigikeyboard_<version>_amd64.deb` (today: the CI artifact `linux-deb`; when
wired into the release flow, attached to the `desktop-<version>` draft beside
the macOS `.pkg` and the Windows `.exe`). One package holds both shells, the way `fcitx5-chewing` and
`ibus-chewing` come from one source:

| Path | What |
|---|---|
| `/usr/lib/<multiarch>/fcitx5/libtaigikeyboard.so` + `/usr/share/fcitx5/{addon,inputmethod}/taigikeyboard.conf` | The Fcitx5 addon (primary) and its registration (`Library=export:libtaigikeyboard` → that file) |
| `/usr/libexec/ibus-engine-taigikeyboard` + `/usr/share/ibus/component/taigikeyboard.xml` | The IBus engine (second) and its component registration |
| `/usr/bin/taigikeyboard-settings` + `/usr/share/applications/tw.taigikeyboard.Settings.desktop` + `/usr/share/icons/hicolor/*/apps/taigikeyboard.png` | The GTK 4 / libadwaita settings window, its launcher entry and icon |
| `/usr/share/taigikeyboard/dictionaries/*` | The dictionary artifacts the engine reads at first key |

`Depends: fcitx5 | ibus` plus what the three binaries link, versioned, from
`dpkg-shlibdeps` at pack time (`linux/packaging/control.in`, `@SHLIBS@`) — GTK
4.14, libadwaita 1.5, GLib 2.80 on Ubuntu 24.04, the runner that builds it.
Built and checked in CI on Ubuntu 24.04 (compile, `ibus-daemon` smoke, the
package's contents and the addon file name); installing and typing with it on
a real desktop is S74, not yet done. A `.deb` installed by hand adds no apt
source: a newer version is another download.

User data is never in the package: `~/.config/taigikeyboard/settings.json` and
`~/.local/share/taigikeyboard/*.db` survive `apt remove`.

## How it is built

`make -C linux deb` on a Linux machine (or the CI runner):

1. `make install PREFIX=/usr DESTDIR=target/deb/root` — the ONE install
   layout, so the package and a source install cannot drift (both shells,
   the registration files, the dictionaries, the desktop entry, the icons).
2. `packaging/control.in` rendered with the version from `linux/Cargo.toml`
   (moved by `make version-desktop x.y.z` with the other two desktops) and
   the `Depends` `dpkg-shlibdeps` computes over the settings window, the IBus
   engine and the Fcitx5 addon.
3. `dpkg-deb --build --root-owner-group`. No `cargo-deb`: it would carry a
   second copy of the asset list. No maintainer scripts: the hicolor icon
   cache is refreshed by the theme package's own dpkg trigger, and Fcitx5 /
   IBus are restarted in the user's session (`fcitx5 -r`, `ibus restart`),
   never from root.

The Fcitx5 addon is C++ over the Rust C ABI and compiles only on Linux
(`linux-roadmap.md` L1); the Mac only syntax-checks it (`make -C linux check-cpp`).
That is why the package is built on the GitHub-hosted runner, never on the
maintainer's Mac.

## Staging and publishing — not wired yet

USER 2026-09-23: 「先不用串release」. The package is built on every CI run
(`.github/workflows/linux-build.yml`: the `.deb` from the install layout,
its contents, the addon file name the `.conf` names, the desktop entry —
kept as the workflow artifact `linux-deb`) and by `make -C linux deb` on a
Linux machine. Nothing attaches it to a `desktop-<version>` draft, and
`scripts/stage-desktop.sh` stages the macOS and Windows halves only. When the
USER wires it in, the shape is the Windows one: a dispatch from `main` by
`stage-desktop.sh` (carrying the staged commit) attaches to the DRAFT, never
over an existing asset and never on a publish, from a job that alone can
write.

## No in-app update

Linux packages are updated by the package manager (`linux-roadmap.md` L10).
The 一般 pane shows the running version and a 去下載 link to taigikeyboard.tw;
the `update*` settings keys stay unwritten and `taigi-windows-update` is not
linked. The announcement (`scripts/announce-release.sh`) writes no Linux site
data yet — the landing page's Linux download is the release page itself.

## Installing by hand

```sh
sudo apt install ./taigikeyboard_<version>_amd64.deb
fcitx5 -r            # or: ibus restart
```

Then add 台語齒盤 in `fcitx5-configtool` (Fcitx5) or the desktop's input-source
settings (IBus). First-machine acceptance is the dogfood run-book in
`linux-roadmap.md` and `dogfood-checklist.md` S74. Uninstall: `sudo apt remove taigikeyboard`.
