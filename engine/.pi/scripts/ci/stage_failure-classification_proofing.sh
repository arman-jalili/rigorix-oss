#!/usr/bin/env bash
# ============================================================================
# stage_failure-classification_proofing.sh
#
# CI stage wrapper that runs all failure-classification proofing checks:
#   1. Contract implementation check — all interfaces have implementations
#   2. Coverage threshold check — meets minimum coverage
#
# Usage: bash .pi/scripts/ci/stage_failure-classification_proofing.sh [--help]
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║  Failure Classification Proofing Stage        ║"
echo "╚══════════════════════════════════════════════╝"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract Implementation Check
# ---------------------------------------------------------------------------
echo "--- 1. Contract Implementation Check ---"
echo ""

if bash "${SCRIPT_DIR}/check_failure-classification_contracts.sh" 2>&1; then
    log_pass "Contract implementation check passed"
else
    log_fail "Contract implementation check failed"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 2: Coverage Threshold Check
# ---------------------------------------------------------------------------
echo "--- 2. Coverage Threshold Check ---"
echo ""

if bash "${SCRIPT_DIR}/check_failure-classification_coverage.sh" 2>&1; then
    log_pass "Coverage threshold check passed"
else
    log_fail "Coverage threshold check failed"
fi

echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "═══ Stage Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Failure-classification proofing stage FAILED."
    exit 1
fi

echo "Failure-classification proofing stage PASSED."
exit 0
