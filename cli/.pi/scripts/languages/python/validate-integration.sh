#!/usr/bin/env bash
# ============================================================================
# validate-integration.sh — Python
# ============================================================================
set -euo pipefail

if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    POETRY_ENV=$(poetry env info --path 2>/dev/null || true)
    if [ -n "$POETRY_ENV" ] && [ -f "$POETRY_ENV/bin/activate" ]; then
        source "$POETRY_ENV/bin/activate"
    fi
fi

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Integration Validation (Python)"
echo "============================================"
echo ""

# ── Integration Tests ──
echo "--- Integration Tests ---"
if [ -d "tests/integration" ] && command -v pytest &>/dev/null; then
    if PYTHONPATH=src pytest tests/integration -q 2>/dev/null; then
        pass "Integration tests passed"
    else
        fail "Integration tests failed"
    fi
elif [ -d "tests/integration" ]; then
    warn "Integration test directory exists but pytest is not available"
else
    pass "No integration tests directory (skipped)"
fi

# ── Docker/Compose Detection ──
echo ""
echo "--- Docker/Compose Detection ---"
if [ -f "docker-compose.yml" ] || [ -f "docker-compose.yaml" ]; then
    warn "Docker Compose detected — integration tests may require running services"
    pass "Docker Compose configuration present"
else
    pass "No Docker Compose configuration"
fi

# ── Contract Tests ──
echo ""
echo "--- Contract Tests ---"
CONTRACT_COUNT=$(find tests -name "*contract*" -o -name "*pact*" 2>/dev/null | wc -l | tr -d ' ')
if [ "$CONTRACT_COUNT" -gt 0 ]; then
    pass "Contract test files found ($CONTRACT_COUNT)"
else
    warn "No contract tests found"
fi

# ── Mock/Stub Detection ──
echo ""
echo "--- Mock/Stub Detection ---"
HAS_MOCKS=false
for pattern in "unittest.mock" "mock.patch" "pytest-mock" "responses" "httpretty" "aioresponses" "responses.activate"; do
    if grep -rq "$pattern" tests/ --include="*.py" 2>/dev/null; then
        HAS_MOCKS=true
        break
    fi
done
if [ "$HAS_MOCKS" = true ]; then
    pass "Mock/stub patterns detected in test suite"
else
    warn "No mock patterns found in test suite"
fi

# ── Database Integration Tests ──
echo ""
echo "--- Database Integration Tests ---"
DB_TEST_COUNT=$(grep -rlE "(pytest.*db|session_scope|test_db|database_url|SQLALCHEMY)" tests/ --include="*.py" 2>/dev/null | wc -l | tr -d ' ')
if [ "$DB_TEST_COUNT" -gt 0 ]; then
    pass "Database integration tests found ($DB_TEST_COUNT files)"
else
    pass "No database integration tests (skipped if not applicable)"
fi

# ── HTTP Integration Tests ──
echo ""
echo "--- HTTP Integration Tests ---"
HAS_HTTP_TEST=false
for pattern in "TestClient" "httpx.AsyncClient" "aiohttp.test" "responses" "requests_mock"; do
    if grep -rq "$pattern" tests/ --include="*.py" 2>/dev/null; then
        HAS_HTTP_TEST=true
        break
    fi
done
if [ "$HAS_HTTP_TEST" = true ]; then
    pass "HTTP integration test patterns detected"
else
    pass "No HTTP integration test patterns found"
fi

# ── Summary ──
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

echo -e "${GREEN}Integration validation passed.${NC}"
exit 0
