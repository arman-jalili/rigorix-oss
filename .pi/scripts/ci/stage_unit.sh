#!/usr/bin/env bash
# Stage 5: Unit Tests
#
# Verifies:
# - Domain tests
# - Application tests
# - Contract tests
# - Verification tests
# - Coverage threshold check

set -euo pipefail

FAIL=0
PASS=0
COVERAGE_PCT=0

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1 — $2"; ((FAIL++)); }

echo "  Running unit tests..."

if command -v pytest &>/dev/null; then
    # Domain tests
    if [[ -d "tests/unit/domain" ]]; then
        if pytest tests/unit/domain -v 2>/dev/null; then
            log_pass "unit: domain tests"
        else
            log_fail "unit: domain tests" "Domain test failures"
        fi
    else
        log_pass "unit: domain tests (no domain tests found)"
    fi

    # Application tests
    if [[ -d "tests/unit/application" ]]; then
        if pytest tests/unit/application -v 2>/dev/null; then
            log_pass "unit: application tests"
        else
            log_fail "unit: application tests" "Application test failures"
        fi
    else
        log_pass "unit: application tests (no application tests found)"
    fi

    # Contract tests
    if [[ -d "tests/contract" ]]; then
        if pytest tests/contract -v 2>/dev/null; then
            log_pass "unit: contract tests"
        else
            log_fail "unit: contract tests" "Contract test failures"
        fi
    else
        log_pass "unit: contract tests (no contract tests found)"
    fi

    # Verification tests
    if [[ -d "tests/verification" ]]; then
        if pytest tests/verification -v 2>/dev/null; then
            log_pass "unit: verification tests"
        else
            log_fail "unit: verification tests" "Verification test failures"
        fi
    else
        log_pass "unit: verification tests (no verification tests found)"
    fi

    # Coverage threshold check
    echo "  Checking coverage threshold..."
    if [[ -f ".pi/scripts/ci/check_coverage_thresholds.py" ]]; then
        if python3 ".pi/scripts/ci/check_coverage_thresholds.py" 2>/dev/null; then
            log_pass "coverage threshold check"
        else
            log_fail "coverage threshold check" "Coverage below threshold"
        fi
    else
        # Basic coverage check
        coverage_file="coverage.xml"
        if [[ -f "$coverage_file" ]]; then
            COVERAGE_PCT=$(grep -oP 'line-rate="[^"]*"' "$coverage_file" 2>/dev/null | head -1 | cut -d'"' -f2 || echo "0")
            threshold="${COVERAGE_THRESHOLD:-80}"
            COVERAGE_INT=$(echo "$COVERAGE_PCT * 100" | bc 2>/dev/null || echo "0")
            if [[ $COVERAGE_INT -ge $threshold ]]; then
                log_pass "coverage threshold (${COVERAGE_PCT} >= ${threshold}%)"
            else
                log_fail "coverage threshold" "${COVERAGE_PCT} < ${threshold}%"
            fi
        else
            log_pass "coverage threshold check (no coverage file found)"
        fi
    fi
elif command -v bun &>/dev/null && [[ -f "package.json" ]]; then
    if bun test 2>/dev/null; then
        log_pass "unit: all tests (bun)"
    else
        log_fail "unit: all tests (bun)" "Test failures"
    fi
elif command -v cargo &>/dev/null && [[ -f "Cargo.toml" ]]; then
    if cargo test 2>/dev/null; then
        log_pass "unit: all tests (cargo)"
    else
        log_fail "unit: all tests (cargo)" "Test failures"
    fi
elif command -v go &>/dev/null && [[ -f "go.mod" ]]; then
    if go test ./... 2>/dev/null; then
        log_pass "unit: all tests (go)"
    else
        log_fail "unit: all tests (go)" "Test failures"
    fi
else
    echo "  No test runner found, skipping unit tests."
fi

if [[ $FAIL -gt 0 ]]; then
    echo "  Unit test stage FAILED (${FAIL} failure(s))"
    exit 1
fi

echo "  Unit test stage passed (${PASS} check(s))"
exit 0
