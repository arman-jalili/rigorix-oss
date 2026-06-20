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

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

echo ""
echo "═══ Plan-Validation Coverage Check ═══"
echo "Threshold: ${THRESHOLD}%"
echo ""

# ---------------------------------------------------------------------------
# Check if cargo-llvm-cov is available
# ---------------------------------------------------------------------------
if command -v cargo-llvm-cov 2>/dev/null || cargo llvm-cov --help &>/dev/null; then
    echo "Running cargo-llvm-cov for $MODULE..."
    cargo llvm-cov --lib --html 2>&1 | tail -5 || true

    # Extract coverage percentage for the module
    COVERAGE_REPORT="target/llvm-cov/html/index.html"
    if [ -f "$COVERAGE_REPORT" ]; then
        # Parse the coverage for our module
        MOD_COVERAGE=$(grep -oP "(?<=>${MODULE}<)[^<]*</a>\s*</td>\s*<td[^>]*>\s*[0-9.]+%" "$COVERAGE_REPORT" 2>/dev/null | grep -oP '[0-9.]+(?=%)' | head -1 || echo "0")
        echo "Coverage: ${MOD_COVERAGE}%"

        if (( $(echo "$MOD_COVERAGE >= $THRESHOLD" | bc -l 2>/dev/null) )); then
            log_pass "Coverage ${MOD_COVERAGE}% meets threshold ${THRESHOLD}%"
        else
            log_fail "Coverage ${MOD_COVERAGE}% below threshold ${THRESHOLD}%"
        fi
    else
        log_fail "Coverage report not found at $COVERAGE_REPORT"
    fi
else
    echo "cargo-llvm-cov not found. Falling back to structural check."
    log_pass "Using structural test presence check instead (coverage tool unavailable)"
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

    if [ -f "$FULL_PATH" ] && grep -q "#\[test\]" "$FULL_PATH" 2>/dev/null; then
        FILE_TEST_COUNT=$(grep -c "#\[test\]" "$FULL_PATH" 2>/dev/null || echo 0)
        log_pass "$NAME tests in domain/$FILE ($FILE_TEST_COUNT tests)"
    elif [ -f "$APP_PATH" ] && grep -q "#\[test\]" "$APP_PATH" 2>/dev/null; then
        FILE_TEST_COUNT=$(grep -c "#\[test\]" "$APP_PATH" 2>/dev/null || echo 0)
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
