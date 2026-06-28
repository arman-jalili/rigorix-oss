#!/usr/bin/env bash
# ============================================================================
# check_recovery-recipes_coverage.sh
#
# Validates that the recovery-recipes module has sufficient test coverage.
# Enforces:
#   - Minimum total tests: 50
#   - Minimum unit tests: 40
#
# Uses `cargo test` output to count test functions.
#
# Usage: bash .pi/scripts/ci/check_recovery-recipes_coverage.sh [--help]
#
# Exit codes: 0 = coverage OK, 1 = below threshold
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Determine project root
if [ -f "$(cd "$PI_DIR/.." && pwd)/engine/Cargo.toml" ]; then
    PROJECT_DIR="$(cd "$PI_DIR/.." && pwd)/engine"
elif [ -f "$(cd "$PI_DIR/.." && pwd)/Cargo.toml" ]; then
    PROJECT_DIR="$(cd "$PI_DIR/.." && pwd)"
else
    echo "ERROR: Cargo.toml not found"
    exit 1
fi

MIN_TOTAL=50
MIN_UNIT=40

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Recovery Recipes Coverage Check ═══"
echo ""

# Count test functions in the module
TEST_COUNT=$(grep -rE '#[tokio::test]|#[test]' "$PROJECT_DIR/src/recovery_recipes/" 2>/dev/null | wc -l)
UNIT_COUNT=$(grep -rE '#[tokio::test]|#[test]' "$PROJECT_DIR/src/recovery_recipes/" 2>/dev/null | wc -l)
echo "  Test functions found: $TEST_COUNT"

if [ "$TEST_COUNT" -ge "$MIN_TOTAL" ]; then
    log_pass "Total tests ($TEST_COUNT) meets minimum ($MIN_TOTAL)"
else
    log_fail "Total tests ($TEST_COUNT) below minimum ($MIN_TOTAL)"
fi

if [ "$UNIT_COUNT" -ge "$MIN_UNIT" ]; then
    log_pass "Unit tests ($UNIT_COUNT) meets minimum ($MIN_UNIT)"
else
    log_fail "Unit tests ($UNIT_COUNT) below minimum ($MIN_UNIT)"
fi

# Verify tests pass
echo ""
echo "--- Test Execution ---"
echo ""

if cargo test --lib recovery_recipes --manifest-path "$PROJECT_DIR/Cargo.toml" 2>&1 | tail -5 | grep -q "test result: ok"; then
    log_pass "All recovery_recipes tests pass"
else
    log_fail "Some recovery_recipes tests fail"
fi

echo ""
echo "═══ Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Recovery recipes coverage check FAILED."
    exit 1
fi

echo "Recovery recipes coverage check PASSED."
exit 0
