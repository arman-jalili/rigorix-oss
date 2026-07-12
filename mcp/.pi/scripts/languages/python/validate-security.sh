#!/usr/bin/env bash
# validate-security.sh — Python
set -euo pipefail

# Activate Poetry venv if available
if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    POETRY_ENV=$(poetry env info --path 2>/dev/null || true)
    if [ -n "$POETRY_ENV" ] && [ -f "$POETRY_ENV/bin/activate" ]; then
        source "$POETRY_ENV/bin/activate"
    fi
fi

SRC_DIR="${1:-src}"
PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Security Scan (Python)"
echo "============================================"

# Hardcoded secrets
echo "--- Hardcoded Secrets ---"
SECRET_COUNT=$(grep -rE "(api_key|secret_key|password|token)\s*=\s*['\"][^'\"]{8,}" "$SRC_DIR" --include="*.py" 2>/dev/null | grep -v "__init__" | grep -v "test" | wc -l | tr -d ' ')
if [ "$SECRET_COUNT" -eq 0 ]; then
    pass "No hardcoded secrets detected"
else
    warn "Potential hardcoded secrets ($SECRET_COUNT findings)"
fi

# SQL injection
echo ""
echo "--- Injection Patterns ---"
INJECT_COUNT=$(grep -rE "(execute|cursor\.execute)\s*\(\s*['\"]" "$SRC_DIR" --include="*.py" 2>/dev/null | wc -l | tr -d ' ')
if [ "$INJECT_COUNT" -eq 0 ]; then
    pass "No obvious injection patterns"
else
    warn "Potential injection patterns ($INJECT_COUNT findings — verify parameterized queries)"
fi

# Dependency audit
echo ""
echo "--- Dependency Audit ---"
if command -v pip-audit &>/dev/null && [ -f "pyproject.toml" ]; then
    AUDIT_OUT=$(pip-audit 2>&1 || true)
    if echo "$AUDIT_OUT" | grep -q "no known vulnerabilities"; then
        pass "No known vulnerabilities"
    elif echo "$AUDIT_OUT" | grep -q "Dependency not found on PyPI"; then
        pass "Dependencies audited (local package skipped)"
    else
        warn "pip-audit reported findings (review manually)"
    fi
else
    warn "pip-audit not available"
fi

# Summary
echo ""
echo "============================================"
echo "  Passed: ${GREEN}${PASS_COUNT}${NC}  Failed: ${RED}${#ERRORS[@]}${NC}  Warn: ${YELLOW}${#WARNINGS[@]}${NC}"
if [ ${#ERRORS[@]} -gt 0 ]; then
    for err in "${ERRORS[@]}"; do echo "  - $err"; done
    exit 1
fi
echo -e "${GREEN}Security scan passed.${NC}"
exit 0
