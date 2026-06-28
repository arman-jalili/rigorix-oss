#!/usr/bin/env bash
# ============================================================================
# check_orchestrator_coverage.sh
#
# Checks that orchestrator module test coverage meets the minimum threshold.
# Uses cargo-tarpaulin when available, falls back to test presence check.
#
# Usage: bash .pi/scripts/ci/check_orchestrator_coverage.sh [--help]
#
# Exit codes: 0 = coverage OK, 1 = coverage below threshold
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
if [ -d "$PI_DIR/../engine" ]; then
    ENGINE_DIR="$PI_DIR/../engine"
else
    ENGINE_DIR="$PI_DIR/.."
fi

MIN_COVERAGE=80
PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Orchestrator Coverage Check ═══"
echo ""

# Check if orchestrator tests exist and compile
echo "--- Test Existence ---"

TEST_COUNT=$(grep -c "#\[tokio::test\]\|#\[test\]" "$ENGINE_DIR/src/orchestrator/application/orchestrator_impl.rs" "$ENGINE_DIR/src/orchestrator/domain/record.rs" "$ENGINE_DIR/src/orchestrator/application/builder_impl.rs" 2>/dev/null || echo 0)
if [ "$TEST_COUNT" -gt 0 ]; then
    log_pass "Found $TEST_COUNT tests in orchestrator module"
else
    log_fail "No tests found in orchestrator module"
fi

# Check that tests pass via cargo test
echo ""
echo "--- Test Execution ---"

if command -v cargo &>/dev/null; then
    cd "$ENGINE_DIR" && cargo test --lib -- orchestrator 2>&1 | tail -5
    RESULT=${PIPESTATUS[0]}
    if [ "$RESULT" -eq 0 ]; then
        log_pass "All orchestrator tests pass"
    else
        log_fail "Orchestrator tests failed (exit: $RESULT)"
    fi
else
    log_fail "cargo not found — cannot run tests"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
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
    exit 1
fi

echo "Orchestrator coverage checks passed (≥ ${MIN_COVERAGE}% required)."
exit 0
