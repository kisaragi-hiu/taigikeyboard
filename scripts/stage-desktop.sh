#!/usr/bin/env bash
# Stage the three desktop installers on this version's draft release: the
# package built here, the Windows installer and the Linux .deb built on
# GitHub-hosted runners.
#
# Usage: stage-desktop.sh   (no options — a release is both halves or neither)
#
# The two builds cannot share a machine — one needs Xcode and a Developer ID,
# the other MSVC and Inno Setup — so this runs the first here and dispatches the
# second to CI, then waits for it. Both build from this commit.
#
# Nothing here reaches a user: both halves stage on a DRAFT release
# (`docs/architecture/desktop-release.md`). Publishing stays a person's.
#
# Every run stages BOTH installers from one commit, and starts from a clean
# draft: an existing one for this version is deleted first (USER 2026-09-10 —
# 「我希望重複release的過程是原子性的,每一次都從新的開始建置」;
# 2026-09-11 — 「我不希望有--skip-macos或是skip-windows,我希望一次就是兩個一起建立」).
#
# There is deliberately no way to stage one half. Both escapes existed for a
# box that was off or a half that failed, and both were how a draft ended up
# holding two installers built from different commits — the exact thing the
# tag is supposed to describe. Re-running the whole thing is the recovery.
#

set -euo pipefail

fail() {
    echo "error: $*" >&2
    exit 1
}

REPOSITORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

[[ $# -eq 0 ]] || {
    echo "error: stage-desktop.sh takes no arguments" >&2
    echo "A release is both installers from one commit; there is no half of one." >&2
    exit 2
}

SOURCE_COMMIT="$(git -C "$REPOSITORY_DIR" rev-parse HEAD)"
[[ -z "$(git -C "$REPOSITORY_DIR" status --porcelain --ignore-submodules=none)" ]] ||
    fail "the working tree is dirty — both halves refuse it, and the box would be put on a commit that is not what is here"
git -C "$REPOSITORY_DIR" fetch --quiet origin main
git -C "$REPOSITORY_DIR" merge-base --is-ancestor "$SOURCE_COMMIT" FETCH_HEAD ||
    fail "HEAD (${SOURCE_COMMIT:0:7}) is not on origin/main — push it first; the box can only fetch what origin has"

DESKTOP_VERSION="$(awk '
    /^\[workspace\.package\]/ { inside = 1; next }
    inside && /^\[/ { exit }
    inside && /^version *=/ { gsub(/[" ]/, "", $3); print $3; exit }
' "$REPOSITORY_DIR/windows/Cargo.toml" | tr -d '\r')"
DESKTOP_TAG="desktop-$DESKTOP_VERSION"
RELEASE_REPOSITORY="taigikeyboard/taigikeyboard"
# What CI is expected to leave on the draft. Checked at the end, because a run
# that reports success and attaches nothing is exactly what happened the first
# time this script drove a second machine.
WINDOWS_ASSET_NAME="TaigiKeyboard-$DESKTOP_VERSION.exe"
LINUX_ASSET_NAME="taigikeyboard_${DESKTOP_VERSION}_amd64.deb"

echo "==> Staging $DESKTOP_TAG from ${SOURCE_COMMIT:0:7}"

# A draft for this version is the previous attempt; it goes, so this run builds
# both halves from one commit. A PUBLISHED release cannot be re-cut — its
# installers are downloadable and its tag is what people already have.
existing_state="$(gh release view "$DESKTOP_TAG" --repo "$RELEASE_REPOSITORY" --json isDraft --jq '.isDraft|tostring' 2>&1)" || existing_state=""
case "$existing_state" in
    true)
        echo "  removing the previous draft — every run stages both halves afresh"
        gh release delete "$DESKTOP_TAG" --repo "$RELEASE_REPOSITORY" --yes ||
            fail "cannot remove the existing draft $DESKTOP_TAG"
        ;;
    false)
        fail "$DESKTOP_TAG is already published — it cannot be re-cut. Bump to the next version (make version-desktop) and stage that"
        ;;
    *)
        [[ "$existing_state" == *"release not found"* || "$existing_state" == *"HTTP 404"* || -z "$existing_state" ]] ||
            fail "cannot read $DESKTOP_TAG in $RELEASE_REPOSITORY: $existing_state"
        ;;
esac

echo ""
echo "==> macOS — build, sign, notarize, stage (this Mac)"
make -C "$REPOSITORY_DIR" macos-release

# The installer and the package are built by `.github/workflows/windows-build.yml`
# and `linux-build.yml` on GitHub-hosted runners, not on the maintainer's box.
# That box is a development machine: its dev TIP is registered from the build
# tree, so a release build has to link over a DLL something still has mapped,
# and its App Control policy blocks freshly built binaries outright. Both are
# properties of that machine rather than of the release, and a clean runner has
# neither; the Linux half has no machine of its own at all.
#
# workflow_dispatch takes a branch or tag, never a bare SHA, so the commit being
# staged has to be what `main` points at — which it is, since the release prep
# was just pushed there.
git -C "$REPOSITORY_DIR" fetch --quiet origin main
[[ "$SOURCE_COMMIT" == "$(git -C "$REPOSITORY_DIR" rev-parse FETCH_HEAD)" ]] ||
    fail "HEAD (${SOURCE_COMMIT:0:7}) is not origin/main's tip — the hosted builds run from main, so push this commit first"

# Dispatches `$1` (a workflow file) on main and waits for the run it started.
# `gh workflow run` returns nothing that identifies the run, so the one built
# from this commit and started after the dispatch is looked up.
dispatch_and_wait() {
    local workflow="$1" what="$2" run_id="" dispatched_at
    echo ""
    echo "==> $what — GitHub-hosted runner"
    dispatched_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    gh workflow run "$workflow" --repo "$RELEASE_REPOSITORY" --ref main ||
        fail "cannot dispatch the $what build"
    echo "  waiting for the run to appear"
    for _ in $(seq 1 30); do
        run_id="$(gh run list --repo "$RELEASE_REPOSITORY" --workflow "$workflow" \
            --json databaseId,headSha,createdAt,event \
            --jq "[.[] | select(.headSha == \"$SOURCE_COMMIT\" and .event == \"workflow_dispatch\" and .createdAt >= \"$dispatched_at\")] | first | .databaseId // empty")"
        [[ -n "$run_id" ]] && break
        sleep 5
    done
    [[ -n "$run_id" ]] ||
        fail "the dispatched $what run never appeared — check $RELEASE_REPOSITORY's Actions tab"
    echo "  https://github.com/$RELEASE_REPOSITORY/actions/runs/$run_id"
    gh run watch "$run_id" --repo "$RELEASE_REPOSITORY" --exit-status ||
        fail "the $what build failed — its log is at the URL above; fix it and run this again, which re-stages every half"
}

dispatch_and_wait windows-build.yml Windows
dispatch_and_wait linux-build.yml Linux

# The draft's own page, from the API: a draft has no tag, so its URL is not the
# `releases/tag/<tag>` address a published release has. It is where the
# maintainer downloads what was staged and, when it passes, presses Publish.
draft_json="$(gh release view "$DESKTOP_TAG" \
    --repo taigikeyboard/taigikeyboard --json url,assets 2> /dev/null || true)"
DRAFT_URL="$(printf '%s' "$draft_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["url"])' 2> /dev/null || true)"
for asset in "$WINDOWS_ASSET_NAME" "$LINUX_ASSET_NAME"; do
    printf '%s' "$draft_json" |
        python3 -c 'import json,sys; sys.exit(0 if sys.argv[1] in [a["name"] for a in json.load(sys.stdin)["assets"]] else 1)' \
            "$asset" 2> /dev/null ||
        fail "a hosted run reported success but $asset is not on the draft — read its log above, then run this again"
done

echo ""
echo "✓ all three installers staged on the draft for ${SOURCE_COMMIT:0:7}"
echo ""
echo "  Open the draft, download the three assets, install and test them:"
echo "    ${DRAFT_URL:-https://github.com/taigikeyboard/taigikeyboard/releases}"
echo ""
echo "  When they pass, press \"Publish release\" on that page. That is the whole"
echo "  remaining step: it tags the commit and announces the release itself."
