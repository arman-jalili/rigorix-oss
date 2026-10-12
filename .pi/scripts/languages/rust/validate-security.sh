#!/usr/bin/env bash
# ============================================================================
# validate-security.sh — Rust
#
# Tree- and cwd-agnostic. The same script is installed in every tree
# (.pi/, engine/.pi/, mcp/.pi/, cli/.pi/, actions/.pi/) and resolves which
# crate(s) it owns from its OWN path, never from the caller's cwd.
#
# Why: the hardening workflow discovers `*/.pi/scripts/validate-*.sh` and runs
# each one from the repo root WITHOUT cd-ing into the crate. A script that
# assumed cwd == crate root therefore aborted (exit 1, truncated output, no
# finding) — the failure looked like a result. Resolving from the script's own
# location makes both invocations identical:
#
#   * tree root is a crate (engine/, mcp/, …)  -> validate that crate
#   * tree root is the workspace (repo root)   -> validate every crate with src/
#
# Advisory by design: findings WARN and do not fail the build. The gating
# dependency audit and the repo-wide secret scan run directly in the ci.yml
# `audit` job. See .pi/guards.tsv for which guards gate.
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

# ---------------------------------------------------------------------------
# Resolve the crate(s) this tree owns — from this script's own location.
# Layout: <tree>/.pi/scripts/languages/rust/validate-security.sh
# ---------------------------------------------------------------------------
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TREE_ROOT="$(cd "${SELF_DIR}/../../../.." && pwd)"

CRATES=""
if [ -f "$TREE_ROOT/Cargo.toml" ] && [ -d "$TREE_ROOT/src" ]; then
    # Single-crate tree (engine/, mcp/, cli/, actions/).
    CRATES="$TREE_ROOT"
else
    # Workspace tree (repo root): every direct child that is a crate.
    for d in "$TREE_ROOT"/*/; do
        if [ -f "${d}Cargo.toml" ] && [ -d "${d}src" ]; then
            if [ -z "$CRATES" ]; then CRATES="${d%/}"; else CRATES="${CRATES}
${d%/}"; fi
        fi
    done
fi

if [ -z "$CRATES" ]; then
    warn "No Rust crate (Cargo.toml + src/) found under ${TREE_ROOT} — skipping"
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

echo "--- Scope ---"
printf '%s\n' "$CRATES" | while IFS= read -r c; do
    [ -n "$c" ] && echo "  ${c#"$TREE_ROOT"/}"
done
echo ""

# ---------------------------------------------------------------------------
# Aggregate over every crate in scope
# ---------------------------------------------------------------------------
SECRETS_FOUND=0
SQL_INJECT=0
UNSAFE_BLOCKS=0
PANIC_CALLS=0
CRYPTO_MODERN=0
CRYPTO_OPENSSL=0

while IFS= read -r cdir; do
    [ -n "$cdir" ] || continue

    n=$(grep -rnE '=\s*"[A-Za-z0-9+/=_\-]{20,}"' --include="*.rs" "$cdir/src" 2>/dev/null \
        | grep -v '_test\.rs' \
        | grep -v '#\[cfg(test)\]' \
        | grep -v 'mod tests' \
        | grep -v 'interfaces/http/' \
        | grep -v 'observability/metrics/' \
        | grep -v 'AKIAIOSFODNN7EXAMPLE' \
        | grep -v 'ghp_secret12345' \
        | wc -l | tr -d ' ' || true)
    SECRETS_FOUND=$((SECRETS_FOUND + n))

    if grep -qE 'sqlx' "$cdir/Cargo.toml" 2>/dev/null; then
        n=$(grep -rE 'query\s*\(\s*(format!|format_args!)' "$cdir/src" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ' || true)
        SQL_INJECT=$((SQL_INJECT + n))
    fi

    n=$(grep -rE 'unsafe\s*\{' --include="*.rs" "$cdir/src" 2>/dev/null | wc -l | tr -d ' ' || true)
    UNSAFE_BLOCKS=$((UNSAFE_BLOCKS + n))

    n=$(grep -rE '\.unwrap\(\)|\.expect\(' --include="*.rs" "$cdir/src" 2>/dev/null \
        | grep -v '#\[cfg(test)\]' \
        | grep -v 'mod tests' \
        | grep -v '_test\.rs' \
        | wc -l | tr -d ' ' || true)
    PANIC_CALLS=$((PANIC_CALLS + n))

    grep -qE '^(ring|rustls)\s*=' "$cdir/Cargo.toml" 2>/dev/null && CRYPTO_MODERN=$((CRYPTO_MODERN + 1)) || true
    grep -qE '^openssl\s*=' "$cdir/Cargo.toml" 2>/dev/null && CRYPTO_OPENSSL=$((CRYPTO_OPENSSL + 1)) || true
done <<< "$CRATES"

# ---------------------------------------------------------------------------
# Hardcoded secrets
# ---------------------------------------------------------------------------
echo "--- Hardcoded Secrets ---"
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
if [ "$SQL_INJECT" -gt 0 ]; then
    fail "Potential SQL injection patterns found ($SQL_INJECT occurrences — avoid format! inside query)"
else
    pass "No SQL injection patterns detected"
fi

# ---------------------------------------------------------------------------
# Dependency audit (workspace-level: cargo resolves the lockfile upward)
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Audit ---"
if command -v cargo &>/dev/null && cargo audit --version &>/dev/null; then
    AUDIT_OUT=$(cd "$TREE_ROOT" && cargo audit 2>&1 || true)
    if echo "$AUDIT_OUT" | grep -qE "No advisabilities found|No known vulnerabilities"; then
        pass "No known vulnerabilities (cargo audit)"
    else
        warn "cargo audit reported findings (review manually)"
    fi
elif command -v cargo &>/dev/null && cargo deny --version &>/dev/null; then
    if (cd "$TREE_ROOT" && cargo deny check 2>/dev/null); then
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
if [ "$PANIC_CALLS" -gt 0 ]; then
    warn "Panic-prone patterns found ($PANIC_CALLS .unwrap()/.expect() calls in non-test code)"
    echo "         (production panic surface is ratcheted by validate-panic-sites.sh)"
else
    pass "No panic-prone patterns in production code"
fi

# ---------------------------------------------------------------------------
# Cryptographic practices
# ---------------------------------------------------------------------------
echo ""
echo "--- Cryptographic Practices ---"
if [ "$CRYPTO_MODERN" -gt 0 ]; then
    pass "Modern crypto libraries detected (ring/rustls)"
elif [ "$CRYPTO_OPENSSL" -gt 0 ]; then
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
