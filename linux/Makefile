# Dev loop for the Linux input method. Two audiences:
#
#   * on the macOS host: `make check` is the whole gate — native tests +
#     clippy for the workspace (zbus and gtk4-rs both build on macOS; GTK and
#     libadwaita from Homebrew), a REAL cross build of the engine for the
#     shipping target through `cargo zigbuild` (zig ships the C toolchain
#     the bundled SQLite needs), `cargo fmt --check` and the i18n check
#     (docs/architecture/linux-roadmap.md L12). Compilation proof only,
#     never behaviour — the dogfood run-book owns that;
#   * on a Linux machine: `make build` produces the release binaries,
#     `make install` puts them where ibus-daemon finds them — see README.md.
#
# The repo-root `make linux-check` forwards here.

export PATH := $(HOME)/.cargo/bin:$(PATH)

LINUX_TARGET := x86_64-unknown-linux-gnu
# `cargo zigbuild` cross-links the engine from macOS; on a Linux host plain
# cargo does the same thing natively.
CROSS_CARGO := $(shell command -v cargo-zigbuild >/dev/null 2>&1 && echo "cargo zigbuild" || echo "cargo build")
# Where `make install` puts things; a package build passes DESTDIR.
PREFIX ?= /usr
DESTDIR ?=

.PHONY: help i18n-check test lint check-target check fmt build install uninstall component clean

help:
	@echo "Linux input method — dev loop"
	@echo ""
	@echo "  make check         - The gate: i18n check + native tests + clippy + cross build + fmt --check"
	@echo "  make test          - Native tests for the workspace"
	@echo "  make lint          - clippy -D warnings, all targets, native"
	@echo "  make check-target  - Build the engine for $(LINUX_TARGET) (cargo zigbuild on macOS)"
	@echo "  make i18n-check    - Fail if the generated i18n Rust is stale"
	@echo "  make fmt           - cargo fmt"
	@echo "  make build         - (Linux) cargo build --release"
	@echo "  make install       - (Linux) install into PREFIX=$(PREFIX) (DESTDIR honoured), then \`ibus restart\`"
	@echo "  make uninstall     - (Linux) remove what install put there; user data stays"

# The generated strings live in the desktop-shared crate; a stale copy would
# compile strings that no longer match their source. Same gate the macOS,
# Windows and desktop Makefiles run.
i18n-check:
	python3 ../tools/i18n/check.py

test: i18n-check
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

# The engine and its platform crate for the shipping target. A real build,
# not `cargo check`: the bundled SQLite's build script compiles C for the
# target, which needs a C toolchain for it — zig's, on macOS.
check-target:
	$(CROSS_CARGO) -p taigikeyboard-ibus --target $(LINUX_TARGET)

check: test lint check-target
	cargo fmt -- --check

# Plain `cargo fmt`, never `--all`: `--all` also formats path dependencies,
# i.e. the engine and desktop crates.
fmt:
	cargo fmt

build:
	cargo build --release

# The component XML with this PREFIX rendered in (`<exec>` / `<setup>`).
component: data/taigikeyboard.xml.in
	sed -e 's#@PREFIX@#$(PREFIX)#g' -e 's#@VERSION@#$(shell grep -m1 '^version' Cargo.toml | cut -d'"' -f2)#g' $< > target/taigikeyboard.xml

# The install layout the engine resolves at runtime
# (`taigi-linux-platform::paths::InstallLayout`); the PREFIX must match the
# one the binaries were built with (`TAIGIKEYBOARD_PREFIX`, default /usr).
install: build component
	install -Dm755 target/release/ibus-engine-taigikeyboard $(DESTDIR)$(PREFIX)/libexec/ibus-engine-taigikeyboard
	install -Dm644 target/taigikeyboard.xml $(DESTDIR)$(PREFIX)/share/ibus/component/taigikeyboard.xml
	install -d $(DESTDIR)$(PREFIX)/share/taigikeyboard/dictionaries
	install -m644 ../dictionaries/dictionary.fst ../dictionaries/dictionary.bin ../dictionaries/association.bin ../dictionaries/syllables.fst $(DESTDIR)$(PREFIX)/share/taigikeyboard/dictionaries/
	@echo "installed under $(DESTDIR)$(PREFIX); run: ibus restart (or ibus write-cache; ibus restart)"

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/libexec/ibus-engine-taigikeyboard
	rm -f $(DESTDIR)$(PREFIX)/share/ibus/component/taigikeyboard.xml
	rm -rf $(DESTDIR)$(PREFIX)/share/taigikeyboard
	@echo "removed; ~/.config/taigikeyboard and ~/.local/share/taigikeyboard are kept"

clean:
	cargo clean
