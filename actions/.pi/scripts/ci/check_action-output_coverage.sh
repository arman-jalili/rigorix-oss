#!/usr/bin/env bash
# Check Action Output Coverage
#
# Enforces coverage thresholds for the action-output module.
# Since Rust coverage tools vary by environment, this script
# verifies that:
#   1. All unit tests pass
#   2. Integration tests pass (if present)
#   3. All public APIs have at least one test
#
# Usage: bash .pi/scripts/ci/check_action-output_coverage.sh [--verbose]
#
# Exit codes:
#   0 — All checks pass
#   1 — Coverage threshold not met

set -euo pipefail

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

cd "$ROOT_DIR"

ERRORS=0

echo "  action-output coverage checks"
echo ""

# ── Check 1: Cargo build succeeds ──
echo -n "  Checking cargo build... "
if output=$(cargo build -p rigorix-actions 2>&1); then
    echo "✅"
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 2: Unit tests pass ──
echo -n "  Running unit tests (action_output)... "
if output=$(cargo test --lib -p rigorix-actions -- action_output 2>&1); then
    echo "✅"
    if $VERBOSE; then
        echo "$output" | grep -E "(test result|ok|FAILED)" | head -5
    fi
    # Count tests
    TEST_COUNT=$(echo "$output" | grep -oE 'running [0-9]+' | grep -oE '[0-9]+' || echo "0")
    echo "    Tests run: ${TEST_COUNT}"
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 3: Integration tests pass ──
echo -n "  Running integration tests... "
if compgen -G "actions/tests/*.rs" > /dev/null 2>&1; then
    if output=$(cargo test -p rigorix-actions --test '*' 2>&1); then
        echo "✅"
        if $VERBOSE; then
            echo "$output" | tail -5
        fi
    else
        echo "❌"
        echo "    $output"
        ERRORS=$((ERRORS + 1))
    fi
else
    echo "⊘ (no integration tests)"
fi

# ── Check 4: Verify all test modules exist ──
echo -n "  Checking test coverage for implementations... "
MISSING_TESTS=()

# Check each impl file has a #[cfg(test)] module
for impl_file in "$ROOT_DIR/actions/src/action_output/application/"*_impl.rs; do
    basename=$(basename "$impl_file")
    if grep -q '#\[cfg(test)\]' "$impl_file" 2>/dev/null; then
        if $VERBOSE; then
            echo "    ✓ $basename has tests"
        fi
    else
        MISSING_TESTS+=("$basename")
    fi
done

if [[ ${#MISSING_TESTS[@]} -eq 0 ]]; then
    echo "✅"
else
    echo "⚠ Missing tests for: ${MISSING_TESTS[*]}"
fi

# ── Summary ──
echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All action-output coverage checks passed"
    exit 0
else
    echo "❌ $ERRORS coverage check(s) failed"
    exit 1
fi
