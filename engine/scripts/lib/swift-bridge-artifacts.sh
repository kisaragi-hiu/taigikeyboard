#!/usr/bin/env bash
# Swift-bridge artifact post-processing shared by the iOS and macOS xcframework
# builds. Both link the `engine/swift-ffi` bridge, so the header/modulemap
# layout and the generated Swift wrappers must be produced identically for
# both platforms — drift between them is a silent cross-platform bug. macOS
# links one more bridge into the same archive (`taigi-macos-ffi`'s
# `RustTaigiDesktop`), passed as an extra (out_dir, bridge_name) pair; iOS
# passes none.
#
# Sourced, never executed. Callers set `set -euo pipefail` and source by
# absolute path:
#
#   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#   source "$SCRIPT_DIR/lib/swift-bridge-artifacts.sh"
#
# Every function takes explicit absolute-path arguments; nothing here reads a
# caller global or the working directory. What stays in the platform entry
# scripts is the platform-shaped work: which triples to build, lipo/slice
# assembly, feature flags, staging strategy, and validation.
#
# `framework_name` is a parameter for readability, not for generality: it is
# interpolated into a `grep` pattern and an `awk` variable, so the contract is
# the repo's fixed `RustTaigi` module name, not an arbitrary string.

# Ask cargo which OUT_DIR belongs to one package's build script (`swift-ffi`,
# or `taigi-macos-ffi` on macOS) for a given configuration, by replaying the caller's exact cargo argv with
# `--message-format=json` and reading the `build-script-executed` record.
#
# Pass the SAME argv the build used — cargo keeps one `<package>-<fingerprint>/
# out` per feature set / config, and a differing flag would select (and build) a
# different fingerprint. Replaying identical flags is a cache hit, so the extra
# invocation costs a fraction of a second and compiles nothing.
#
# This replaces a "newest `out` directory by mtime" heuristic, which is wrong
# whenever the current configuration's build script did not re-run: after a
# `--dev` build, a subsequent cached release build leaves the panic-injector
# fingerprint newest, and the heuristic staged ITS headers/wrappers against the
# release archive. Observed on 2026-08-15 — mtime picked
# `swift-ffi-0db2fb3c…` while cargo reported `swift-ffi-887d0482…`. Harmless
# only because both fingerprints happened to emit an identical bridge surface.
#
# Usage: swift_bridge_find_out_dir <workspace_dir> <package> <cargo_arg>...
swift_bridge_find_out_dir() {
    local workspace_dir="$1" package="$2"
    shift 2
    local out_dir
    # `|| return` is load-bearing: the caller assigns this function's output via
    # command substitution, which clears `errexit` inside the subshell, so a
    # cargo or parser failure would otherwise be swallowed.
    out_dir="$(
        cd "$workspace_dir" \
        && cargo "$@" --message-format=json --quiet \
        | python3 -c '
import json, sys

package = sys.argv[1]

out_dirs = []
for line in sys.stdin:
    try:
        message = json.loads(line)
    except json.JSONDecodeError:
        continue
    # Cargo package-id format for a path dependency: `path+file:///…/swift-ffi#0.1.0`.
    # The leading slash keeps a hypothetical sibling `my-swift-ffi` from matching.
    # The directory is the package name for every bridge crate in this repo.
    # If a future cargo changes this format, no record matches and the run fails
    # loudly below — it never falls back to guessing.
    if message.get("reason") == "build-script-executed" and f"/{package}#" in message.get("package_id", ""):
        out_dirs.append(message["out_dir"])
unique = sorted(set(out_dirs))
if len(unique) != 1:
    sys.exit(f"expected exactly 1 {package} build-script-executed record, got {len(unique)}")
print(unique[0])
' "$package"
    )" || return $?
    if [[ -z "$out_dir" ]]; then
        echo "error: cargo reported no $package OUT_DIR for: cargo $*" >&2
        return 1
    fi
    printf '%s\n' "$out_dir"
}

# Every bridge's build script writes its own copy of the shared runtime
# (`SwiftBridgeCore.{h,swift}`); one module can hold it only once. Staging the
# first copy is safe only while the others are byte-identical — which they
# are when every bridge crate resolves the same swift-bridge version.
#
# Usage: _swift_bridge_assert_same_core <bridge_out_dir> [<extra_out_dir> <extra_bridge_name>]...
_swift_bridge_assert_same_core() {
    local bridge_out_dir="$1"
    shift
    if (($# % 2)); then
        echo "error: extra bridges come in (out_dir, bridge_name) pairs, got: $*" >&2
        return 1
    fi
    while (($#)); do
        local file
        for file in SwiftBridgeCore.h SwiftBridgeCore.swift; do
            if ! cmp -s "$bridge_out_dir/$file" "$1/$file"; then
                echo "error: $1/$file differs from $bridge_out_dir/$file — bridges built against different swift-bridge versions" >&2
                return 1
            fi
        done
        shift 2
    done
}

# Copy the C headers into `headers_dir` and write the modulemap that exposes
# them as the `<framework_name>` Clang module. swift-bridge nests per-bridge
# artefacts under `<OUT>/<bridge_name>/`; the first bridge is the engine's,
# named after the module:
#   <OUT>/SwiftBridgeCore.{h,swift}
#   <OUT>/<framework_name>/<framework_name>.{h,swift}
# Each extra (out_dir, bridge_name) pair adds `<bridge_name>.h` to the module.
#
# Usage: swift_bridge_stage_headers <bridge_out_dir> <headers_dir> <framework_name> [<extra_out_dir> <extra_bridge_name>]...
swift_bridge_stage_headers() {
    local bridge_out_dir="$1" headers_dir="$2" framework_name="$3"
    shift 3
    _swift_bridge_assert_same_core "$bridge_out_dir" "$@" || return
    mkdir -p "$headers_dir"
    cp "$bridge_out_dir/SwiftBridgeCore.h" "$headers_dir/"
    cp "$bridge_out_dir/$framework_name/$framework_name.h" "$headers_dir/"
    local header_lines="    header \"$framework_name.h\""
    while (($#)); do
        cp "$1/$2/$2.h" "$headers_dir/"
        header_lines+=$'\n'"    header \"$2.h\""
        shift 2
    done
    cat > "$headers_dir/module.modulemap" <<EOF
module $framework_name {
    header "SwiftBridgeCore.h"
$header_lines
    export *
}
EOF
}

# swift-bridge does not emit `import` statements; both wrappers reference C
# symbols defined in the xcframework's modulemap-exposed module. Inject the
# import at the top of each wrapper so any consumer target picks up the C
# surface without a project-level bridging header.
_swift_bridge_inject_import() {
    local file="$1" framework_name="$2"
    if ! grep -q "^import $framework_name" "$file"; then
        local tmp
        tmp=$(mktemp)
        if grep -q '^import Foundation' "$file"; then
            awk -v import_line="import $framework_name" \
                '{ print } /^import Foundation/ && !injected { print import_line; injected=1 }' \
                "$file" > "$tmp"
        else
            { echo "import $framework_name"; echo ""; cat "$file"; } > "$tmp"
        fi
        mv "$tmp" "$file"
    fi
}

# Copy the generated Swift wrappers — the shared runtime once, then one per
# bridge — into `out_dir` and apply both patches swift-bridge's output needs
# before it compiles cleanly. Extra (out_dir, bridge_name) pairs as in
# `swift_bridge_stage_headers`, which callers run first: it is what checks the
# pairs and that every bridge's shared runtime is the same.
#
# Usage: swift_bridge_stage_wrappers <bridge_out_dir> <out_dir> <framework_name> [<extra_out_dir> <extra_bridge_name>]...
swift_bridge_stage_wrappers() {
    local bridge_out_dir="$1" out_dir="$2" framework_name="$3"
    shift 3
    cp "$bridge_out_dir/SwiftBridgeCore.swift" "$out_dir/"
    cp "$bridge_out_dir/$framework_name/$framework_name.swift" "$out_dir/"

    _swift_bridge_inject_import "$out_dir/$framework_name.swift" "$framework_name"
    _swift_bridge_inject_import "$out_dir/SwiftBridgeCore.swift" "$framework_name"
    while (($#)); do
        cp "$1/$2/$2.swift" "$out_dir/"
        _swift_bridge_inject_import "$out_dir/$2.swift" "$framework_name"
        shift 2
    done

    # Swift 5.9+ warns when a module declares conformance of an imported type to
    # an imported protocol unless the conformance is annotated `@retroactive`.
    # swift-bridge does not yet emit the annotation; patch it in.
    sed -i '' \
        -e 's/^extension RustStr: Identifiable {$/extension RustStr: @retroactive Identifiable {/' \
        -e 's/^extension RustStr: Equatable {$/extension RustStr: @retroactive Equatable {/' \
        "$out_dir/SwiftBridgeCore.swift"
}
