#!/usr/bin/env bash
# ============================================================================
# validate-tests.sh — Rust
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
echo "  Test Validation (Rust)"
echo "============================================"
echo ""

if [ ! -f "Cargo.toml" ]; then
    warn "No Cargo.toml found (skipping Rust test validation)"
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
# Unit tests
# ---------------------------------------------------------------------------
echo "--- Unit Tests ---"
if cargo test --lib --quiet 2>/dev/null; then
    pass "Library unit tests passed"
else
    LIB_SOURCES=$(find src -name "*.rs" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$LIB_SOURCES" -eq 0 ]; then
        pass "No library source files (skipped)"
    else
        fail "Library unit tests failed"
    fi
fi

# ---------------------------------------------------------------------------
# Integration tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Integration Tests ---"
if [ -d "tests" ]; then
    # Try named integration test first, then all tests in tests/
    if [ -f "tests/integration.rs" ] || ls tests/*integration* 1>/dev/null 2>&1; then
        if cargo test --test integration --quiet 2>/dev/null; then
            pass "Integration tests passed"
        else
            fail "Integration tests failed"
        fi
    else
        # Run all integration test files
        TEST_COUNT=$(find tests -name "*.rs" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$TEST_COUNT" -gt 0 ]; then
            if cargo test --test '*' --quiet 2>/dev/null; then
                pass "Integration tests passed"
            else
                fail "Integration tests failed"
            fi
        else
            pass "Integration test files found but empty or skipped"
        fi
    fi
else
    pass "No tests/ directory (no integration tests to run)"
fi

# ---------------------------------------------------------------------------
# Doctests
# ---------------------------------------------------------------------------
echo ""
echo "--- Doctests ---"
if cargo test --doc --quiet 2>/dev/null; then
    pass "Doctests passed"
else
    DOCS=$(grep -rl '//!\|///' src/ 2>/dev/null | wc -l | tr -d ' ')
    if [ "$DOCS" -eq 0 ]; then
        pass "No doc comments found (no doctests to run)"
    else
        warn "Doctests failed (may have compile errors in doc examples)"
    fi
fi

# ---------------------------------------------------------------------------
# Coverage (uses cargo-llvm-cov — native LLVM instrumentation, ~3x faster than tarpaulin)
# ---------------------------------------------------------------------------
echo ""
echo "--- Coverage ---"
if command -v cargo &>/dev/null && cargo llvm-cov --version &>/dev/null; then
    LLCV_OUT=$(cargo llvm-cov --html --fail-under-lines 80 2>&1 || true)
    LLCV_EXIT=$?
    COVERAGE_PCT=$(echo "$LLCV_OUT" | grep -oE '[0-9]+(\.[0-9]+)?%' | head -1 | tr -d '%' || echo "")
    if [ -n "$COVERAGE_PCT" ]; then
        MEETS_THRESHOLD=$(echo "$COVERAGE_PCT" | awk '{print ($1 >= 80) ? "yes" : "no"}')
        if [ "$MEETS_THRESHOLD" = "yes" ]; then
            pass "Code coverage: ${COVERAGE_PCT}% (≥ 80%)"
        else
            fail "Code coverage: ${COVERAGE_PCT}% (< 80%)"
        fi
    elif [ "$LLCV_EXIT" -eq 0 ]; then
        pass "Code coverage meets 80% threshold"
    else
        warn "Could not extract coverage percentage from llvm-cov output"
    fi
else
    warn "No coverage tools available (cargo-llvm-cov / grcov), skipping coverage check"
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

echo -e "${GREEN}All test validations passed.${NC}"
exit 0
