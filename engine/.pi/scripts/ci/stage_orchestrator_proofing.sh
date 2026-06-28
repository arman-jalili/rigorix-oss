#!/usr/bin/env bash
# ============================================================================
# stage_orchestrator_proofing.sh
#
# CI stage wrapper for orchestrator proofing checks. Runs both contract
# implementation check and coverage threshold check.
#
# Usage: bash .pi/scripts/ci/stage_orchestrator_proofing.sh [--help]
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PASS=0
FAIL=0

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Orchestrator Proofing Stage ═══"
echo ""

# Run contract check
echo "--- Contract Implementation Check ---"
if bash "$SCRIPT_DIR/check_orchestrator_contracts.sh"; then
    log_pass "Contract implementation check passed"
else
    log_fail "Contract implementation check failed"
fi

echo ""
echo "--- Coverage Check ---"
if bash "$SCRIPT_DIR/check_orchestrator_coverage.sh"; then
    log_pass "Coverage check passed"
else
    log_fail "Coverage check failed"
fi

# Summary
echo ""
echo "═══ Stage Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo "Orchestrator proofing stage FAILED."
    exit 1
fi

echo "Orchestrator proofing stage PASSED."
exit 0
