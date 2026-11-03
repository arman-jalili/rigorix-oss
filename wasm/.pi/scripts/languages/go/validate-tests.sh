#!/usr/bin/env bash
# ============================================================================
# validate-tests.sh — Go
# ============================================================================
set -euo pipefail

ERRORS=()
WARNINGS=()
PASS_COUNT=0

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Test Validation (Go)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Unit Tests
# ---------------------------------------------------------------------------
echo "--- Unit Tests ---"
if [ -f "go.mod" ]; then
    # Prefer internal/ and pkg/ if they exist; fall back to ./...
    if [ -d "internal" ] || [ -d "pkg" ]; then
        TEST_PATHS=""
        [ -d "internal" ] && TEST_PATHS="./internal/..."
        [ -d "pkg" ] && TEST_PATHS="${TEST_PATHS:+$TEST_PATHS }./pkg/..."
        if go test ${TEST_PATHS} -count=1 -v 2>/dev/null; then
            pass "Unit tests passed"
        else
            fail "Unit tests failed"
        fi
    else
        if go test ./... -count=1 -v 2>/dev/null; then
            pass "Unit tests passed"
        else
            fail "Unit tests failed"
        fi
    fi
else
    pass "No go.mod (skipping unit tests)"
fi

# ---------------------------------------------------------------------------
# Integration Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Integration Tests ---"
if [ -f "go.mod" ]; then
    HAS_INTEGRATION=false
    if [ -d "tests/integration" ]; then
        HAS_INTEGRATION=true
    fi
    # Also check for _test.go files with build tag "integration"
    if grep -rl '//go:build integration' --include="*_test.go" . 2>/dev/null | grep -q .; then
        HAS_INTEGRATION=true
    fi
    if [ "$HAS_INTEGRATION" = true ]; then
        if go test -tags=integration ./... -count=1 -v 2>/dev/null; then
            pass "Integration tests passed"
        else
            fail "Integration tests failed"
        fi
    else
        pass "No integration tests found (skipped)"
    fi
else
    pass "No go.mod (skipping integration tests)"
fi

# ---------------------------------------------------------------------------
# Coverage
# ---------------------------------------------------------------------------
echo ""
echo "--- Coverage ---"
if [ -f "go.mod" ]; then
    COVERAGE_OUTPUT=$(go test -coverprofile=coverage.out ./... 2>&1 || true)
    if [ -f "coverage.out" ]; then
        COVERAGE_PCT=$(go tool cover -func=coverage.out 2>/dev/null | grep "^total:" | awk '{print $3}' | tr -d '%' || echo "0")
        if [ -z "$COVERAGE_PCT" ] || [ "$COVERAGE_PCT" = "0.0" ]; then
            COVERAGE_PCT="0"
        fi
        # Compare coverage percentage (use awk for float comparison)
        MEETS_THRESHOLD=$(echo "$COVERAGE_PCT" | awk '{if ($1 >= 80) print 1; else print 0}')
        if [ "$MEETS_THRESHOLD" -eq 1 ]; then
            pass "Coverage: ${COVERAGE_PCT}% (≥ 80%)"
        else
            fail "Coverage: ${COVERAGE_PCT}% (< 80%)"
        fi
        rm -f coverage.out
    else
        warn "No coverage output generated"
    fi
else
    warn "go.mod not found, skipping coverage"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${#ERRORS[@]}${NC}"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    exit 1
fi

echo -e "${GREEN}Test validation passed.${NC}"
exit 0
