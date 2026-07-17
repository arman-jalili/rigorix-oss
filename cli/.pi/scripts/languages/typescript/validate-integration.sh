#!/usr/bin/env bash
# ============================================================================
# validate-integration.sh — TypeScript
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Integration Validation (TypeScript)"
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
# Integration Test Runner
# ---------------------------------------------------------------------------
echo "--- Integration Test Runner ---"
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
# Docker/Compose
# ---------------------------------------------------------------------------
echo ""
echo "--- Docker/Compose ---"
if [ -f "docker-compose.yml" ] || [ -f "docker-compose.yaml" ]; then
    SERVICES=$(grep -E "^\s{2}\w+:" docker-compose.yml 2>/dev/null | grep -vE "^\s{2}version:|^\s{2}services:" | wc -l | tr -d ' ')
    if [ -f "docker-compose.yaml" ] && [ "$SERVICES" -eq 0 ]; then
        SERVICES=$(grep -E "^\s{2}\w+:" docker-compose.yaml 2>/dev/null | grep -vE "^\s{2}version:|^\s{2}services:" | wc -l | tr -d ' ')
    fi
    pass "Docker Compose found ($SERVICES services — integration tests may require running services)"
else
    pass "No Docker Compose configuration"
fi

# ---------------------------------------------------------------------------
# Contract Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Contract Tests ---"
CONTRACT_FILES=0
for dir in tests test; do
    if [ -d "$dir" ]; then
        COUNT=$(find "$dir" -name "*.contract.test.ts" -o -name "*.contract.test.tsx" 2>/dev/null | wc -l | tr -d ' ')
        CONTRACT_FILES=$((CONTRACT_FILES + COUNT))
    fi
done
if [ "$CONTRACT_FILES" -gt 0 ]; then
    pass "Contract test files found ($CONTRACT_FILES)"
else
    pass "No contract test files (*.contract.test.ts)"
fi

# ---------------------------------------------------------------------------
# Mock/Stub Detection
# ---------------------------------------------------------------------------
echo ""
echo "--- Mock/Stub Detection ---"
MOCK_USAGE=0
for ext in ts tsx; do
    COUNT=$(find . -name "*.$ext" -not -path "./node_modules/*" -not -path "./dist/*" \
        -exec grep -lE "(vitest\.mock|jest\.mock|from\s+['\"]msw['\"]|from\s+['\"]@testing-library)" {} + 2>/dev/null | wc -l | tr -d ' ')
    MOCK_USAGE=$((MOCK_USAGE + COUNT))
done
if [ "$MOCK_USAGE" -gt 0 ]; then
    pass "Mock/stub usage detected ($MOCK_USAGE files with vitest.mock, jest.mock, msw, or @testing-library)"
else
    warn "No mocking libraries detected (vitest.mock, jest.mock, msw)"
fi

# ---------------------------------------------------------------------------
# Database Integration
# ---------------------------------------------------------------------------
echo ""
echo "--- Database Integration ---"
DB_INTEGRATION_FILES=0
for dir in tests test; do
    if [ -d "$dir" ]; then
        COUNT=$(find "$dir" -name "*_integration.test.ts" -o -name "*_integration.test.tsx" -o -name "*.db.test.ts" -o -name "*.db.test.tsx" 2>/dev/null | wc -l | tr -d ' ')
        DB_INTEGRATION_FILES=$((DB_INTEGRATION_FILES + COUNT))
    fi
done
if [ "$DB_INTEGRATION_FILES" -gt 0 ]; then
    pass "Database integration test files found ($DB_INTEGRATION_FILES)"
else
    pass "No database integration test files"
fi

# ---------------------------------------------------------------------------
# HTTP Integration
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Integration ---"
HTTP_LIBS=0
for ext in ts tsx; do
    COUNT=$(find . -name "*.$ext" -not -path "./node_modules/*" -not -path "./dist/*" \
        -exec grep -lE "(supertest|msw|@testing-library)" {} + 2>/dev/null | wc -l | tr -d ' ')
    HTTP_LIBS=$((HTTP_LIBS + COUNT))
done
if [ "$HTTP_LIBS" -gt 0 ]; then
    pass "HTTP integration libraries detected ($HTTP_LIBS files with supertest, msw, or @testing-library)"
else
    pass "No HTTP integration libraries detected"
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

echo -e "${GREEN}Integration validation passed.${NC}"
exit 0
