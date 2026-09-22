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

.PHONY: help i18n-check test lint check-target check check-cpp fmt build build-fcitx5 install install-fcitx5 uninstall component run-engine run-settings clean

help:
	@echo "Linux input method — dev loop"
	@echo ""
	@echo "  make check         - The gate: i18n check + native tests + clippy + cross build + fmt --check"
	@echo "  make test          - Native tests for the workspace"
	@echo "  make lint          - clippy -D warnings, all targets, native"
	@echo "  make check-target  - Build the engine for $(LINUX_TARGET) (cargo zigbuild on macOS)"
	@echo "  make check-cpp     - Syntax-check the Fcitx5 addon against ../references/fcitx5 headers (macOS host; skipped when the clone is absent)"
	@echo "  make i18n-check    - Fail if the generated i18n Rust is stale"
	@echo "  make fmt           - cargo fmt"
	@echo "  make build         - (Linux) cargo build --release (IBus engine + the FFI static library)"
	@echo "  make build-fcitx5  - (Linux) the Fcitx5 addon: cmake over the FFI static library (needs fcitx5-modules-dev, extra-cmake-modules)"
	@echo "  make install       - (Linux) install both shells + dictionaries into PREFIX=$(PREFIX) (DESTDIR honoured)"
	@echo "  make uninstall     - (Linux) remove what install put there; user data stays"
	@echo "  make run-engine    - (Linux) run the IBus engine from this tree against a running ibus-daemon (RUST_LOG=debug, repo dictionaries)"
	@echo "  make run-settings  - Open the settings window from this tree (macOS host too: GTK + libadwaita from Homebrew); PANE=about picks a pane"

# The generated strings live in the desktop-shared crate; a stale copy would
# compile strings that no longer match their source. Same gate the macOS,
# Windows and desktop Makefiles run.
i18n-check:
	python3 ../tools/i18n/check.py

test: i18n-check
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

# The IBus engine and the FFI static library for the shipping target. A real
# build, not `cargo check`: the bundled SQLite's build script compiles C for
# the target, which needs a C toolchain for it — zig's, on macOS.
check-target:
	$(CROSS_CARGO) -p taigikeyboard-ibus -p taigi-linux-ffi --target $(LINUX_TARGET)

check: test lint check-target check-cpp
	cargo fmt -- --check

# The addon's C++ cannot link on the Mac, but it can be parsed: clang in
# -fsyntax-only mode over the fcitx5 headers of the version Ubuntu 24.04
# ships (`git clone --depth 1 --branch 5.1.7 https://github.com/fcitx/fcitx5
# ../references/fcitx5`; gitignored). The three `*_export.h` CMake would
# generate are stubbed. Catches what CI catches five minutes later —
# a typo, a macro absent from 5.1.7 — never a link or runtime fault.
FCITX5_HEADERS := ../references/fcitx5/src/lib
FCITX5_STUBS := target/fcitx5-stubs
check-cpp:
	@if [ ! -d "$(FCITX5_HEADERS)" ]; then echo "check-cpp: no ../references/fcitx5 clone, skipped"; exit 0; fi; \
	mkdir -p $(FCITX5_STUBS); \
	for m in FCITXCORE FCITXUTILS FCITXCONFIG; do \
	  f=$(FCITX5_STUBS)/$$(echo $$m | tr A-Z a-z)_export.h; \
	  printf '#pragma once\n#define %s_EXPORT\n#define %s_DEPRECATED\n#define %s_DEPRECATED_EXPORT\n#define %s_NO_EXPORT\n' $$m $$m $$m $$m > $$f; \
	done; \
	clang++ -std=c++17 -fsyntax-only -Wall -Wextra -DFCITX_GETTEXT_DOMAIN='"fcitx5-taigikeyboard"' -DTAIGIKEYBOARD_VERSION='"0"' \
	  -I $(FCITX5_HEADERS) -I $(FCITX5_STUBS) -I crates/taigi-linux-ffi/include fcitx5/src/engine.cpp && echo "check-cpp: ok"

# Plain `cargo fmt`, never `--all`: `--all` also formats path dependencies,
# i.e. the engine and desktop crates.
fmt:
	cargo fmt

build:
	cargo build --release

# The Fcitx5 addon (linux/fcitx5/): CMake against Fcitx5Core, over the static
# library `build` produced. PREFIX must match the binaries' TAIGIKEYBOARD_PREFIX.
FCITX5_BUILD_DIR := target/fcitx5-build
build-fcitx5: build
	cmake -S fcitx5 -B $(FCITX5_BUILD_DIR) -DCMAKE_BUILD_TYPE=Release \
		-DCMAKE_INSTALL_PREFIX=$(PREFIX) \
		-DTAIGI_FFI_LIB=$(CURDIR)/target/release/libtaigi_linux_ffi.a
	cmake --build $(FCITX5_BUILD_DIR) --parallel

# The component XML with this PREFIX rendered in (`<exec>` / `<setup>`).
component: data/taigikeyboard.xml.in
	sed -e 's#@PREFIX@#$(PREFIX)#g' -e 's#@VERSION@#$(shell grep -m1 '^version' Cargo.toml | cut -d'"' -f2)#g' $< > target/taigikeyboard.xml

# The install layout the engine resolves at runtime
# (`taigi-linux-platform::paths::InstallLayout`); the PREFIX must match the
# one the binaries were built with (`TAIGIKEYBOARD_PREFIX`, default /usr).
install: build component install-fcitx5
	install -Dm755 target/release/ibus-engine-taigikeyboard $(DESTDIR)$(PREFIX)/libexec/ibus-engine-taigikeyboard
	install -Dm755 target/release/taigikeyboard-settings $(DESTDIR)$(PREFIX)/bin/taigikeyboard-settings
	install -Dm644 target/taigikeyboard.xml $(DESTDIR)$(PREFIX)/share/ibus/component/taigikeyboard.xml
	install -d $(DESTDIR)$(PREFIX)/share/taigikeyboard/dictionaries
	install -m644 ../dictionaries/dictionary.fst ../dictionaries/dictionary.bin ../dictionaries/association.bin ../dictionaries/syllables.fst $(DESTDIR)$(PREFIX)/share/taigikeyboard/dictionaries/
	@echo "installed under $(DESTDIR)$(PREFIX); restart fcitx5 (fcitx5 -r) and/or ibus (ibus restart)"

# The addon's .so + its two .conf files, through CMake's own install rules.
install-fcitx5: build-fcitx5
	DESTDIR=$(DESTDIR) cmake --install $(FCITX5_BUILD_DIR)

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/libexec/ibus-engine-taigikeyboard
	rm -f $(DESTDIR)$(PREFIX)/bin/taigikeyboard-settings
	rm -f $(DESTDIR)$(PREFIX)/lib/fcitx5/taigikeyboard.so $(DESTDIR)$(PREFIX)/lib/*/fcitx5/taigikeyboard.so
	rm -f $(DESTDIR)$(PREFIX)/share/fcitx5/inputmethod/taigikeyboard.conf $(DESTDIR)$(PREFIX)/share/fcitx5/addon/taigikeyboard.conf
	rm -f $(DESTDIR)$(PREFIX)/share/ibus/component/taigikeyboard.xml
	rm -rf $(DESTDIR)$(PREFIX)/share/taigikeyboard
	@echo "removed; ~/.config/taigikeyboard and ~/.local/share/taigikeyboard are kept"

# The dev loop on a Linux box with ibus-daemon up: the engine from this tree,
# reading the repo's dictionaries, logging every key. `ibus engine
# taigikeyboard` from another terminal selects it (the component XML must
# already be installed once so the daemon knows the name).
run-engine:
	TAIGIKEYBOARD_DATA_DIR=$(CURDIR)/.. RUST_LOG=debug cargo run -p taigikeyboard-ibus -- --ibus

# The settings window from this tree — the visual smoke the Windows window
# never had on the host (roadmap L3). Writes the real ~/.config/taigikeyboard/settings.json.
PANE ?=
run-settings:
	RUST_LOG=debug cargo run -p taigikeyboard-settings -- $(if $(PANE),--pane $(PANE),)

clean:
	cargo clean
