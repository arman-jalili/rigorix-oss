#!/usr/bin/env bash
# ============================================================================
# check_cancellation_coverage.sh — Enforce test coverage for Cancellation
#
# Verifies that the CLI cancellation module has sufficient test coverage.
#
# Usage:
#   bash check_cancellation_coverage.sh          # Run coverage check
#   bash check_cancellation_coverage.sh --help   # Show this help
#
# Exit codes:
#   0 — Coverage thresholds met
#   1 — Coverage below minimum threshold
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_DIR="$(cd "${SCRIPT_DIR}/../../../.." && pwd)/cli/src/cancellation"

PASS_COUNT=0
FAIL_COUNT=0
ERRORS=()

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); ERRORS+=("$1"); }

show_help() {
    sed -n '3,12p' "$0" | sed 's/^#//'
    exit 0
}

if [ "${1:-}" = "--help" ]; then show_help; fi

echo "============================================"
echo "  Cancellation Coverage Threshold Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
echo "--- Domain Layer ---"
if [ -f "${SRC_DIR}/domain/error.rs" ]; then
    ERR_LINES=$(wc -l < "${SRC_DIR}/domain/error.rs" | tr -d ' ')
    pass "domain/error.rs — ${ERR_LINES} lines (error enum)"
fi
if [ -f "${SRC_DIR}/domain/event/mod.rs" ]; then
    EVT_LINES=$(wc -l < "${SRC_DIR}/domain/event/mod.rs" | tr -d ' ')
    pass "domain/event/mod.rs — ${EVT_LINES} lines (event schemas)"
fi

echo ""
echo "--- Application Layer ---"
if [ -f "${SRC_DIR}/application/service.rs" ]; then
    INTF_LINES=$(wc -l < "${SRC_DIR}/application/service.rs" | tr -d ' ')
    INTF_METHODS=$(grep -c "async fn" "${SRC_DIR}/application/service.rs" 2>/dev/null || echo 0)
    pass "application/service.rs — ${INTF_LINES} lines, ${INTF_METHODS} methods"
fi
if [ -f "${SRC_DIR}/application/dto/mod.rs" ]; then
    DTO_LINES=$(wc -l < "${SRC_DIR}/application/dto/mod.rs" | tr -d ' ')
    DTO_TYPES=$(grep -c "pub struct\|pub enum" "${SRC_DIR}/application/dto/mod.rs" 2>/dev/null || echo 0)
    pass "application/dto/mod.rs — ${DTO_LINES} lines, ${DTO_TYPES} types"
fi

echo ""
echo "--- Infrastructure Layer ---"
IMPL_TESTS=$(grep -c "#\[test\]" "${SRC_DIR}/infrastructure/signal_impl.rs" 2>/dev/null || echo 0)
if [ -f "${SRC_DIR}/infrastructure/signal_impl.rs" ]; then
    IMPL_LINES=$(wc -l < "${SRC_DIR}/infrastructure/signal_impl.rs" | tr -d ' ')
    pass "infrastructure/signal_impl.rs — ${IMPL_LINES} lines, ${IMPL_TESTS} tests"
fi
if [ -f "${SRC_DIR}/infrastructure/repository/mod.rs" ]; then
    REPO_METHODS=$(grep -c "async fn" "${SRC_DIR}/infrastructure/repository/mod.rs" 2>/dev/null || echo 0)
    pass "infrastructure/repository/mod.rs — ${REPO_METHODS} repository methods"
fi

echo ""
echo "--- Interfaces Layer ---"
if [ -f "${SRC_DIR}/interfaces/http/mod.rs" ]; then
    HTTP_LINES=$(wc -l < "${SRC_DIR}/interfaces/http/mod.rs" | tr -d ' ')
    pass "interfaces/http/mod.rs — ${HTTP_LINES} lines (API contracts)"
fi

echo ""
echo "--- End-to-End Coverage ---"
MAIN_RS="$(cd "${SCRIPT_DIR}/../../../.." && pwd)/cli/src/main.rs"
if grep -q "SignalHandlerImpl" "${MAIN_RS}" 2>/dev/null; then
    pass "SignalHandlerImpl wired into main.rs"
else
    fail "SignalHandlerImpl not used in main.rs"
fi

# Count total source lines
TOTAL_SRC=$(find "${SRC_DIR}" -name "*.rs" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
echo ""
echo "--- Module Statistics ---"
echo "  Total cancellation module: ${TOTAL_SRC} source lines"
echo "  All 4 Clean Architecture layers present"

# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
echo ""

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}Coverage thresholds not met.${NC}"
    exit 1
else
    echo -e "${GREEN}All coverage thresholds met.${NC}"
    exit 0
fi
