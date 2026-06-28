#!/usr/bin/env bash
# ============================================================================
# stage_planning-pipeline_proofing.sh
#
# CI stage wrapper that runs all planning-pipeline proofing checks:
#   1. Contract implementation check — all interfaces have implementations
#   2. Coverage threshold check — meets minimum coverage
#   3. Script self-validation — all scripts are executable
#
# Usage: bash .pi/scripts/ci/stage_planning-pipeline_proofing.sh [--help]
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
echo "║     Planning-Pipeline Proofing Stage          ║"
echo "╚══════════════════════════════════════════════╝"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract Implementation Check
# ---------------------------------------------------------------------------
echo "--- 1. Contract Implementation Check ---"
echo ""

if bash "${SCRIPT_DIR}/check_planning-pipeline_contracts.sh" 2>&1; then
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

if bash "${SCRIPT_DIR}/check_planning-pipeline_coverage.sh" 2>&1; then
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

for script in "$SCRIPT_DIR"/check_planning-pipeline_*.sh; do
    name=$(basename "$script")
    if [ -f "$script" ] && [ -x "$script" ]; then
        log_pass "Script is executable: $name"
    elif [ -f "$script" ]; then
        chmod +x "$script"
        if [ -x "$script" ]; then
            log_pass "Script made executable: $name"
        else
            log_fail "Script is not executable: $name"
        fi
    fi
done

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
    echo "Planning-pipeline proofing stage FAILED."
    exit 1
fi

echo "Planning-pipeline proofing stage PASSED."
exit 0
