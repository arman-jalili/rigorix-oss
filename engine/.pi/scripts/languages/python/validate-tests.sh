#!/usr/bin/env bash
# ============================================================================
# validate-tests.sh — Python (Poetry-based)
# ============================================================================
set -euo pipefail

# Activate Poetry venv if available
if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    POETRY_ENV=$(poetry env info --path 2>/dev/null || true)
    if [ -n "$POETRY_ENV" ] && [ -f "$POETRY_ENV/bin/activate" ]; then
        source "$POETRY_ENV/bin/activate"
    fi
fi

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
echo "  Test Validation (Python)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Unit Tests
# ---------------------------------------------------------------------------
echo "--- Unit Tests ---"
if [ -d "tests/unit" ] && command -v pytest &>/dev/null; then
    if PYTHONPATH=src pytest tests/unit -q 2>/dev/null; then
        pass "Unit tests passed"
    else
        fail "Unit tests failed"
    fi
else
    pass "No unit tests (skipped)"
fi

# ---------------------------------------------------------------------------
# Integration Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Integration Tests ---"
if [ -d "tests/integration" ] && command -v pytest &>/dev/null; then
    if PYTHONPATH=src pytest tests/integration -q 2>/dev/null; then
        pass "Integration tests passed"
    else
        fail "Integration tests failed"
    fi
else
    pass "No integration tests (skipped)"
fi

# ---------------------------------------------------------------------------
# Coverage
# ---------------------------------------------------------------------------
echo ""
echo "--- Coverage ---"
if command -v pytest &>/dev/null && command -v coverage &>/dev/null; then
    COVERAGE_OUTPUT=$(PYTHONPATH=src pytest tests/ --cov=src --cov-report=term-missing 2>&1 || true)
    COVERAGE_PCT=$(echo "$COVERAGE_OUTPUT" | grep "^TOTAL" | awk '{print $NF}' | tr -d '%' || echo "0")
    if [ -z "$COVERAGE_PCT" ]; then
        COVERAGE_PCT="0"
    fi
    if [ "$(echo "$COVERAGE_PCT >= 80" | bc -l 2>/dev/null || echo 0)" -eq 1 ]; then
        pass "Coverage: ${COVERAGE_PCT}% (≥ 80%)"
    else
        fail "Coverage: ${COVERAGE_PCT}% (< 80%)"
    fi
else
    warn "coverage tools not available"
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
