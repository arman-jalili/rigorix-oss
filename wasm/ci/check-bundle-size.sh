#!/usr/bin/env bash
# check-bundle-size.sh — NFR-002 / criterion 23: measure, then gate at +25%.
#
# Two modes, and the difference matters:
#
#   shipped  — a wasm-bindgen artifact exists (pkg/*.wasm). The budget is enforced:
#              measured_bytes and budget_bytes are read from ci/bundle-budget.txt and a
#              regression beyond +25% fails the build.
#   awaiting — there is no shipped artifact yet (packaging pending). The gate records the
#              release cdylib size and says it is not enforcing a ceiling. It does NOT
#              silently pass: it prints the state and the reason, and it refuses to treat
#              a zero-byte or absent artifact as "within budget".
#
# Fail-closed in both modes: a measurement that cannot be taken is an error, and a budget
# file that is missing or unparseable is an error.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUDGET_FILE="$CRATE_DIR/ci/bundle-budget.txt"
SHIPPED="$CRATE_DIR/pkg/rigorix_gate_wasm_bg.wasm"
cd "$CRATE_DIR"

# The target directory belongs to the *workspace*, not to this crate: since the crate became
# a member (ADR-011) cargo writes to <repo>/target. Ask cargo rather than guessing, so the
# gate reads the same artifact the build produced — a gate pointing at a path that cannot
# exist fails for the wrong reason, which reads like a finding.
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 2>/dev/null \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -1)"
if [ -z "${TARGET_DIR:-}" ]; then
    echo "❌ could not resolve the target directory (cargo metadata failed)" >&2
    echo "   a gate that cannot find what it measures does not pass" >&2
    exit 1
fi
RAW="$TARGET_DIR/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm"

if [ ! -f "$BUDGET_FILE" ]; then
    echo "❌ no budget file: a size gate without a budget is an opinion" >&2
    exit 1
fi

read_field() {
    sed -nE "s/^$1:[[:space:]]*//p" "$BUDGET_FILE" | head -1
}

MEASURED=$(read_field measured_bytes)
BUDGET=$(read_field budget_bytes)
if [ -z "${MEASURED:-}" ] || [ -z "${BUDGET:-}" ]; then
    echo "❌ the budget file is unparseable (measured_bytes / budget_bytes)" >&2
    exit 1
fi

if [ -f "$SHIPPED" ]; then
    ACTUAL=$(wc -c <"$SHIPPED" | tr -d ' ')
    echo "== shipped artifact: $(basename "$SHIPPED") =="
    echo "   measured ${MEASURED} B · budget ${BUDGET} B · actual ${ACTUAL} B"
    if [ "${BUDGET:-0}" -eq 0 ]; then
        echo "❌ a shipped artifact exists but no budget was set: measure it first" >&2
        exit 1
    fi
    if [ "$ACTUAL" -gt "$BUDGET" ]; then
        echo "❌ over budget by $((ACTUAL - BUDGET)) B (limit = measured + 25%)" >&2
        exit 1
    fi
    echo "✅ within budget ($((BUDGET - ACTUAL)) B headroom)"
    exit 0
fi

echo "== no shipped artifact yet (packaging pending) =="
echo "   expected at pkg/$(basename "$SHIPPED")"
if [ ! -f "$RAW" ]; then
    echo "   building the release cdylib to record the trend..."
    if ! cargo build --release --target wasm32-unknown-unknown --quiet; then
        echo "❌ the release cdylib does not build, so nothing can be measured" >&2
        exit 1
    fi
fi
RAW_SIZE=$(wc -c <"$RAW" | tr -d ' ')
echo "   release cdylib: ${RAW_SIZE} B (no exports yet — not a shippable number)"
echo "   state: $(read_field state)"
if [ "$RAW_SIZE" -lt 512 ]; then
    echo "   ⚠️  the module exports nothing yet, so this size is not a budget subject."
    echo "      The ceiling starts at the first shipped artifact; from then on this gate"
    echo "      fails on any regression beyond +25%."
fi
echo
echo "Bundle size: trend recorded, ceiling not yet in force (by design, and stated)."
