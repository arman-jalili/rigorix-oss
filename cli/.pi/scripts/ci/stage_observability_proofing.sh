#!/usr/bin/env bash
# ============================================================================
# stage_observability_proofing.sh — CI stage for observability proofing
#
# Runs all observability proofing scripts in sequence.
# Exits 0 if all pass, 1 otherwise.
# Designed to be called by run_hardening_stages.sh.
#
# Usage:
#   bash stage_observability_proofing.sh          # Run all checks
#   bash stage_observability_proofing.sh --help   # Show this help
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

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); }

show_help() {
    sed -n '3,12p' "$0" | sed 's/^#//'
    exit 0
}

if [ "${1:-}" = "--help" ]; then show_help; fi

echo "============================================"
echo "  Observability Proofing Stage"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract Implementation Check
# ---------------------------------------------------------------------------
echo "--- 1/3: Contract Implementation Check ---"
if (cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/check_observability_contracts.sh") 2>/dev/null; then
    pass "check_observability_contracts.sh"
else
    fail "check_observability_contracts.sh"
fi

# ---------------------------------------------------------------------------
# Check 2: Coverage Threshold Check
# ---------------------------------------------------------------------------
echo ""
echo "--- 2/3: Coverage Threshold Check ---"
if (cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/check_observability_coverage.sh") 2>/dev/null; then
    pass "check_observability_coverage.sh"
else
    fail "check_observability_coverage.sh"
fi

# ---------------------------------------------------------------------------
# Check 3: CI Validation (build + tests + lint + format)
# ---------------------------------------------------------------------------
echo ""
echo "--- 3/3: CI Validation ---"
if (cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/../validate-ci.sh") 2>/dev/null; then
    pass "validate-ci.sh"
else
    fail "validate-ci.sh"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Observability Proofing Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
echo ""

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}Observability proofing checks failed.${NC}"
    exit 1
else
    echo -e "${GREEN}All observability proofing checks passed.${NC}"
    exit 0
fi
