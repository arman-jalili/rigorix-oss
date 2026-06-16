#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Rust
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
echo "  CI/MR Validation (Rust)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Cargo.toml detection
# ---------------------------------------------------------------------------
echo "--- Project Detection ---"
if [ -f "Cargo.toml" ]; then
    pass "Cargo.toml found"
    # Extract workspace info if available
    if grep -q '^\[workspace\]' Cargo.toml; then
        echo "  Workspace project detected"
    fi
else
    fail "No Cargo.toml found (not a Rust project)"
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
    fi
    exit 1
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo ""
echo "--- Build ---"
if cargo build --package rigorix --quiet 2>/dev/null; then
    pass "Build succeeded"
else
    fail "Build failed"
fi

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"
if cargo test --package rigorix --quiet 2>/dev/null; then
    pass "All tests passed"
else
    fail "Tests failed"
fi

# ---------------------------------------------------------------------------
# Lint
# ---------------------------------------------------------------------------
echo ""
echo "--- Lint ---"
if command -v cargo &>/dev/null && cargo clippy --version &>/dev/null; then
    if cargo clippy --package rigorix --all-targets -- -D warnings 2>/dev/null; then
        pass "Clippy passed"
    else
        fail "Clippy found issues"
    fi
else
    warn "cargo clippy not available, skipping lint"
fi

# ---------------------------------------------------------------------------
# Format
# ---------------------------------------------------------------------------
echo ""
echo "--- Format ---"
if command -v cargo &>/dev/null && cargo fmt --version &>/dev/null; then
    if cargo fmt --check --package rigorix 2>/dev/null; then
        pass "Format check passed"
    else
        fail "Format check failed (run 'cargo fmt')"
    fi
else
    warn "cargo fmt not available, skipping format check"
fi

# ---------------------------------------------------------------------------
# Security Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Security Audit ---"
if command -v cargo &>/dev/null && cargo audit --version &>/dev/null; then
    AUDIT_OUT=$(cargo audit 2>&1 || true)
    if echo "$AUDIT_OUT" | grep -q "No advisabilities found\|No known vulnerabilities\|info"; then
        pass "No known vulnerabilities"
    else
        warn "cargo audit reported findings (review manually)"
    fi
elif command -v cargo &>/dev/null && cargo deny --version &>/dev/null; then
    if cargo deny check 2>/dev/null; then
        pass "cargo deny passed"
    else
        warn "cargo deny reported issues (review manually)"
    fi
else
    warn "No Rust audit tools available (cargo audit / cargo deny), skipping vulnerability audit"
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
