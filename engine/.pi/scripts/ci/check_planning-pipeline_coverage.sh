#!/usr/bin/env bash
# ============================================================================
# check_planning-pipeline_coverage.sh
#
# Enforces minimum code coverage thresholds for the planning-pipeline module.
# Falls back to test count if no coverage tool is available.
#
# Usage: bash .pi/scripts/ci/check_planning-pipeline_coverage.sh [--help]
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
echo "═══ Planning-Pipeline Coverage Threshold Check ═══"
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

echo "Minimum coverage: ${MIN_COVERAGE}%"
echo ""

# Try llvm-cov first, then tarpaulin
if [[ "${RIGORIX_COVERAGE:-}" == "1" ]] && command -v cargo-llvm-cov &>/dev/null; then
    echo "Using cargo-llvm-cov for coverage..."
    cd "$PROJECT_ROOT"

    COVERAGE_OUTPUT=$(cargo llvm-cov --lib --json 2>&1 || true)
    COVERAGE_PCT=$(echo "$COVERAGE_OUTPUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('data',[{}])[0].get('totals',{}).get('lines',{}).get('percent',0))" 2>/dev/null || echo "")

    if [ -n "$COVERAGE_PCT" ]; then
        COVERAGE_INT=${COVERAGE_PCT%.*}
        if [ "$COVERAGE_INT" -ge "$MIN_COVERAGE" ] 2>/dev/null; then
            log_pass "Coverage ${COVERAGE_PCT}% meets threshold (≥${MIN_COVERAGE}%)"
        else
            log_fail "Coverage ${COVERAGE_PCT}% is below threshold (${MIN_COVERAGE}%)"
        fi
    else
        log_fail "Could not determine coverage percentage"
    fi

    cd ..
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
        COVERAGE_PCT=$(echo "$COVERAGE_OUTPUT" | grep -oE '[0-9]+\.[0-9]+' | tail -1)
    fi

    if [ -n "$COVERAGE_PCT" ]; then
        COVERAGE_INT=${COVERAGE_PCT%.*}
        if [ "$COVERAGE_INT" -ge "$MIN_COVERAGE" ] 2>/dev/null; then
            log_pass "Coverage ${COVERAGE_PCT}% meets threshold (≥${MIN_COVERAGE}%)"
        else
            log_fail "Coverage ${COVERAGE_PCT}% is below threshold (${MIN_COVERAGE}%)"
        fi
    else
        log_fail "Could not determine coverage percentage"
    fi

    cd ..
else
    echo "Instrumented coverage skipped (set RIGORIX_COVERAGE=1 to enable)."
    echo "Falling back to test count verification..."

    # Count tests in planning module
    TEST_COUNT=$(grep -c '#\[tokio::test\]\|#\[test\]' "$PROJECT_ROOT/src/planning/tests.rs" 2>/dev/null || echo 0)
    echo "Planning module tests found: $TEST_COUNT"

    if [ "$TEST_COUNT" -ge 40 ]; then
        log_pass "Sufficient tests ($TEST_COUNT) as proxy for coverage"
    else
        log_fail "Fewer than 40 tests ($TEST_COUNT) — coverage may be insufficient"
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
    echo "Planning-pipeline coverage check FAILED."
    exit 1
fi

echo "Planning-pipeline coverage check PASSED."
exit 0
