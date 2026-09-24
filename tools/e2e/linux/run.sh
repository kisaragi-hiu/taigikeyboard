#!/usr/bin/env bash
# `make e2e PLATFORM=linux`: sync this tree to a Linux VM, build the
# test-mode IME into a private prefix there (the VM's own install is never
# touched), drive every scenario through Fcitx5 and IBus, and bring the run
# back. Usage: run.sh <run-dir> [scenario-id]
#
# The VM: TAIGI_E2E_LINUX_HOST (default the macOS UTM guest, memory
# reference_linux_dogfood_vms). Unreachable → every scenario is `skipped`,
# never a failure. Needs on the VM: the linux/ build dependencies, Rust,
# xvfb, xdotool, python3-gi + GTK 3, fcitx5-frontend-gtk3, ibus-gtk3.
set -euo pipefail

RUN_DIR="${1:?usage: run.sh <run-dir> [scenario-id]}"
ONLY="${2:-}"
HOST="${TAIGI_E2E_LINUX_HOST:-binhian@192.168.64.2}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
REMOTE=taigi-e2e
FRAMEWORKS=(fcitx5 ibus)

mkdir -p "$RUN_DIR"
if ! ssh -o ConnectTimeout=5 -o BatchMode=yes "$HOST" true 2>/dev/null; then
    for framework in "${FRAMEWORKS[@]}"; do
        python3 "$REPO_ROOT/tools/e2e/linux/driver.py" --framework "$framework" \
            --out "$RUN_DIR" --skip "Linux VM $HOST unreachable"
    done
    exit 0
fi

ssh "$HOST" mkdir -p "$REMOTE/src"
# Only what `make -C linux install` reads; `target/` stays on the VM so the
# next run builds incrementally.
(cd "$REPO_ROOT" && rsync -a --delete --relative --exclude 'target/' --exclude '.git' --exclude 'e2e/runs/' \
    engine desktop linux dictionaries i18n fonts symbols tools e2e "$HOST:$REMOTE/src/")

ssh "$HOST" bash -s -- "$ONLY" "${FRAMEWORKS[@]}" <<EOF
set -euo pipefail
only="\$1"; shift
source "\$HOME/.cargo/env"
cd "\$HOME/$REMOTE"
rm -rf run prefix
make -s -C src/linux install E2E=1 PREFIX="\$HOME/$REMOTE/prefix" > build.log 2>&1 \
    || { tail -30 build.log; exit 1; }
for framework in "\$@"; do
    xvfb-run -a dbus-run-session -- python3 src/tools/e2e/linux/driver.py \
        --framework "\$framework" --prefix "\$HOME/$REMOTE/prefix" --out "\$HOME/$REMOTE/run" \
        \${only:+--only "\$only"}
done
EOF

rsync -a "$HOST:$REMOTE/run/" "$RUN_DIR/"
