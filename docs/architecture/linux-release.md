# Linux release — a `.deb` on the desktop draft, distro-managed after that

> **Type**: Reference (living)
> **Keywords**: `linux`, `release`, `deb`, `dpkg-deb`, `Fcitx5`, `IBus`, `desktop train`
> **Related**: desktop-release.md (the shared flow), linux-roadmap.md (L10 / L11), windows-release.md (the sibling half)

How the Linux half of a desktop version ships. The shared flow — one draft
release `desktop-<version>` for all three desktops, tested and published by a
person — is `desktop-release.md`; this file is only what the Linux artifact
is and how it is built.

## The artifact

`taigikeyboard_<version>_amd64.deb`, plus `taigikeyboard_<version>_amd64.deb.sha256`,
attached to the `desktop-<version>` draft beside the macOS `.pkg` and the
Windows `.exe`. One package holds both shells, the way `fcitx5-chewing` and
`ibus-chewing` come from one source:

| Path | What |
|---|---|
| `/usr/lib/<multiarch>/fcitx5/taigikeyboard.so` + `/usr/share/fcitx5/{addon,inputmethod}/taigikeyboard.conf` | The Fcitx5 addon (primary) and its registration |
| `/usr/libexec/ibus-engine-taigikeyboard` + `/usr/share/ibus/component/taigikeyboard.xml` | The IBus engine (second) and its component registration |
| `/usr/bin/taigikeyboard-settings` + `/usr/share/applications/tw.taigikeyboard.Settings.desktop` + `/usr/share/icons/hicolor/*/apps/taigikeyboard.png` | The GTK 4 / libadwaita settings window, its launcher entry and icon |
| `/usr/share/taigikeyboard/dictionaries/*` | The dictionary artifacts the engine reads at first key |

`Depends: fcitx5 | ibus, libgtk-4-1, libadwaita-1-0, libc6` (`linux/packaging/control.in`).
Built for and tested on Ubuntu 24.04 (GTK 4.14, libadwaita 1.5, fcitx5 5.1.7,
GLib 2.80); newer distributions are covered by the same dependencies.

User data is never in the package: `~/.config/taigikeyboard/settings.json` and
`~/.local/share/taigikeyboard/*.db` survive `apt remove`.

## How it is built

`make -C linux deb` on a Linux machine (or the CI runner):

1. `make install PREFIX=/usr DESTDIR=target/deb/root` — the ONE install
   layout, so the package and a source install cannot drift (both shells,
   the registration files, the dictionaries, the desktop entry, the icons).
2. `packaging/control.in` rendered with the version from `linux/Cargo.toml`
   (moved by `make version-desktop x.y.z` with the other two desktops).
3. `dpkg-deb --build --root-owner-group`. No `cargo-deb`: it would carry a
   second copy of the asset list.

The Fcitx5 addon is C++ over the Rust C ABI and compiles only on Linux
(`linux-roadmap.md` L1); the Mac only syntax-checks it (`make -C linux check-cpp`).
That is why the package is built on the GitHub-hosted runner, never on the
maintainer's Mac.

## Staging and publishing

`.github/workflows/linux-build.yml` is the Linux half of a desktop release,
the way `windows-build.yml` is the Windows half:

- On every pull request touching `linux/**` / `desktop/**` / `engine/**` it
  builds the package and keeps it as a workflow artifact (`linux-deb`), and
  checks that the expected paths are inside it. Nothing reaches a draft.
- `scripts/stage-desktop.sh` (`make desktop-release`) dispatches it on `main`
  after the Windows run and waits; the run attaches the `.deb` and its
  `.sha256` to the `desktop-<version>` draft the macOS half created. A
  dispatch from any other ref never attaches.
- A `desktop-<version>` **publish** also runs it and re-attaches, so a
  release always carries a package built from the tagged commit.

Publishing stays a person's (`desktop-release.md`). No signing: a `.deb`
downloaded from the project page is verified by its `.sha256`, as the unsigned
Windows channel is; an apt repository with its own key is outside this slice.

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
