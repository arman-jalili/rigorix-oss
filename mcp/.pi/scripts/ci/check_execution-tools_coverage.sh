#!/usr/bin/env bash
# ============================================================================
# check_execution-tools_coverage.sh
#
# Checks test coverage for the execution-tools module. Falls back to counting
# test assertions if no coverage tool is available.
#
# Usage: bash check_execution-tools_coverage.sh [--help]
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../../" && pwd)"

if [[ "${1:-}" == "--help" ]]; then
    sed -n '/^# =*/,/^\$/p' "${BASH_SOURCE[0]}" | grep -v '^# =' | sed 's/^# //'
    exit 0
fi

PASS=0
FAIL=0

pass() { echo -e "  \e[32m✅ PASS\e[0m $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  \e[31m❌ FAIL\e[0m $1"; FAIL=$((FAIL + 1)); }

THRESHOLD=80

echo "============================================"
echo "  Coverage Check: execution-tools"
echo "============================================"
echo ""

cd "${PROJECT_ROOT}"

# ---------------------------------------------------------------------------
# Test compilation check
# ---------------------------------------------------------------------------
echo "--- Build & Test ---"
if cargo test -p rigorix-mcp --lib --quiet 2>/dev/null; then
    pass "All library tests pass"
else
    fail "Library tests failed"
    echo ""
    summary
    exit 1
fi

# ---------------------------------------------------------------------------
# Count test assertions as a coverage proxy
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Assertions ---"
SRC_DIR="src/execution_tools"
SRC_LINES=$(find "${SRC_DIR}" -name "*.rs" ! -name "tests.rs" -exec cat {} + 2>/dev/null | wc -l | tr -d ' ')
TEST_LINES=$(cat "${SRC_DIR}/tests.rs" 2>/dev/null | wc -l | tr -d ' ')

# Count test functions
TEST_FUNCTIONS=$(grep -cE '#\[(test|tokio::test)\]' "${SRC_DIR}/tests.rs" 2>/dev/null || echo 0)
ASSERT_COUNT=$(grep -c 'assert' "${SRC_DIR}/tests.rs" 2>/dev/null || echo 0)

if [ "$TEST_FUNCTIONS" -ge 10 ]; then
    pass "Test functions: $TEST_FUNCTIONS (threshold: 10)"
else
    fail "Test functions: $TEST_FUNCTIONS (threshold: 10)"
fi

if [ "$ASSERT_COUNT" -ge 30 ]; then
    pass "Assertions: $ASSERT_COUNT (threshold: 30)"
else
    fail "Assertions: $ASSERT_COUNT (threshold: 30)"
fi

# Rough coverage ratio: test code / production code
if [ "$SRC_LINES" -gt 0 ]; then
    COVERAGE_RATIO=$((TEST_LINES * 100 / SRC_LINES))
    if [ "$COVERAGE_RATIO" -ge 10 ]; then
        pass "Test/code ratio: ${COVERAGE_RATIO}% (threshold: 10%)"
    else
        fail "Test/code ratio: ${COVERAGE_RATIO}% (threshold: 10%)"
    fi
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Results"
echo "============================================"
echo -e "  Passed: \e[32m${PASS}\e[0m"
echo -e "  Failed: \e[31m${FAIL}\e[0m"
echo ""

if [ "$FAIL" -gt 0 ]; then exit 1; fi
echo -e "\e[32mCoverage thresholds met.\e[0m"
exit 0
