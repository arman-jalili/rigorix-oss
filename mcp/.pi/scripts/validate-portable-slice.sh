#!/usr/bin/env bash
# ============================================================================
# validate-portable-slice.sh — the slice the pipeline compiles, compiled here
# ============================================================================
# #1626's witness (`gate::domain::Governed`) is produced by one function, and
# that function's only caller (`gate::infrastructure::assembly`) is native-only.
# Under `--features portable --target wasm32-unknown-unknown` with `-D warnings`
# the producer was therefore dead code, and `rigorix-engine` failed to build —
# in CI's `portable` job and in the `wasm gates` job, both. `local-ci` said
# 126/126 green because **it compiled no wasm32 target at all**: a local gate
# that measures a different subject than the pipeline is not a gate.
#
# This script's subject is exactly the pipeline's two differential steps:
#
#   * `cargo check -p rigorix-engine --features portable --target
#     wasm32-unknown-unknown` with `RUSTFLAGS=-D warnings` — the invocation
#     `.github/workflows/ci.yml` runs, warning policy included;
#   * `validate-portable-inventory.sh --gates` — a gate with no manifest row is
#     an exclusion nobody reviewed, and a row with no gate is a claim about a
#     build that still compiles the module.
#
# What it deliberately does not run: the pipeline's `cargo test --features
# portable --tests` (minutes, and the `lint` stage already runs the same tests
# natively) and `wasm/ci/check-bundle-independence.sh` (it computes the shipped
# crate's graph; `guards` runs it). Both are named here so the difference
# between "this script" and "the portable job" is a recorded decision.
#
# A missing target is a *skip with its reason*, not a pass: CI installs the
# target in the step before, so the skip can only happen on a machine that has
# not, and saying so is the honest thing to print.
#
# Usage:
#   validate-portable-slice.sh              # check (gating)
#   validate-portable-slice.sh --self-test  # prove the check bites
# ============================================================================
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

TARGET='wasm32-unknown-unknown'

# The pipeline's own invocation, so it cannot drift from the job it mirrors.
check_slice() {
    local root="$1"
    RUSTFLAGS="-D warnings" cargo check --locked -p rigorix-engine --features portable \
        --target "$TARGET" --manifest-path "$root/Cargo.toml" 2>&1
}

run() {
    local root="$1" failed=0

    if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
        printf '  %b⚠%b skipped: %s is not installed (`rustup target add %s`) — CI installs it, so this machine is not the pipeline\n' \
            "$YELLOW" "$NC" "$TARGET" "$TARGET"
    else
        local out status=0
        out="$(check_slice "$root")" || status=$?
        if [ "$status" -ne 0 ]; then
            printf '  %b❌%b the slice does not build for %s\n' "$RED" "$NC" "$TARGET"
            printf '%s\n' "$out" | grep -E '^(error|warning)' -A4 | head -40
            failed=$((failed + 1))
        else
            printf '  %b✅%b the slice builds for %s under -D warnings\n' "$GREEN" "$NC" "$TARGET"
        fi
    fi

    if ! bash "$root/.pi/scripts/validate-portable-inventory.sh" --gates >/tmp/portable-gates.out 2>&1; then
        printf '  %b❌%b a portable gate and the inventory disagree\n' "$RED" "$NC"
        sed 's/\x1b\[[0-9;]*m//g' /tmp/portable-gates.out | tail -20
        failed=$((failed + 1))
    else
        printf '  %b✅%b every portable gate has an inventory row\n' "$GREEN" "$NC"
    fi

    return "$failed"
}

if [ "${1:-}" = '--self-test' ]; then
    # Plant the defect this script exists for — a producer in the slice with no caller in it —
    # in a throwaway worktree, and assert both that the plant applied and that the check failed.
    # A mutation that does not apply is not a test, and a seed that cannot be found is inert.
    tmp="$(mktemp -d)"
    tree="$tmp/tree"
    ko=0
    cleanup() {
        git -C "$ROOT" worktree remove --force "$tree" >/dev/null 2>&1 || true
        find "$tmp" -mindepth 1 -delete 2>/dev/null || true
        rmdir "$tmp" 2>/dev/null || true
    }
    trap cleanup EXIT

    if ! git -C "$ROOT" worktree add --detach "$tree" HEAD >/dev/null 2>&1; then
        printf '  %b❌%b self-test could not create a worktree\n' "$RED" "$NC"
        printf 'self-test: 0/2 assertions\n'
        exit 1
    fi

    dom="$tree/engine/src/gate/domain/mod.rs"
    if ! grep -q '^pub mod governed;' "$dom"; then
        printf '  %b❌%b self-test seed inert: could not find the gate on `governed`\n' "$RED" "$NC"
        ko=$((ko + 1))
    else
        python3 - "$dom" <<'PY2'
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace('#[cfg(not(feature = "portable"))]\npub mod governed;', 'pub mod governed;')
open(p, 'w').write(s)
PY2
        if git -C "$tree" diff --quiet -- engine/src/gate/domain/mod.rs; then
            printf '  %b❌%b self-test seed did not apply: the copy is unchanged\n' "$RED" "$NC"
            ko=$((ko + 1))
        else
            out="$(check_slice "$tree")" || status=$?
            if printf '%s' "${out:-}" | grep -q 'never used'; then
                printf '  %b✅%b a producer with no caller in the slice fails the check (fails, as it must)\n' "$GREEN" "$NC"
            else
                printf '  %b❌%b a producer with no caller in the slice did not fail the check\n' "$RED" "$NC"
                printf '%s\n' "${out:-}" | grep -E '^error' -A3 | head -12
                ko=$((ko + 1))
            fi
        fi
    fi

    # A seed that asserts nothing is worse than no seed: the real tree must pass, or the
    # verdict above is measuring something other than the pipeline.
    if ! bash "$ROOT/.pi/scripts/validate-portable-inventory.sh" --gates >/dev/null 2>&1; then
        printf '  %b❌%b the real tree does not pass its own inventory check\n' "$RED" "$NC"
        ko=$((ko + 1))
    fi

    total=2
    printf 'self-test: %d/%d assertions\n' "$((total - ko))" "$total"
    exit $(( ko > 0 ? 1 : 0 ))
fi

failed=0
run "$ROOT" || failed=$?
exit $(( failed > 0 ? 1 : 0 ))