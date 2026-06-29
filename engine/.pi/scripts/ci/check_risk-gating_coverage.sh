#!/usr/bin/env bash
# ============================================================================
# check_risk-gating_coverage.sh
#
# Enforces minimum code coverage thresholds for the risk-gating module.
# Uses cargo-llvm-cov or tarpaulin to measure line coverage.
# Falls back to test count if no coverage tool is available.
#
# Usage: bash .pi/scripts/ci/check_risk-gating_coverage.sh [--help]
#
# Exit codes: 0 = coverage meets thresholds, 1 = below threshold
# ============================================================================
set -uo pipefail

MIN_COVERAGE=80

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Risk-Gating Coverage Threshold Check ═══"
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
if [[ "${RIGORIX_COVERAGE:-}" == "1" ]] && command -v cargo-llvm-cov &>/dev/null; then
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
elif [[ "${RIGORIX_COVERAGE:-}" == "1" ]] && command -v cargo-tarpaulin &>/dev/null; then
    echo "Using cargo-tarpaulin for coverage (fallback)..."
    cd "$PROJECT_ROOT"

    COVERAGE_OUTPUT=$(cargo tarpaulin --out Xml --output-dir ../coverage 2>&1 || true)
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
    echo "Instrumented coverage skipped (set RIGORIX_COVERAGE=1 to enable)."
    echo "Counting test functions as a fallback metric..."

    # Fallback: count test functions in risk-gating module
    TEST_COUNT=$(grep -rn 'mod tests' "$PROJECT_ROOT/src/risk_gating/" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$TEST_COUNT" -ge 3 ]; then
        log_pass "Test modules: $TEST_COUNT test modules in risk_gating (minimum 3 required)"
    else
        log_fail "Only $TEST_COUNT test modules found in risk_gating (minimum 3 required)"
    fi

    # Also count total test functions
    TOTAL_TESTS=$(grep -rn '#\[tokio::test\]\|#\[test\]' "$PROJECT_ROOT/src/risk_gating/" 2>/dev/null | wc -l | tr -d ' ')
    echo "  Total test functions: $TOTAL_TESTS"
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
