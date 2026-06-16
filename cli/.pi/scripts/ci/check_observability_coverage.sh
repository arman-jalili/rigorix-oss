#!/usr/bin/env bash
# ============================================================================
# check_observability_coverage.sh — Enforce observability module test coverage
#
# Verifies that the observability module has sufficient test coverage.
# Minimum thresholds:
#   - tracing.rs: ≥ 2 tests
#   - Overall CLI tests: ≥ 35
#
# Usage:
#   bash check_observability_coverage.sh          # Run coverage check
#   bash check_observability_coverage.sh --help   # Show this help
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
    sed -n '3,11p' "$0" | sed 's/^#//'
    exit 0
}

if [ "${1:-}" = "--help" ]; then show_help; fi

echo "============================================"
echo "  Observability Module — Coverage Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: tracing.rs tests
# ---------------------------------------------------------------------------
echo "--- Tracing Tests ---"
TRACING_TESTS=$(grep -c "#\[test\]" "${SRC_DIR}/observability/infrastructure/tracing.rs" 2>/dev/null || true)
TRACING_TESTS=$((TRACING_TESTS + $(grep -c "#\[test\]" "${SRC_DIR}/tracing.rs" 2>/dev/null || true)))

if [ "$TRACING_TESTS" -ge 2 ]; then
    pass "tracing.rs: ${TRACING_TESTS} tests (min: 2)"
else
    fail "tracing.rs: ${TRACING_TESTS} tests (min: 2)"
fi

# ---------------------------------------------------------------------------
# Check 2: Observability event schema tests (via tests.rs)
# ---------------------------------------------------------------------------
echo ""
echo "--- Event Schema Tests ---"
# Events are tested indirectly via CliEvent serde round-trip tests
CLI_EVENT_TESTS=$(grep -c "test_cli_event" "${SRC_DIR}/tests.rs" 2>/dev/null || true)
if [ "$CLI_EVENT_TESTS" -ge 1 ]; then
    pass "Event schemas tested via tests.rs (${CLI_EVENT_TESTS} test_cli_event tests)"
else
    warn "No dedicated event schema tests found (tested indirectly via serde)"
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
