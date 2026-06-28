#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Rust
#
# Runs CI validation for the specific crate in the current directory.
# Uses `-p rigorix-{crate}` to scope build/test/clippy to this crate only,
# avoiding cross-crate interference in workspace projects.
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PARENT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

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

# Detect crate name from Cargo.toml
CRATE_NAME=""
if [ -f "Cargo.toml" ]; then
    CRATE_NAME=$(grep '^name' Cargo.toml | head -1 | sed 's/name = "//;s/"//')
fi

# ── Project Detection ──
echo "--- Project Detection ---"
if [ -f "Cargo.toml" ]; then
    pass "Cargo.toml found (crate: ${CRATE_NAME:-unknown})"
else
    fail "No Cargo.toml found"
    exit 1
fi

SCOPE_FLAG=""
if [[ -n "$CRATE_NAME" ]]; then
    SCOPE_FLAG="-p $CRATE_NAME"
    echo "  Scoping to crate: $CRATE_NAME"
fi

# ── Build ──
echo ""
echo "--- Build ---"
if cargo build $SCOPE_FLAG --quiet 2>/dev/null; then
    pass "Build succeeded"
else
    fail "Build failed"
fi

# ── Tests ──
echo ""
echo "--- Tests ---"
if cargo test $SCOPE_FLAG --quiet 2>/dev/null; then
    pass "All tests passed"
else
    fail "Tests failed"
fi

# ── Lint ──
echo ""
echo "--- Lint ---"
if cargo clippy $SCOPE_FLAG -- -D warnings 2>/dev/null; then
    pass "Clippy passed"
else
    fail "Clippy found issues"
fi

# ── Format ──
echo ""
echo "--- Format ---"
if cargo fmt $SCOPE_FLAG --check 2>/dev/null; then
    pass "Format check passed"
else
    fail "Format check failed"
fi

# ── Summary ──
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
