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
FRAMEWORKS=(fcitx5 ibus)
source "$(dirname "${BASH_SOURCE[0]}")/remote.sh"

mkdir -p "$RUN_DIR"
if ! e2e_reachable ssh "$HOST"; then
    for framework in "${FRAMEWORKS[@]}"; do
        python3 "$E2E_REPO_ROOT/tools/e2e/linux/driver.py" --framework "$framework" \
            --out "$RUN_DIR" --skip "Linux VM $HOST unreachable"
    done
    exit 0
fi

e2e_sync ssh "$HOST"
e2e_build ssh "$HOST"
# ssh joins its arguments into one command line, so an empty one would
# vanish: `-` stands for "every scenario".
ssh "$HOST" bash -s -- "${ONLY:--}" "${FRAMEWORKS[@]}" <<EOF
set -euo pipefail
only="\$1"; shift
[ "\$only" = - ] && only=
cd "\$HOME/$E2E_REMOTE"
for framework in "\$@"; do
    xvfb-run -a dbus-run-session -- python3 src/tools/e2e/linux/driver.py \
        --framework "\$framework" --prefix "\$HOME/$E2E_REMOTE/prefix" --out "\$HOME/$E2E_REMOTE/run" \
        \${only:+--only "\$only"}
done
EOF

rsync -a "$HOST:$E2E_REMOTE/run/" "$RUN_DIR/"
