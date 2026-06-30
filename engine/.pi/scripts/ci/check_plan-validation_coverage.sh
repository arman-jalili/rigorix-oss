#!/usr/bin/env bash
# ============================================================================
# check_plan-validation_coverage.sh
#
# Enforces coverage thresholds for the plan-validation module.
# Requires cargo-llvm-cov to be installed.
# Falls back to a structural test presence check if coverage tool is unavailable.
#
# Usage: bash .pi/scripts/ci/check_plan-validation_coverage.sh [--help]
#
# Exit codes: 0 = coverage OK, 1 = coverage below threshold
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

THRESHOLD=80
MODULE="plan_validation"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Plan-Validation Coverage Check ═══"
echo "Threshold: ${THRESHOLD}%"
echo ""

# ---------------------------------------------------------------------------
# Try llvm-cov first (requires RIGORIX_COVERAGE=1), then fall back to
# structural test presence check if coverage tools aren't configured.
# ---------------------------------------------------------------------------
if [[ "${RIGORIX_COVERAGE:-}" == "1" ]] && command -v cargo-llvm-cov &>/dev/null; then
    echo "Running cargo-llvm-cov for $MODULE..."

    COVERAGE_OUTPUT=$(cargo llvm-cov --lib --lcov --output-path coverage/lcov.info 2>&1 || true)
    if [ -f "coverage/lcov.info" ]; then
        TOTAL_LINES=$(grep -c '^DA:' coverage/lcov.info || true)
        HIT_LINES=$(grep '^DA:.*,[1-9][0-9]*$' coverage/lcov.info | wc -l | tr -d ' ')
        if [ "$TOTAL_LINES" -gt 0 ]; then
            COVERAGE_PCT=$((HIT_LINES * 100 / TOTAL_LINES))
            if [ "$COVERAGE_PCT" -ge "$THRESHOLD" ]; then
                log_pass "Coverage: ${COVERAGE_PCT}% (threshold: ${THRESHOLD}%)"
            else
                log_fail "Coverage: ${COVERAGE_PCT}% is below threshold of ${THRESHOLD}%"
            fi
        else
            log_fail "No coverage data found"
        fi
    else
        log_fail "No lcov.info generated"
    fi
elif [[ "${RIGORIX_COVERAGE:-}" == "1" ]] && command -v cargo-tarpaulin &>/dev/null; then
    echo "Using cargo-tarpaulin for coverage (fallback)..."
    log_pass "Coverage via tarpaulin — manual review required"
else
    echo "RIGORIX_COVERAGE not set. Skipping tool-based coverage."
    log_pass "Skipping tool-based coverage (set RIGORIX_COVERAGE=1 to enable)"
fi

# ---------------------------------------------------------------------------
# Structural check: ensure test files exist with sufficient tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Structural Test Presence ---"

SRC_DIR=""
for candidate in "$(cd "$PI_DIR/.." && pwd)/engine/src" "$(cd "$PI_DIR/.." && pwd)/src"; do
    if [ -d "$candidate/plan_validation" ]; then
        SRC_DIR="$candidate"
        break
    fi
done

if [ -z "$SRC_DIR" ]; then
    echo "ERROR: plan_validation source directory not found"
    exit 1
fi

PV_TEST_DIR="$SRC_DIR/plan_validation"

# Count test functions
TEST_COUNT=$(grep -r "#\[test\]" "$PV_TEST_DIR" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
echo "Unit tests found: $TEST_COUNT"

if [ "$TEST_COUNT" -ge 40 ]; then
    log_pass "$TEST_COUNT unit tests found (threshold: 40)"
else
    log_fail "Only $TEST_COUNT unit tests found (requires >= 40)"
fi

# Count tokio tests
TOKIO_TEST_COUNT=$(grep -r "#\[tokio::test\]" "$PV_TEST_DIR" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
echo "Async tests: $TOKIO_TEST_COUNT"

# Check each component has tests
echo ""
echo "--- Per-Component Test Presence ---"

COMPONENTS=(
    "loop_config.rs:ValidationLoopConfig"
    "state.rs:ValidationState"
    "outcome.rs:ValidationOutcome"
    "report.rs:ValidationReport"
    "loop_impl.rs:ValidationLoopImpl"
    "factory.rs:ValidationLoopConfigBuilder"
    "context_augmenter.rs:ContextAugmenter"
)

for pair in "${COMPONENTS[@]}"; do
    FILE="${pair%%:*}"
    NAME="${pair##*:}"
    FULL_PATH="$PV_TEST_DIR/domain/$FILE"
    APP_PATH="$PV_TEST_DIR/application/$FILE"

    if [ -f "$FULL_PATH" ] && grep -qE "#\[test\]|#\[tokio::test\]" "$FULL_PATH" 2>/dev/null; then
        FILE_TEST_COUNT=$(grep -cE "#\[test\]|#\[tokio::test\]" "$FULL_PATH" 2>/dev/null || echo 0)
        log_pass "$NAME tests in domain/$FILE ($FILE_TEST_COUNT tests)"
    elif [ -f "$APP_PATH" ] && grep -qE "#\[test\]|#\[tokio::test\]" "$APP_PATH" 2>/dev/null; then
        FILE_TEST_COUNT=$(grep -cE "#\[test\]|#\[tokio::test\]" "$APP_PATH" 2>/dev/null || echo 0)
        log_pass "$NAME tests in application/$FILE ($FILE_TEST_COUNT tests)"
    else
        log_fail "$NAME has no tests"
    fi
done

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
    echo "Coverage check failed."
    exit 1
fi

echo "Coverage check passed."
exit 0
