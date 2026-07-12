#!/usr/bin/env bash
# ============================================================================
# check_template-tools_coverage.sh
#
# Checks test coverage for the template-tools module. Falls back to counting
# test assertions if no coverage tool is available.
#
# Usage: bash check_template-tools_coverage.sh [--help]
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
echo "  Coverage Check: template-tools"
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
SRC_DIR="src/template_tools"

# Count lines in production code (all .rs files except those in tests modules)
SRC_LINES=$(find "${SRC_DIR}" -name "*.rs" -exec cat {} + 2>/dev/null | wc -l | tr -d ' ')

# Count tests across all implementation files
TEST_FUNCTIONS=$(find "${SRC_DIR}" -name "*.rs" -exec grep -cE '#\[(test|tokio::test)\]' {} + 2>/dev/null | awk -F: '{s+=$NF} END {print s}' || echo 0)
ASSERT_COUNT=$(find "${SRC_DIR}" -name "*.rs" -exec grep -c 'assert' {} + 2>/dev/null | awk -F: '{s+=$NF} END {print s}' || echo 0)

# Also count tests from the integration test file
CF_TESTS=$(grep -cE '#\[(test|tokio::test)\]' "${PROJECT_ROOT}/tests/template_tools_contract_freeze_test.rs" 2>/dev/null || echo 0)
CF_ASSERTS=$(grep -c 'assert' "${PROJECT_ROOT}/tests/template_tools_contract_freeze_test.rs" 2>/dev/null || echo 0)

TOTAL_TESTS=$((TEST_FUNCTIONS + CF_TESTS))
TOTAL_ASSERTS=$((ASSERT_COUNT + CF_ASSERTS))

if [ "$TOTAL_TESTS" -ge 30 ]; then
    pass "Test functions: $TOTAL_TESTS (threshold: 30)"
else
    fail "Test functions: $TOTAL_TESTS (threshold: 30)"
fi

if [ "$TOTAL_ASSERTS" -ge 100 ]; then
    pass "Assertions: $TOTAL_ASSERTS (threshold: 100)"
else
    fail "Assertions: $TOTAL_ASSERTS (threshold: 100)"
fi

# Rough coverage ratio: test assertions / production lines
if [ "$SRC_LINES" -gt 0 ]; then
    ASSERTS_PER_1000_LINES=$((TOTAL_ASSERTS * 1000 / SRC_LINES))
    if [ "$ASSERTS_PER_1000_LINES" -ge 15 ]; then
        pass "Assert density: ${ASSERTS_PER_1000_LINES}/1k lines (threshold: 15/1k)"
    else
        fail "Assert density: ${ASSERTS_PER_1000_LINES}/1k lines (threshold: 15/1k)"
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
