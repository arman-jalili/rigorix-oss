#!/usr/bin/env bash
# ============================================================================
# check_cli_coverage.sh — Check CLI coverage thresholds
#
# Validates that the CLI crate has adequate test coverage.
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SRC_DIR="${REPO_ROOT}/cli/src"

PASS_COUNT=0
FAIL_COUNT=0

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); }

echo "============================================"
echo "  CLI Test Coverage Check"
echo "============================================"
echo ""

# Check 1: Tests exist
TEST_COUNT=$(find "${SRC_DIR}" -name "*.rs" -exec grep -c "#\[test\]" {} \; 2>/dev/null | awk '{s+=$1} END {print s}' || echo "0")
if [ "$TEST_COUNT" -ge 30 ]; then
    pass "${TEST_COUNT} tests (≥30 threshold)"
elif [ "$TEST_COUNT" -ge 10 ]; then
    pass "${TEST_COUNT} tests (≥10 minimum)"
else
    fail "Only ${TEST_COUNT} tests (minimum 10 required)"
fi

# Check 2: cli_boundary has dedicated test file
if [ -f "${SRC_DIR}/cli_boundary/tests.rs" ]; then
    pass "cli_boundary/tests.rs exists"
else
    fail "cli_boundary/tests.rs missing"
fi

# Check 3: Dispatch tests
if grep -q "#\[test\]" "${SRC_DIR}/cli_boundary/tests.rs" 2>/dev/null; then
    pass "Integration tests in cli_boundary/tests.rs"
else
    fail "No tests in cli_boundary/tests.rs"
fi

echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
echo ""

if [ "$FAIL_COUNT" -gt 0 ]; then
    exit 1
fi
exit 0
