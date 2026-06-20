#!/usr/bin/env bash
# ============================================================================
# stage_diff-analyzer_proofing.sh
# ============================================================================
# CI stage wrapper for diff-analyzer proofing checks.
# Runs all diff-analyzer validation scripts and reports pass/fail.
#
# Usage: bash .pi/scripts/ci/stage_diff-analyzer_proofing.sh [--verbose]
#
# This stage runs:
#   1. check_diff-analyzer_contracts.sh - Validate contract implementations
#   2. check_diff-analyzer_coverage.sh  - Verify test coverage thresholds
# ============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
PASS=0
FAIL=0

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "============================================"
echo "  Stage: diff-analyzer Proofing"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract Implementation Validation
# ---------------------------------------------------------------------------
echo "--- 1. Contract Implementation Validation ---"
if bash "$SCRIPT_DIR/check_diff-analyzer_contracts.sh" 2>&1; then
    pass "Contract implementation check passed"
else
    fail "Contract implementation check failed"
fi
echo ""

# ---------------------------------------------------------------------------
# Check 2: Coverage Threshold Validation
# ---------------------------------------------------------------------------
echo "--- 2. Coverage Threshold Validation ---"
if bash "$SCRIPT_DIR/check_diff-analyzer_coverage.sh" 2>&1; then
    pass "Coverage check passed"
else
    fail "Coverage check failed"
fi
echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "============================================"
echo "  Stage Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS}${NC}"
echo -e "  Failed:   ${RED}${FAIL}${NC}"
echo ""

if [ $FAIL -gt 0 ]; then
    echo -e "${RED}Stage FAILED.${NC}"
    exit 1
fi

echo -e "${GREEN}Stage PASSED.${NC}"
exit 0
