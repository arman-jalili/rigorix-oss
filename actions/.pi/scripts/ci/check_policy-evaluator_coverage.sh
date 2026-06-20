#!/usr/bin/env bash
# ============================================================================
# check_policy-evaluator_coverage.sh
# ============================================================================
# Verifies that policy-evaluator modules meet minimum test coverage thresholds.
# Exits 0 if coverage meets thresholds, 1 otherwise.
#
# Usage: bash .pi/scripts/ci/check_policy-evaluator_coverage.sh [--threshold <pct>]
#
# This script checks:
#   1. That each implementation file has a corresponding test module
#   2. That test count per module meets a minimum threshold
#   3. That the overall test suite passes
# ============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
PASS=0
FAIL=0
WARN=0

pass() { echo -e "${GREEN}OK${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}FAIL${NC} $1"; FAIL=$((FAIL + 1)); }
warn() { echo -e "${YELLOW}WARN${NC} $1"; WARN=$((WARN + 1)); }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PE_DIR="$PROJECT_DIR/src/policy_evaluator"

echo "============================================"
echo "  Policy Evaluator Coverage Check"
echo "============================================"
echo ""

THRESHOLD=80
if [[ "$*" == *--threshold* ]]; then
    THRESHOLD=$(echo "$*" | sed 's/.*--threshold //' | awk '{print $1}')
fi

echo "Coverage threshold: ${THRESHOLD}%"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Every implementation file has a test module
# ---------------------------------------------------------------------------
echo "--- Implementation Test Coverage ---"

IMPL_FILES="policy_document_factory_impl rules_factory_impl compiled_rules_factory_impl policy_result_factory_impl policy_loader_impl policy_tamper_detector_impl policy_evaluator_impl org_policy_merger_impl policy_report_generator_impl policy_evaluation_pipeline_impl"

for file in $IMPL_FILES; do
    filepath="$PE_DIR/application/${file}.rs"
    if [ ! -f "$filepath" ]; then
        fail "Implementation file missing: ${file}.rs"
        continue
    fi

    if grep -q '#\[cfg(test)\]' "$filepath" 2>/dev/null; then
        test_count=$(grep -c '#\[tokio::test\]' "$filepath" 2>/dev/null || echo 0)
        if [ "$test_count" -ge 1 ]; then
            pass "${file}.rs -- $test_count test(s)"
        else
            warn "${file}.rs -- test module exists but no test functions found"
        fi
    else
        fail "${file}.rs -- missing test module"
    fi
done

echo ""

# ---------------------------------------------------------------------------
# Check 2: Total test count meets threshold
# ---------------------------------------------------------------------------
echo "--- Total Test Coverage ---"

total_tests=0
for file in "$PE_DIR/application"/*_impl.rs; do
    count=$(grep -c '#\[tokio::test\]' "$file" 2>/dev/null || echo 0)
    total_tests=$((total_tests + count))
done

echo "Total test functions: $total_tests"
if [ "$total_tests" -ge 20 ]; then
    pass "Test count ($total_tests) meets minimum threshold (20)"
else
    fail "Test count ($total_tests) below minimum threshold (20)"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 3: Test suite passes
# ---------------------------------------------------------------------------
echo "--- Test Suite ---"

cd "$PROJECT_DIR"
if cargo test --lib policy_evaluator 2>&1 | tail -3 | grep -q "test result: ok"; then
    pass "All policy-evaluator tests pass"
else
    fail "Some policy-evaluator tests failed"
fi

echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS}${NC}"
echo -e "  Failed:   ${RED}${FAIL}${NC}"
echo -e "  Warnings: ${YELLOW}${WARN}${NC}"
echo ""

if [ $FAIL -gt 0 ]; then
    echo -e "${RED}Coverage check FAILED.${NC}"
    exit 1
fi

echo -e "${GREEN}Coverage check PASSED.${NC}"
exit 0
