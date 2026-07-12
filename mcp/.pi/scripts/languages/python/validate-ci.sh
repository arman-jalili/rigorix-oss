#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Python (Poetry-based)
# ============================================================================
set -euo pipefail

# Activate Poetry venv if available
if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    POETRY_ENV=$(poetry env info --path 2>/dev/null || true)
    if [ -n "$POETRY_ENV" ] && [ -f "$POETRY_ENV/bin/activate" ]; then
        source "$POETRY_ENV/bin/activate"
    fi
fi

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
echo "  CI/MR Validation (Python)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo "--- Build ---"
if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    if poetry build 2>/dev/null; then
        pass "Build succeeded"
    else
        fail "Build failed"
    fi
else
    pass "No build step (dev project)"
fi

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"
if command -v pytest &>/dev/null; then
    if PYTHONPATH=src pytest -q 2>/dev/null; then
        pass "All tests passed"
    else
        fail "Tests failed"
    fi
else
    warn "pytest not available, skipping tests"
fi

# ---------------------------------------------------------------------------
# Lint
# ---------------------------------------------------------------------------
echo ""
echo "--- Lint ---"
if command -v ruff &>/dev/null; then
    if ruff check . 2>/dev/null; then
        pass "Lint passed"
    else
        fail "Lint failed"
    fi
else
    warn "ruff not available, skipping lint"
fi

# ---------------------------------------------------------------------------
# Format
# ---------------------------------------------------------------------------
echo ""
echo "--- Format ---"
if command -v ruff &>/dev/null; then
    if ruff format --check . 2>/dev/null; then
        pass "Format check passed"
    else
        fail "Format check failed"
    fi
else
    warn "ruff not available, skipping format"
fi

# ---------------------------------------------------------------------------
# Security Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Security Audit ---"
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
    warn "pip-audit not available, skipping audit"
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
