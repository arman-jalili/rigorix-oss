#!/usr/bin/env bash
# ============================================================================
# validate-security.sh — TypeScript
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Security Validation (TypeScript)"
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
# Hardcoded Secrets
# ---------------------------------------------------------------------------
echo "--- Hardcoded Secrets ---"
SECRET_COUNT=0
for ext in ts tsx; do
    COUNT=$(find . -name "*.$ext" -not -path "./node_modules/*" -not -path "./.next/*" -not -path "./dist/*" \
        -exec grep -E "(apiKey|api_key|apiKey|secret|password|token|privateKey)\s*[:=]\s*['\"][^'\"]{8,}['\"]" {} + 2>/dev/null | wc -l | tr -d ' ')
    SECRET_COUNT=$((SECRET_COUNT + COUNT))
done
if [ "$SECRET_COUNT" -eq 0 ]; then
    pass "No hardcoded secrets detected"
else
    fail "Potential hardcoded secrets found ($SECRET_COUNT occurrences)"
fi

# ---------------------------------------------------------------------------
# SQL/NoSQL Injection
# ---------------------------------------------------------------------------
echo ""
echo "--- SQL/NoSQL Injection ---"
INJECTION_PATTERNS=0
for ext in ts tsx; do
    # Template literals used with query/exec calls
    COUNT=$(find . -name "*.$ext" -not -path "./node_modules/*" -not -path "./.next/*" -not -path "./dist/*" \
        -exec grep -E "\.(query|exec)\(\s*\`" {} + 2>/dev/null | wc -l | tr -d ' ')
    INJECTION_PATTERNS=$((INJECTION_PATTERNS + COUNT))
done
if [ "$INJECTION_PATTERNS" -eq 0 ]; then
    pass "No SQL/NoSQL injection patterns detected"
else
    warn "Potential injection patterns found ($INJECTION_PATTERNS template literal queries — review manually)"
fi

# ---------------------------------------------------------------------------
# Dependency Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Audit ---"
if [ "$PKG_MGR" = "bun" ] && command -v bun &>/dev/null; then
    AUDIT_OUT=$(bun audit 2>&1 || true)
    CRITICAL=$(echo "$AUDIT_OUT" | grep -ci "critical" 2>/dev/null || echo "0")
    HIGH=$(echo "$AUDIT_OUT" | grep -ci "high" 2>/dev/null || echo "0")
    if [ "$CRITICAL" -gt 0 ] || [ "$HIGH" -gt 0 ]; then
        fail "Dependency audit found critical/high vulnerabilities"
    else
        pass "No critical/high vulnerabilities in dependencies"
    fi
elif command -v npm &>/dev/null; then
    AUDIT_OUT=$(npm audit 2>&1 || true)
    if echo "$AUDIT_OUT" | grep -q "found 0 vulnerabilities" 2>/dev/null; then
        pass "No vulnerabilities in dependencies"
    else
        CRITICAL_HIGH=$(echo "$AUDIT_OUT" | grep -iE "(critical|high).*vulnerab" 2>/dev/null | head -1 || true)
        if [ -n "$CRITICAL_HIGH" ]; then
            fail "Dependency audit found critical/high vulnerabilities"
        else
            pass "No critical/high vulnerabilities in dependencies"
        fi
    fi
else
    warn "No package manager available for dependency audit"
fi

# ---------------------------------------------------------------------------
# eval / Dangerous Patterns
# ---------------------------------------------------------------------------
echo ""
echo "--- eval / Dangerous Patterns ---"
DANGEROUS_COUNT=0
for ext in ts tsx; do
    COUNT=$(find . -name "*.$ext" -not -path "./node_modules/*" -not -path "./.next/*" -not -path "./dist/*" \
        -exec grep -E "(eval\(|innerHTML\s*=|dangerouslySetInnerHTML)" {} + 2>/dev/null | wc -l | tr -d ' ')
    DANGEROUS_COUNT=$((DANGEROUS_COUNT + COUNT))
done
if [ "$DANGEROUS_COUNT" -eq 0 ]; then
    pass "No dangerous patterns (eval/innerHTML/dangerouslySetInnerHTML) found"
else
    warn "Dangerous patterns detected ($DANGEROUS_COUNT occurrences — review for XSS risk)"
fi

# ---------------------------------------------------------------------------
# XSS Prevention
# ---------------------------------------------------------------------------
echo ""
echo "--- XSS Prevention ---"
SANITIZE_LIBS=0
for ext in ts tsx; do
    COUNT=$(find . -name "*.$ext" -not -path "./node_modules/*" -not -path "./.next/*" -not -path "./dist/*" \
        -exec grep -lE "(DOMPurify|sanitize-html|xss|sanitize)" {} + 2>/dev/null | wc -l | tr -d ' ')
    SANITIZE_LIBS=$((SANITIZE_LIBS + COUNT))
done
if [ "$SANITIZE_LIBS" -gt 0 ]; then
    pass "Sanitization libraries detected ($SANITIZE_LIBS files)"
else
    # Only warn if there are TSX files (likely has JSX/DOM rendering)
    TSX_COUNT=$(find . -name "*.tsx" -not -path "./node_modules/*" -not -path "./.next/*" -not -path "./dist/*" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$TSX_COUNT" -gt 0 ]; then
        warn "No sanitization library found in TSX files (consider DOMPurify or sanitize-html)"
    else
        pass "No TSX files, sanitization not applicable"
    fi
fi

# ---------------------------------------------------------------------------
# HTTPS Enforcement
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTPS Enforcement ---"
HTTP_ONLY=0
for ext in ts tsx; do
    COUNT=$(find . -name "*.$ext" -not -path "./node_modules/*" -not -path "./.next/*" -not -path "./dist/*" \
        -exec grep -E "['\"]http://(?!localhost)" {} + 2>/dev/null | grep -ivE "(test|spec|mock|example|dev)" | wc -l | tr -d ' ')
    HTTP_ONLY=$((HTTP_ONLY + COUNT))
done
if [ "$HTTP_ONLY" -eq 0 ]; then
    pass "No hardcoded HTTP URLs (non-localhost) found"
else
    warn "Hardcoded HTTP URLs detected ($HTTP_ONLY occurrences — use HTTPS for production)"
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

echo -e "${GREEN}Security validation passed.${NC}"
exit 0
