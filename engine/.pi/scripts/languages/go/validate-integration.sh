#!/usr/bin/env bash
# ============================================================================
# validate-integration.sh — Go
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Integration Validation (Go)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Integration Test Runner
# ---------------------------------------------------------------------------
echo "--- Integration Test Runner ---"
if [ -f "go.mod" ]; then
    HAS_INTEGRATION=false
    # Check for dedicated integration test files
    if find . -name "*_integration_test.go" -not -path "./vendor/*" 2>/dev/null | grep -q .; then
        HAS_INTEGRATION=true
    fi
    # Check for tests/integration directory
    if [ -d "tests/integration" ]; then
        HAS_INTEGRATION=true
    fi
    # Check for integration build tag
    if grep -rl '//go:build integration' --include="*_test.go" . 2>/dev/null | grep -q .; then
        HAS_INTEGRATION=true
    fi
    if [ "$HAS_INTEGRATION" = true ]; then
        if go test -tags=integration ./... -count=1 2>/dev/null; then
            pass "Integration tests passed"
        else
            fail "Integration tests failed"
        fi
    else
        pass "No integration test files found (skipped)"
    fi
else
    pass "No go.mod (skipping integration tests)"
fi

# ---------------------------------------------------------------------------
# Docker/Compose Dependencies
# ---------------------------------------------------------------------------
echo ""
echo "--- Docker/Compose Dependencies ---"
if [ -f "docker-compose.yml" ] || [ -f "docker-compose.yaml" ]; then
    warn "Docker Compose file found; integration tests may require running services"
elif [ -f "Dockerfile" ]; then
    warn "Dockerfile found; integration tests may require containerized environment"
else
    pass "No Docker Compose files found"
fi

# ---------------------------------------------------------------------------
# Contract Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Contract Tests ---"
if [ -f "go.mod" ]; then
    CONTRACT_FILES=$(find . -name "*_contract_test.go" -not -path "./vendor/*" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$CONTRACT_FILES" -gt 0 ]; then
        pass "Contract test files found ($CONTRACT_FILES files)"
    else
        warn "No contract test files (*_contract_test.go) found"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# Mock/Stub Detection
# ---------------------------------------------------------------------------
echo ""
echo "--- Mock/Stub Detection ---"
if [ -f "go.mod" ]; then
    MOCK_PATTERNS=0
    grep -qE '"github.com/golang/mock' . -r --include="*.go" 2>/dev/null && MOCK_PATTERNS=$((MOCK_PATTERNS + 1))
    grep -qE '"github.com/stretchr/testify/mock"' . -r --include="*.go" 2>/dev/null && MOCK_PATTERNS=$((MOCK_PATTERNS + 1))
    grep -qE '"go.uber.org/mock' . -r --include="*.go" 2>/dev/null && MOCK_PATTERNS=$((MOCK_PATTERNS + 1))
    # Check for manual mock directories
    if [ -d "mock" ] || [ -d "mocks" ] || find . -name "mock_*" -path "*/test*" 2>/dev/null | grep -q .; then
        MOCK_PATTERNS=$((MOCK_PATTERNS + 1))
    fi
    if [ "$MOCK_PATTERNS" -gt 0 ]; then
        pass "Mock/stub patterns detected ($MOCK_PATTERNS approaches found)"
    else
        warn "No mock/stub patterns detected (consider gomock or testify/mock)"
    fi
else
    warn "Not in a Go module, skipping mock detection"
fi

# ---------------------------------------------------------------------------
# Database Integration
# ---------------------------------------------------------------------------
echo ""
echo "--- Database Integration ---"
if [ -f "go.mod" ]; then
    DB_TEST_FILES=0
    find . -name "*_integration_test.go" -not -path "./vendor/*" 2>/dev/null | grep -q . && DB_TEST_FILES=$((DB_TEST_FILES + 1))
    find . -name "*db_test.go" -not -path "./vendor/*" 2>/dev/null | grep -q . && DB_TEST_FILES=$((DB_TEST_FILES + 1))
    find . -name "*repository*_test.go" -not -path "./vendor/*" 2>/dev/null | grep -q . && DB_TEST_FILES=$((DB_TEST_FILES + 1))
    if [ "$DB_TEST_FILES" -gt 0 ]; then
        pass "Database integration test patterns found ($DB_TEST_FILES patterns)"
    else
        warn "No database integration test files found (*_integration_test.go, *db_test.go)"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# HTTP Integration
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Integration ---"
if [ -f "go.mod" ]; then
    HTTPTEST_COUNT=$(grep -rl '"net/http/httptest"' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$HTTPTEST_COUNT" -gt 0 ]; then
        pass "HTTP integration tests found (httptest in $HTTPTEST_COUNT files)"
    else
        warn "No httptest usage found (consider net/http/httptest for HTTP integration tests)"
    fi
else
    pass "No Go source files to check"
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
