#!/usr/bin/env bash
# ============================================================================
# validate-security.sh — Go
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Security Scan (Go)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Hardcoded Secrets
# ---------------------------------------------------------------------------
echo "--- Hardcoded Secrets ---"
if [ -f "go.mod" ]; then
    SECRET_COUNT=$(grep -rE '(apiKey|secretKey|password|secret|token)\s*=\s*"[^"]{8,}"' . --include="*.go" 2>/dev/null \
        | grep -v "_test.go" \
        | grep -vE '(example|placeholder|dummy|TODO|FIXME)' \
        | wc -l | tr -d ' ')
    if [ "$SECRET_COUNT" -eq 0 ]; then
        pass "No hardcoded secrets detected"
    else
        fail "Potential hardcoded secrets found ($SECRET_COUNT occurrences)"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# SQL/Query Injection
# ---------------------------------------------------------------------------
echo ""
echo "--- Injection Patterns ---"
if [ -f "go.mod" ]; then
    # Check for db.Exec/db.Query combined with fmt.Sprintf (string interpolation into queries)
    INJECT_COUNT=0
    # Find files using db.Exec or db.Query with fmt.Sprintf
    for file in $(grep -rlE '(db\.Exec|db\.Query)\s*\(\s*fmt\.Sprintf' . --include="*.go" 2>/dev/null); do
        INJECT_COUNT=$((INJECT_COUNT + 1))
    done
    if [ "$INJECT_COUNT" -eq 0 ]; then
        pass "No obvious SQL injection patterns (no Exec/Query with Sprintf)"
    else
        fail "Potential SQL injection patterns ($INJECT_COUNT files use Exec/Query with fmt.Sprintf)"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# Dependency Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Audit ---"
if command -v govulncheck &>/dev/null && [ -f "go.mod" ]; then
    VULN_OUT=$(govulncheck ./... 2>&1 || true)
    if echo "$VULN_OUT" | grep -q "No vulnerabilities found"; then
        pass "No known vulnerabilities"
    else
        warn "govulncheck reported findings (review manually)"
    fi
elif [ -f "go.mod" ]; then
    DEP_COUNT=$(go list -m all 2>/dev/null | wc -l | tr -d ' ')
    warn "govulncheck not available; $DEP_COUNT dependencies require manual review"
else
    warn "Not in a Go module, skipping dependency audit"
fi

# ---------------------------------------------------------------------------
# TLS/HTTPS Usage
# ---------------------------------------------------------------------------
echo ""
echo "--- TLS/HTTPS Usage ---"
if [ -f "go.mod" ]; then
    # Check for hardcoded http:// URLs in non-test files (excluding localhost and examples)
    HTTP_URLS=$(grep -rE 'https?://[^"'\''`\s]+' . --include="*.go" 2>/dev/null \
        | grep -v "_test.go" \
        | grep -vE '(localhost|127\.0\.0\.1|example\.com|example\.org)' \
        | grep -c 'http://' || true)
    if [ "$HTTP_URLS" -eq 0 ]; then
        pass "No production http:// URLs found (using https://)"
    else
        warn "Found $HTTP_URLS potential http:// URLs (consider https://)"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# Input Validation (HTML vs Text Template)
# ---------------------------------------------------------------------------
echo ""
echo "--- Input Validation ---"
if [ -f "go.mod" ]; then
    TEXT_TEMPLATE=$(grep -rl '"text/template"' . --include="*.go" 2>/dev/null | grep -v "_test.go" | wc -l | tr -d ' ')
    HTML_TEMPLATE=$(grep -rl '"html/template"' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$TEXT_TEMPLATE" -eq 0 ]; then
        pass "No text/template usage in production code (html/template auto-escapes)"
    elif [ "$HTML_TEMPLATE" -gt 0 ]; then
        warn "Found text/template in $TEXT_TEMPLATE files; html/template used in $HTML_TEMPLATE files (prefer html/template for HTML output)"
    else
        fail "Found text/template in $TEXT_TEMPLATE production files (use html/template for HTML output)"
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

echo -e "${GREEN}Security scan passed.${NC}"
exit 0
