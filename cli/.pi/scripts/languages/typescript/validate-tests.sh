#!/usr/bin/env bash
# ============================================================================
# validate-tests.sh — TypeScript
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Test Validation (TypeScript)"
echo "============================================"
echo ""

# Detect package manager
PKG_MGR=""
if [ -f "bun.lock" ] || [ -f "bun.lockb" ]; then
    PKG_MGR="bun"
elif [ -f "yarn.lock" ]; then
    PKG_MGR="yarn"
elif [ -f "package-lock.json" ]; then
    PKG_MGR="npm"
fi

# ---------------------------------------------------------------------------
# Unit Tests
# ---------------------------------------------------------------------------
echo "--- Unit Tests ---"
if [ -d "tests/unit" ] || [ -d "test/unit" ]; then
    UNIT_DIR=""
    if [ -d "tests/unit" ]; then
        UNIT_DIR="tests/unit"
    elif [ -d "test/unit" ]; then
        UNIT_DIR="test/unit"
    fi

    if [ "$PKG_MGR" = "bun" ] && command -v bun &>/dev/null; then
        if bun test "$UNIT_DIR" 2>/dev/null; then
            pass "Unit tests passed (bun test)"
        else
            fail "Unit tests failed (bun test)"
        fi
    elif command -v npx &>/dev/null; then
        if npx vitest run "$UNIT_DIR" 2>/dev/null; then
            pass "Unit tests passed (npx vitest)"
        elif npx jest "$UNIT_DIR" 2>/dev/null; then
            pass "Unit tests passed (npx jest)"
        else
            fail "Unit tests failed"
        fi
    else
        warn "No test runner available for unit tests"
    fi
else
    pass "No unit test directory (skipped)"
fi

# ---------------------------------------------------------------------------
# Integration Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Integration Tests ---"
if [ -d "tests/integration" ] || [ -d "test/integration" ]; then
    INT_DIR=""
    if [ -d "tests/integration" ]; then
        INT_DIR="tests/integration"
    elif [ -d "test/integration" ]; then
        INT_DIR="test/integration"
    fi

    if [ "$PKG_MGR" = "bun" ] && command -v bun &>/dev/null; then
        if bun test "$INT_DIR" 2>/dev/null; then
            pass "Integration tests passed (bun test)"
        else
            fail "Integration tests failed (bun test)"
        fi
    elif command -v npx &>/dev/null; then
        if npx vitest run "$INT_DIR" 2>/dev/null; then
            pass "Integration tests passed (npx vitest)"
        elif npx jest "$INT_DIR" 2>/dev/null; then
            pass "Integration tests passed (npx jest)"
        else
            fail "Integration tests failed"
        fi
    else
        warn "No test runner available for integration tests"
    fi
else
    pass "No integration test directory (skipped)"
fi

# ---------------------------------------------------------------------------
# Coverage
# ---------------------------------------------------------------------------
echo ""
echo "--- Coverage ---"
if command -v npx &>/dev/null; then
    COVERAGE_OUTPUT=""
    # Try vitest first
    if npx vitest run --coverage 2>&1 | tee /tmp/ts_coverage_output >/dev/null; then
        COVERAGE_OUTPUT=$(cat /tmp/ts_coverage_output)
    fi
    # If vitest didn't produce output, try jest
    if [ -z "$COVERAGE_OUTPUT" ]; then
        npx jest --coverage 2>&1 | tee /tmp/ts_coverage_output >/dev/null || true
        COVERAGE_OUTPUT=$(cat /tmp/ts_coverage_output)
    fi

    if [ -n "$COVERAGE_OUTPUT" ]; then
        # Extract percentage — vitest and jest report differently
        COVERAGE_PCT=$(echo "$COVERAGE_OUTPUT" | grep -iE "(statements|all files).*[0-9]+(\.[0-9]+)?%" | grep -oE '[0-9]+(\.[0-9]+)?%' | head -1 | tr -d '%' || echo "")
        if [ -z "$COVERAGE_PCT" ]; then
            # Fallback: look for any percentage on a TOTAL or All lines
            COVERAGE_PCT=$(echo "$COVERAGE_OUTPUT" | grep -iE "(total|all).*[0-9]+(\.[0-9]+)?%" | grep -oE '[0-9]+(\.[0-9]+)?%' | head -1 | tr -d '%' || echo "")
        fi
        if [ -n "$COVERAGE_PCT" ]; then
            # Compare against 80% threshold
            COVERAGE_INT=$(echo "$COVERAGE_PCT" | cut -d'.' -f1)
            if [ "${COVERAGE_INT:-0}" -ge 80 ]; then
                pass "Coverage: ${COVERAGE_PCT}% (≥ 80%)"
            else
                fail "Coverage: ${COVERAGE_PCT}% (< 80%)"
            fi
        else
            warn "Coverage output parsed but percentage not detected"
        fi
    else
        warn "No coverage tool produced output"
    fi
    rm -f /tmp/ts_coverage_output
else
    warn "npx not available, skipping coverage"
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
