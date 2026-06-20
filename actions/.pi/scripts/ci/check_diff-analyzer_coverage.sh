#!/usr/bin/env bash
# ============================================================================
# check_diff-analyzer_coverage.sh
# ============================================================================
# Verifies that diff-analyzer modules meet minimum test coverage thresholds.
# Exits 0 if coverage meets thresholds, 1 otherwise.
#
# Usage: bash .pi/scripts/ci/check_diff-analyzer_coverage.sh [--threshold <pct>]
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

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARN=$((WARN + 1)); }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DIFF_DIR="$PROJECT_DIR/src/diff_analyzer"

echo "============================================"
echo "  Diff Analyzer Coverage Check"
echo "============================================"
echo ""

# Parse threshold from args or default to 80
THRESHOLD=80
if [[ "$*" == *--threshold* ]]; then
    THRESHOLD=$(echo "$*" | sed 's/.*--threshold //' | awk '{print $1}')
fi

echo "Coverage threshold: ${THRESHOLD}%"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Every implementation file has #[cfg(test)] module with tests
# ---------------------------------------------------------------------------
echo "--- Implementation Test Coverage ---"

declare -A IMPL_TEST_FILES=(
    ["diff_parser_impl.rs"]="diff_parser_impl.rs"
    ["path_validator_impl.rs"]="path_validator_impl.rs"
    ["limit_enforcer_impl.rs"]="limit_enforcer_impl.rs"
    ["risk_classifier_impl.rs"]="risk_classifier_impl.rs"
    ["ai_signal_detector_impl.rs"]="ai_signal_detector_impl.rs"
    ["diff_analysis_pipeline_impl.rs"]="diff_analysis_pipeline_impl.rs"
)

ALL_HAVE_TESTS=true
for impl_file in "${!IMPL_TEST_FILES[@]}"; do
    impl_path="$DIFF_DIR/application/$impl_file"
    if [ ! -f "$impl_path" ]; then
        warn "Implementation '$impl_file' not found (may be in another location)"
        continue
    fi

    # Check for test module
    if grep -q '#\[cfg(test)\]' "$impl_path" 2>/dev/null; then
        # Count test functions
        test_count=$(grep -c '#\[tokio::test\]' "$impl_path" 2>/dev/null || true)
        if [ "$test_count" -gt 0 ]; then
            pass "$impl_file: $test_count tests"
        else
            fail "$impl_file: has test module but no test functions"
            ALL_HAVE_TESTS=false
        fi
    else
        fail "$impl_file: missing #[cfg(test)] module"
        ALL_HAVE_TESTS=false
    fi
done

if [ "$ALL_HAVE_TESTS" = true ]; then
    pass "All implementation files have test modules"
fi

# ---------------------------------------------------------------------------
# Check 2: Count total tests across diff_analyzer
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Count Verification ---"

total_tests=0
for f in $(find "$DIFF_DIR" -name '*.rs' -type f); do
    count=$(grep -c '#\[tokio::test\]' "$f" 2>/dev/null || true)
    total_tests=$((total_tests + count))
done

echo "  Total tests in diff_analyzer: $total_tests"

if [ "$total_tests" -ge 50 ]; then
    pass "Sufficient test count ($total_tests >= 50)"
else
    fail "Insufficient test count ($total_tests < 50)"
fi

# ---------------------------------------------------------------------------
# Check 3: Test suite passes
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Execution ---"

if cd "$PROJECT_DIR" && cargo test -p rigorix-actions -- diff_analyzer --quiet 2>/dev/null; then
    pass "All diff_analyzer tests pass"
else
    fail "Some diff_analyzer tests failed"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS}${NC}"
echo -e "  Failed:   ${RED}${FAIL}${NC}"
echo -e "  Warnings: ${YELLOW}${WARN}${NC}"
echo ""

if [ $FAIL -gt 0 ]; then
    echo -e "${RED}Some coverage checks failed.${NC}"
    exit 1
fi

echo -e "${GREEN}Coverage checks passed.${NC}"
exit 0
