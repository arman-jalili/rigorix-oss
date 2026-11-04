#!/usr/bin/env bash
# check-bundle-size.sh — NFR-002 / criterion 23: measure, then gate at +25%.
#
# Two modes, and the difference matters:
#
#   shipped  — a shipped artifact exists (pkg/*.wasm). Two assertions, both required:
#              (1) it carries no `name` custom section — size alone cannot prove the
#                  strip ran, since an unstripped 243 366 B file sits under a 262 351 B
#                  budget (#1353); (2) measured_bytes and budget_bytes, read from
#                  ci/bundle-budget.txt, are not exceeded by +25%.
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
CEILING=$(read_field ceiling_bytes)
if [ -z "${MEASURED:-}" ] || [ -z "${BUDGET:-}" ] || [ -z "${CEILING:-}" ]; then
    echo "❌ the budget file is unparseable (measured_bytes / budget_bytes / ceiling_bytes)" >&2
    exit 1
fi

STRIP="$CRATE_DIR/ci/strip_name_section.py"

if [ -f "$SHIPPED" ]; then
    # #1353: the presence assertion comes first and is the load-bearing one. The subject
    # of this gate is the artifact a host ships, and a host ships no debug symbols; the
    # size check alone would pass on an unstripped file because unstripped < budget.
    echo "== shipped artifact: $(basename "$SHIPPED") =="
    # Prove the parser can see a `name` section before trusting it to say there is none
    # (`.pi/scripts/validate-release-train.sh` sets the same precedent).
    if ! python3 "$STRIP" --self-test; then
        echo "❌ the strip helper cannot detect a `name` section, so its verdict means nothing" >&2
        exit 1
    fi
    if ! python3 "$STRIP" --check "$SHIPPED"; then
        echo "❌ the artifact under measurement still carries its debug section" >&2
        echo "   the packaging step (ci/run-module-smoke.sh) must strip it before this gate" >&2
        exit 1
    fi
    ACTUAL=$(wc -c <"$SHIPPED" | tr -d ' ')
    echo "   measured ${MEASURED} B · budget ${BUDGET} B · ceiling ${CEILING} B · actual ${ACTUAL} B"
    # #1386: `measured_bytes` claims to *be* the measurement, so the artifact must match it.
    # Without this, a stale floor is invisible: the budget below is derived from a number
    # nobody checked, and it drifts with every rebuild that changes size.
    #
    # #1405: but a byte-*exact* floor is a per-machine fact, not a cross-machine one. Same
    # commit, same pinned toolchain (1.96), same `wasm-bindgen` (0.2.130): 1337205 B on macOS,
    # 1335752 B on the Linux runner — 1453 B (0.11 %) of absolute paths baked into panic
    # messages. The exact assertion refused a correct artifact on every CI run, and a gate that
    # is always red hides the ones that are sometimes red. So the comparison carries a
    # declared tolerance, and the *property* #1386 wanted is intact: a stale floor is caught
    # by an order of magnitude (169250 → 1337205 is +690 %), and 1 % is far below any change
    # worth a re-record. `budget_bytes` and `ceiling_bytes` stay hard, exact comparisons.
    MEASUREMENT_TOLERANCE_PERCENT=1
    DELTA=$(( ACTUAL > MEASURED ? ACTUAL - MEASURED : MEASURED - ACTUAL ))
    ALLOWED_DELTA=$(( MEASURED / 100 * MEASUREMENT_TOLERANCE_PERCENT ))
    if [ "$DELTA" -gt "$ALLOWED_DELTA" ]; then
        echo "❌ the recorded measurement is not this artifact: actual ${ACTUAL} B, recorded ${MEASURED} B" >&2
        echo "   delta ${DELTA} B exceeds the ${MEASUREMENT_TOLERANCE_PERCENT} % build-environment tolerance (${ALLOWED_DELTA} B)" >&2
        echo "   re-record measured_bytes from the artifact this build produced, then derive budget" >&2
        exit 1
    fi
    if [ "$DELTA" -gt 0 ]; then
        echo "   recorded measurement differs by ${DELTA} B — build environment, within tolerance"
    fi
    if [ "${BUDGET:-0}" -eq 0 ]; then
        echo "❌ a shipped artifact exists but no budget was set: measure it first" >&2
        exit 1
    fi
    if [ "$ACTUAL" -gt "$BUDGET" ]; then
        echo "❌ over budget by $((ACTUAL - BUDGET)) B (limit = measured + 25%)" >&2
        exit 1
    fi
    # The +25 % ratchet is derived; the ceiling is the number the specification fixed, and a
    # raised ratchet must never carry an artifact past it (#1386).
    if [ "$ACTUAL" -gt "$CEILING" ]; then
        echo "❌ over the ${CEILING} B ceiling by $((ACTUAL - CEILING)) B" >&2
        exit 1
    fi
    echo "✅ within budget ($((BUDGET - ACTUAL)) B headroom)"
    exit 0
fi

echo "== no shipped artifact yet (packaging pending) =="
echo "   expected at pkg/$(basename "$SHIPPED")"
if [ ! -f "$RAW" ]; then
    echo "   building the release cdylib to record the trend..."
    if ! bash "$CRATE_DIR/ci/wasm-build.sh"; then
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
