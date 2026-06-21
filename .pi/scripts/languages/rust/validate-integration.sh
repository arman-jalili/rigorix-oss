#!/usr/bin/env bash
# ============================================================================
# validate-integration.sh — Rust
# ============================================================================
set -euo pipefail

PASS_COUNT=0
ERRORS=()
WARNINGS=()

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Integration Validation (Rust)"
echo "============================================"
echo ""

if [ ! -f "Cargo.toml" ]; then
    warn "No Cargo.toml found (skipping Rust integration validation)"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
    echo -e "  Failed:   ${RED}${#ERRORS[@]}${NC}"
    echo ""
    echo -e "${GREEN}No Rust project detected, nothing to validate.${NC}"
    exit 0
fi

# ---------------------------------------------------------------------------
# Integration test runner
# ---------------------------------------------------------------------------
echo "--- Integration Test Runner ---"
if [ -d "tests" ]; then
    INT_TEST_FILES=$(find tests -name "*.rs" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$INT_TEST_FILES" -gt 0 ]; then
        if cargo test --test '*' --quiet 2>/dev/null; then
            pass "Integration tests passed ($INT_TEST_FILES test files)"
        else
            fail "Integration tests failed"
        fi
    else
        warn "tests/ directory exists but contains no .rs files"
    fi
else
    warn "No tests/ directory found (no integration tests to run)"
fi

# ---------------------------------------------------------------------------
# Docker/Compose services
# ---------------------------------------------------------------------------
echo ""
echo "--- Docker/Compose ---"
if [ -f "docker-compose.yml" ] || [ -f "docker-compose.yaml" ]; then
    warn "Docker Compose detected — integration tests may require services (run docker-compose up)"
else
    pass "No Docker Compose configuration found"
fi

# ---------------------------------------------------------------------------
# Contract tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Contract Tests ---"
CONTRACT_TESTS=$(find . -name "*contract*test*.rs" -o -name "*test*contract*.rs" 2>/dev/null | wc -l | tr -d ' ')
if [ "$CONTRACT_TESTS" -gt 0 ]; then
    pass "Contract test files found ($CONTRACT_TESTS)"
else
    warn "No contract test files found (consider adding contract tests for external interfaces)"
fi

# ---------------------------------------------------------------------------
# Mock/stub detection
# ---------------------------------------------------------------------------
echo ""
echo "--- Mock/Stub Detection ---"
MOCK_LIBS=0
if grep -qE '^\s*(mockall|mockito|wiremock)\s*=' Cargo.toml 2>/dev/null; then
    MOCK_LIBS=1
    pass "Mocking library detected in Cargo.toml"
fi
if [ "$MOCK_LIBS" -eq 0 ]; then
    # Also check for manual mock patterns
    if grep -rqE '#\[mockall|MockServer|mock_' --include="*.rs" . 2>/dev/null; then
        pass "Mocking patterns found in source"
    else
        warn "No mocking library or patterns detected (mockall, mockito, or wiremock)"
    fi
fi

# ---------------------------------------------------------------------------
# Database integration
# ---------------------------------------------------------------------------
echo ""
echo "--- Database Integration ---"
DB_INTEG=$(find . -name "*_integration_test.rs" -o -name "*db*test*.rs" 2>/dev/null | wc -l | tr -d ' ')
if [ "$DB_INTEG" -gt 0 ]; then
    pass "Database integration test files found ($DB_INTEG)"
else
    warn "No database integration test files found"
fi

# ---------------------------------------------------------------------------
# HTTP integration
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Integration ---"
HTTP_TEST=0
if grep -rqE 'axum::test|reqwest|actix_web::test' --include="*.rs" . 2>/dev/null; then
    HTTP_TEST=1
    pass "HTTP integration testing detected"
fi
if grep -qE '^\s*(axum|actix-web|warp|rocket)\s*=' Cargo.toml 2>/dev/null; then
    if [ "$HTTP_TEST" -eq 0 ]; then
        warn "HTTP framework detected but no HTTP test imports found (consider adding integration tests)"
    fi
else
    pass "No HTTP framework dependency found"
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

echo -e "${GREEN}Integration validation completed.${NC}"
exit 0
