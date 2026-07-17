#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Go
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
echo "  CI/MR Validation (Go)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo "--- Build ---"
if [ -f "go.mod" ]; then
    if go build ./... 2>/dev/null; then
        pass "Build succeeded"
    else
        fail "Build failed"
    fi
else
    pass "No go.mod (not a Go module, skipping build)"
fi

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"
if [ -f "go.mod" ]; then
    TEST_FILES=$(find . -name "*_test.go" -not -path "./vendor/*" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$TEST_FILES" -gt 0 ]; then
        if go test ./... -count=1 2>/dev/null; then
            pass "All tests passed"
        else
            fail "Tests failed"
        fi
    else
        pass "No test files found (skipped)"
    fi
else
    pass "No go.mod (skipping tests)"
fi

# ---------------------------------------------------------------------------
# Lint
# ---------------------------------------------------------------------------
echo ""
echo "--- Lint ---"
if command -v golangci-lint &>/dev/null && [ -f "go.mod" ]; then
    if golangci-lint run 2>/dev/null; then
        pass "Lint passed"
    else
        fail "Lint failed"
    fi
elif command -v go &>/dev/null && [ -f "go.mod" ]; then
    if go vet ./... 2>/dev/null; then
        pass "go vet passed"
    else
        fail "go vet failed"
    fi
else
    warn "No Go lint tools available, skipping lint"
fi

# ---------------------------------------------------------------------------
# Format
# ---------------------------------------------------------------------------
echo ""
echo "--- Format ---"
if command -v gofmt &>/dev/null && [ -f "go.mod" ]; then
    FMT_OUT=$(gofmt -d . 2>/dev/null | grep -c . || true)
    if [ "$FMT_OUT" -eq 0 ]; then
        pass "Format check passed"
    else
        fail "Format check failed (${FMT_OUT} files need formatting)"
    fi
else
    warn "gofmt not available, skipping format check"
fi

# ---------------------------------------------------------------------------
# Security Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Security Audit ---"
if command -v govulncheck &>/dev/null && [ -f "go.mod" ]; then
    VULN_OUT=$(govulncheck ./... 2>&1 || true)
    if echo "$VULN_OUT" | grep -q "No vulnerabilities found"; then
        pass "No known vulnerabilities"
    else
        warn "govulncheck reported findings (review manually)"
    fi
else
    warn "govulncheck not available, skipping vulnerability audit"
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

echo -e "${GREEN}All CI checks passed.${NC}"
exit 0
