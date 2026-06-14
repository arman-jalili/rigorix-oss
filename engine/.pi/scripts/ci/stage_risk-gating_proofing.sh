#!/usr/bin/env bash
# ============================================================================
# stage_risk-gating_proofing.sh
#
# CI stage wrapper that runs all risk-gating proofing checks:
#   1. Contract implementation check — all interfaces have implementations
#   2. Coverage threshold check — meets minimum coverage
#   3. Script self-validation — all scripts are executable
#
# Usage: bash .pi/scripts/ci/stage_risk-gating_proofing.sh [--help]
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║       Risk-Gating Proofing Stage             ║"
echo "╚══════════════════════════════════════════════╝"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract Implementation Check
# ---------------------------------------------------------------------------
echo "--- 1. Contract Implementation Check ---"
echo ""

if bash "${SCRIPT_DIR}/check_risk-gating_contracts.sh" 2>&1; then
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

if bash "${SCRIPT_DIR}/check_risk-gating_coverage.sh" 2>&1; then
    log_pass "Coverage threshold check passed"
else
    log_fail "Coverage threshold check failed"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 3: Script Self-Validation
# ---------------------------------------------------------------------------
echo "--- 3. Script Self-Validation ---"
echo ""

for script in "$SCRIPT_DIR"/check_risk-gating_*.sh; do
    name=$(basename "$script")
    if [ -f "$script" ] && [ -x "$script" ]; then
        log_pass "Script is executable: $name"
    elif [ -f "$script" ]; then
        # Make executable and recheck
        chmod +x "$script"
        if [ -x "$script" ]; then
            log_pass "Script made executable: $name"
        else
            log_fail "Script is not executable: $name"
        fi
    fi
done

# Self-check: stage script should also be executable
if [ -x "$0" ]; then
    log_pass "Stage script is executable: $(basename "$0")"
else
    chmod +x "$0"
    log_pass "Stage script made executable: $(basename "$0")"
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
    echo "Risk-gating proofing stage FAILED."
    exit 1
fi

echo "Risk-gating proofing stage PASSED."
exit 0
