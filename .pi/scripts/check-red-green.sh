#!/usr/bin/env bash
# ============================================================================
# check-red-green.sh — a changed test must fail on the base commit (class C8)
#
# A green test proves nothing until it has been observed red. This script takes
# the test files a branch changed, plants them onto the base commit in a
# throwaway worktree, and requires the run to FAIL there. If it passes on the
# base, the test encodes the behaviour it was written to catch — or never
# exercised the fix.
#
# Usage:
#   bash .pi/scripts/check-red-green.sh <base-ref> <test-file>... -- <test-filter> [--test <target>]
#
# Example (the #1082 C2 demonstration):
#   bash .pi/scripts/check-red-green.sh origin/main engine/tests/red_green_probe.rs \
#     -- git_read --test red_green_probe
#
# Exit codes: 0 = the test is red on the base (good), 1 = it is green on the base
# (the test does not cover the change), 2 = usage / setup failure.
# ============================================================================
set -uo pipefail

if [ $# -lt 4 ]; then
    sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
fi

BASE_REF="$1"; shift
FILES=()
while [ $# -gt 0 ] && [ "$1" != "--" ]; do FILES+=("$1"); shift; done
[ "${1:-}" = "--" ] && shift
FILTER_ARGS=("$@")

if [ ${#FILES[@]} -eq 0 ] || [ ${#FILTER_ARGS[@]} -eq 0 ]; then
    echo "usage: check-red-green.sh <base-ref> <test-file>... -- <test-filter> [--test <target>]" >&2
    exit 2
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORKTREE="$(mktemp -d)/base"
CHANGED=0

cleanup() { git -C "$REPO_ROOT" worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "── planting ${#FILES[@]} test file(s) on ${BASE_REF} ──"
git -C "$REPO_ROOT" worktree add --detach "$WORKTREE" "$BASE_REF" >/dev/null || {
    echo "❌ cannot create a worktree at ${BASE_REF}" >&2; exit 2; }

for f in "${FILES[@]}"; do
    dest="$WORKTREE/$f"
    mkdir -p "$(dirname "$dest")"
    if ! cp "$REPO_ROOT/$f" "$dest"; then
        echo "❌ cannot plant $f" >&2; exit 2
    fi
    CHANGED=1
done

if [ "$CHANGED" -eq 0 ]; then
    echo "❌ nothing planted" >&2; exit 2
fi

echo "── running on ${BASE_REF}: cargo test ${FILTER_ARGS[*]} ──"
(
    cd "$WORKTREE" || exit 2
    # Share the target dir so the run reuses dependency artifacts.
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}" cargo test "${FILTER_ARGS[@]}"
) >/tmp/red-green.log 2>&1
STATUS=$?

if [ "$STATUS" -eq 0 ]; then
    echo "❌ GREEN on ${BASE_REF} — this test does not cover the change (class C8)"
    tail -20 /tmp/red-green.log
    exit 1
fi

echo "✅ RED on ${BASE_REF} — the test fails without the fix, so it can defend it"
grep -E "^test result|^error|panicked at" /tmp/red-green.log | head -5
exit 0
