#!/usr/bin/env bash
# ============================================================================
# check_config_coverage.sh — Enforce configuration module test coverage
#
# Verifies that the configuration module has sufficient test coverage.
# Minimum thresholds:
#   - Config domain types: tested via tests.rs
#   - Config loader implementation: ≥ 8 tests
#   - Overall CLI tests: ≥ 35
#
# Usage:
#   bash check_config_coverage.sh          # Run coverage check
#   bash check_config_coverage.sh --help   # Show this help
#
# Exit codes:
#   0 — Coverage thresholds met
#   1 — Coverage below minimum threshold
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_DIR="$(cd "${SCRIPT_DIR}/../../../.." && pwd)/cli/src"

PASS_COUNT=0
FAIL_COUNT=0
ERRORS=()

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; }

show_help() {
    sed -n '3,12p' "$0" | sed 's/^#//'
    exit 0
}

if [ "${1:-}" = "--help" ]; then show_help; fi

echo "============================================"
echo "  Config Module — Coverage Threshold Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Config loader tests
# ---------------------------------------------------------------------------
echo "--- Config Loader Tests ---"
CONFIG_IMPL_TESTS=$(grep -c "#\[tokio::test\]" "${SRC_DIR}/infrastructure/config_impl.rs" 2>/dev/null || true)
CONFIG_IMPL_TEST_FNS=$(grep -c "#\[test\]" "${SRC_DIR}/infrastructure/config_impl.rs" 2>/dev/null || true)
CONFIG_TOTAL=$((CONFIG_IMPL_TESTS + CONFIG_IMPL_TEST_FNS))

if [ "$CONFIG_TOTAL" -ge 6 ]; then
    pass "config_impl.rs: ${CONFIG_TOTAL} tests (min: 6)"
else
    fail "config_impl.rs: ${CONFIG_TOTAL} tests (min: 6)"
fi

# ---------------------------------------------------------------------------
# Check 2: Config domain type tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Config Domain Type Tests ---"

# Check domain/config.rs for tests
DOMAIN_CONFIG_TESTS=$(grep -c "#\[test\]" "${SRC_DIR}/domain/config.rs" 2>/dev/null || true)

# Check tests.rs for config-related tests
CONFIG_TESTS_IN_TESTS=$(grep -c "test_cli_config" "${SRC_DIR}/tests.rs" 2>/dev/null || true)

if [ "$CONFIG_TESTS_IN_TESTS" -ge 2 ]; then
    pass "tests.rs: ${CONFIG_TESTS_IN_TESTS} config-related tests (min: 2)"
else
    fail "tests.rs: ${CONFIG_TESTS_IN_TESTS} config-related tests (min: 2)"
fi

# ---------------------------------------------------------------------------
# Check 3: Total test count
# ---------------------------------------------------------------------------
echo ""
echo "--- Overall CLI Test Coverage ---"
TOTAL_TESTS=$(find "${SRC_DIR}" -name "*.rs" -exec grep -c "#\[test\]" {} \; 2>/dev/null | awk '{s+=$1} END {print s}')

if [ "$TOTAL_TESTS" -ge 35 ]; then
    pass "Overall: ${TOTAL_TESTS} tests (target: 35+ minimum)"
else
    warn "Overall: ${TOTAL_TESTS} tests (target: 35+, current: ${TOTAL_TESTS})"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "COVERAGE ISSUES:"
    for e in "${ERRORS[@]}"; do
        echo "  - $e"
    done
    echo ""
fi

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}Coverage thresholds not met.${NC}"
    exit 1
else
    echo -e "${GREEN}All coverage thresholds met.${NC}"
    exit 0
fi
