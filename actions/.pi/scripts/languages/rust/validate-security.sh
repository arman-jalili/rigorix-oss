#!/usr/bin/env bash
# ============================================================================
# validate-security.sh — Rust
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
echo "  Security Validation (Rust)"
echo "============================================"
echo ""

if [ ! -f "Cargo.toml" ]; then
    warn "No Cargo.toml found (skipping Rust security validation)"
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
# Hardcoded secrets
# ---------------------------------------------------------------------------
echo "--- Hardcoded Secrets ---"
# Search for long literal strings assigned to variables in non-test files
SECRETS_FOUND=$(grep -rnE '=\s*"[A-Za-z0-9+/=_\-]{20,}"' --include="*.rs" src/ 2>/dev/null \
    | grep -v '_test\.rs' \
    | grep -v '#\[cfg(test)\]' \
    | grep -v 'mod tests' \
    | grep -v 'interfaces/http/' \
    | grep -v 'observability/metrics/' \
    | grep -v 'AKIAIOSFODNN7EXAMPLE' \
    | grep -v 'ghp_secret12345' \
    | wc -l | tr -d ' ' || true)
if [ "$SECRETS_FOUND" -gt 0 ]; then
    warn "Possible hardcoded secrets found ($SECRETS_FOUND occurrences — verify manually, likely false positives)"
else
    pass "No hardcoded secrets detected"
fi

# ---------------------------------------------------------------------------
# SQL injection
# ---------------------------------------------------------------------------
echo ""
echo "--- SQL Injection ---"
SQL_INJECT=0
if grep -qE 'sqlx' Cargo.toml 2>/dev/null; then
    # Check for unsafe query patterns: format! inside query! or query()
    SQL_INJECT=$(grep -rE 'query\s*\(\s*(format!|format_args!)' src/ --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
fi
if [ "$SQL_INJECT" -gt 0 ]; then
    fail "Potential SQL injection patterns found ($SQL_INJECT occurrences — avoid format! inside query)"
else
    pass "No SQL injection patterns detected"
fi

# ---------------------------------------------------------------------------
# Dependency audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Audit ---"
if command -v cargo &>/dev/null && cargo audit --version &>/dev/null; then
    AUDIT_OUT=$(cargo audit 2>&1 || true)
    if echo "$AUDIT_OUT" | grep -qE "No advisabilities found|No known vulnerabilities"; then
        pass "No known vulnerabilities (cargo audit)"
    else
        warn "cargo audit reported findings (review manually)"
    fi
elif command -v cargo &>/dev/null && cargo deny --version &>/dev/null; then
    if cargo deny check 2>/dev/null; then
        pass "No known vulnerabilities (cargo deny)"
    else
        warn "cargo deny reported issues (review manually)"
    fi
else
    warn "No Rust audit tools available (cargo audit / cargo deny)"
fi

# ---------------------------------------------------------------------------
# Unsafe code
# ---------------------------------------------------------------------------
echo ""
echo "--- Unsafe Code ---"
UNSAFE_BLOCKS=$(grep -rE 'unsafe\s*\{' --include="*.rs" src/ 2>/dev/null | wc -l | tr -d ' ')
if [ "$UNSAFE_BLOCKS" -gt 0 ]; then
    warn "Unsafe code blocks found ($UNSAFE_BLOCKS occurrences — recommend manual audit)"
else
    pass "No unsafe code blocks detected"
fi

# ---------------------------------------------------------------------------
# Panic-prone patterns
# ---------------------------------------------------------------------------
echo ""
echo "--- Panic-Prone Patterns ---"
PANIC_CALLS=$(grep -rE '\.unwrap\(\)|\.expect\(' --include="*.rs" src/ 2>/dev/null \
    | grep -v '#\[cfg(test)\]' \
    | grep -v 'mod tests' \
    | grep -v '_test\.rs' \
    | wc -l | tr -d ' ')
if [ "$PANIC_CALLS" -gt 0 ]; then
    warn "Panic-prone patterns found ($PANIC_CALLS .unwrap()/.expect() calls in non-test code)"
else
    pass "No panic-prone patterns in production code"
fi

# ---------------------------------------------------------------------------
# Cryptographic practices
# ---------------------------------------------------------------------------
echo ""
echo "--- Cryptographic Practices ---"
if grep -qE '^(ring|rustls)\s*=' Cargo.toml 2>/dev/null; then
    pass "Modern crypto libraries detected (ring/rustls)"
elif grep -qE '^openssl\s*=' Cargo.toml 2>/dev/null; then
    warn "OpenSSL detected (prefer ring or rustls for new projects)"
else
    pass "No cryptographic library dependencies found"
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

echo -e "${GREEN}Security validation completed.${NC}"
exit 0
