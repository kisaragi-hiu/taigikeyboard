# Sourced by tools/e2e/linux*/run.sh: put this tree on a Linux VM and build
# the test-mode IME there into ~/$E2E_REMOTE/prefix (the VM's own install is
# never touched). Each function takes the ssh command (one word-split string,
# e.g. "ssh -J win -p 2222") and the host.

E2E_REMOTE=taigi-e2e
E2E_REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

e2e_reachable() {
    $1 -o ConnectTimeout=5 -o BatchMode=yes "$2" true 2>/dev/null
}

e2e_sync() {
    local ssh_cmd=$1 host=$2
    # macOS openrsync ignores --delete under --relative: clear the small trees
    # whose deletions matter (a removed scenario must not keep running).
    $ssh_cmd "$host" "mkdir -p $E2E_REMOTE/src && rm -rf $E2E_REMOTE/src/e2e $E2E_REMOTE/src/tools"
    # Only what `make -C linux install` reads; `target/` stays on the VM so the
    # next run builds incrementally.
    (cd "$E2E_REPO_ROOT" && rsync -a -e "$ssh_cmd" --delete --relative --exclude 'target/' --exclude '.git' --exclude 'e2e/runs/' \
        engine desktop linux dictionaries i18n fonts symbols tools e2e "$host:$E2E_REMOTE/src/")
}

e2e_build() {
    $1 "$2" bash -s <<EOF
set -euo pipefail
source "\$HOME/.cargo/env"
cd "\$HOME/$E2E_REMOTE"
rm -rf run prefix
make -s -C src/linux install E2E=1 PREFIX="\$HOME/$E2E_REMOTE/prefix" > build.log 2>&1 \
    || { tail -30 build.log; exit 1; }
EOF
}
