#!/usr/bin/env bash
# ============================================================================
# stage_cancellation_proofing.sh — CI stage wrapper for Cancellation proofing
#
# Runs all Cancellation proofing scripts in sequence.
# Designed to be called by run_hardening_stages.sh.
#
# Usage:
#   bash stage_cancellation_proofing.sh          # Run all checks
#   bash stage_cancellation_proofing.sh --help   # Show this help
#
# Exit codes:
#   0 — All proofing checks pass
#   1 — One or more proofing checks fail
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
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
echo "  Cancellation Proofing Stage"
echo "============================================"
echo ""

echo "--- 1/3: Contract Implementation Check ---"
if (cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/check_cancellation_contracts.sh") 2>/dev/null; then
    pass "check_cancellation_contracts.sh"
else
    fail "check_cancellation_contracts.sh"
fi

echo ""
echo "--- 2/3: Coverage Threshold Check ---"
if (cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/check_cancellation_coverage.sh") 2>/dev/null; then
    pass "check_cancellation_coverage.sh"
else
    fail "check_cancellation_coverage.sh"
fi

echo ""
echo "--- 3/3: CI Validation ---"
if (cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/../validate-ci.sh") 2>/dev/null; then
    pass "validate-ci.sh"
else
    fail "validate-ci.sh"
fi

echo ""
echo "============================================"
echo "  Cancellation Proofing Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
echo ""

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}Proofing checks failed.${NC}"
    exit 1
else
    echo -e "${GREEN}All proofing checks passed.${NC}"
    exit 0
fi
