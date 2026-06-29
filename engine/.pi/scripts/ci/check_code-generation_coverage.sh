#!/usr/bin/env bash
# ============================================================================
# check_code-generation_coverage.sh
#
# Enforces minimum code coverage thresholds for the code-generation module.
# Uses cargo-llvm-cov or tarpaulin to measure line coverage.
# Falls back to test count if no coverage tool is available.
#
# Usage: bash .pi/scripts/ci/check_code-generation_coverage.sh [--help]
#
# Exit codes: 0 = coverage meets thresholds, 1 = below threshold
# ============================================================================
set -uo pipefail

MIN_COVERAGE=80
MIN_TEST_COUNT=30

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Code-Generation Coverage Threshold Check ═══"
echo ""

# Determine if we're in a Rust project
if [ ! -f "Cargo.toml" ] && [ ! -f "engine/Cargo.toml" ]; then
    log_fail "No Cargo.toml found (not a Rust project)"
    echo ""
    echo "Coverage threshold check FAILED."
    exit 1
fi

# Determine project root
PROJECT_ROOT="."
if [ -f "engine/Cargo.toml" ]; then
    PROJECT_ROOT="engine"
fi

echo "Project: $PROJECT_ROOT"
echo "Minimum coverage: ${MIN_COVERAGE}%"
echo ""

# Try llvm-cov first, then tarpaulin
if command -v cargo-llvm-cov &>/dev/null; then
    echo "Using cargo-llvm-cov for coverage..."

    COVERAGE_OUTPUT=$(cargo llvm-cov --workspace --lcov --output-path coverage/lcov.info 2>&1 || true)
    if [ -f "coverage/lcov.info" ]; then
        TOTAL_LINES=$(grep -c '^DA:' coverage/lcov.info || true)
        HIT_LINES=$(grep '^DA:.*,[1-9][0-9]*$' coverage/lcov.info | wc -l | tr -d ' ')
        if [ "$TOTAL_LINES" -gt 0 ]; then
            COVERAGE_PCT=$((HIT_LINES * 100 / TOTAL_LINES))
            if [ "$COVERAGE_PCT" -ge "$MIN_COVERAGE" ]; then
                log_pass "Coverage: ${COVERAGE_PCT}% (threshold: ${MIN_COVERAGE}%)"
            else
                log_fail "Coverage: ${COVERAGE_PCT}% is below threshold of ${MIN_COVERAGE}%"
            fi
        else
            log_fail "No coverage data found"
        fi
    else
        log_fail "No lcov.info generated"
    fi
elif command -v cargo-tarpaulin &>/dev/null; then
    echo "Using cargo-tarpaulin for coverage (fallback)..."
    cd "$PROJECT_ROOT"

    COVERAGE_OUTPUT=$(cargo tarpaulin --out Xml --output-dir ../coverage --exclude-files "src/code_gen/interfaces/*" 2>&1 || true)
    COVERAGE_PCT=$(echo "$COVERAGE_OUTPUT" | grep -oE '[0-9]+\.[0-9]+%' | tail -1 | tr -d '%')

    if [ -z "$COVERAGE_PCT" ]; then
        if [ -f "../coverage/cobertura.xml" ]; then
            COVERAGE_PCT=$(grep -oE 'line-rate="[0-9.]+"' ../coverage/cobertura.xml | head -1 | grep -oE '[0-9.]+' | awk '{printf "%.0f", $1 * 100}')
        fi
    fi

    if [ -z "$COVERAGE_PCT" ]; then
        log_fail "Could not determine coverage percentage from tarpaulin output"
    elif [ "$(printf '%.0f' "$COVERAGE_PCT")" -ge "$MIN_COVERAGE" ]; then
        log_pass "Coverage: ${COVERAGE_PCT}% (threshold: ${MIN_COVERAGE}%)"
    else
        log_fail "Coverage: ${COVERAGE_PCT}% is below threshold of ${MIN_COVERAGE}%"
    fi

    cd ..

else
    echo "No coverage tool found (cargo-tarpaulin or cargo-llvm-cov)."
    echo "Counting test functions as a fallback metric..."

    TEST_COUNT=$(grep -r '^\s*#\[test\]' "$PROJECT_ROOT/src/code_gen/" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$TEST_COUNT" -ge "$MIN_TEST_COUNT" ]; then
        log_pass "Test count: $TEST_COUNT tests in code_gen module (minimum $MIN_TEST_COUNT required)"
    else
        log_fail "Only $TEST_COUNT tests found in code_gen module (minimum $MIN_TEST_COUNT required)"
    fi
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
    echo "Coverage thresholds not met."
    exit 1
fi

echo "Coverage thresholds satisfied."
exit 0
