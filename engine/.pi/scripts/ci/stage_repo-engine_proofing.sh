#!/usr/bin/env bash
# Stage 22: Repo-Engine Proofing
#
# Runs contract implementation checks and coverage thresholds for the
# repo-engine module. This stage enforces that every frozen contract
# has a concrete implementation and that coverage stays above 80%.
#
# Usage: bash .pi/scripts/ci/stage_repo-engine_proofing.sh
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

FAIL=0
PASS=0

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ((FAIL++)); }

echo ""
echo "═══ Stage 22: Repo-Engine Proofing ═══"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract implementation check
# ---------------------------------------------------------------------------
echo "--- Contract Implementation Check ---"
if bash "${SCRIPT_DIR}/check_repo-engine_contracts.sh" 2>&1; then
    log_pass "All contracts have implementations"
else
    log_fail "Some contracts missing implementations"
fi

# ---------------------------------------------------------------------------
# Check 2: Coverage threshold check
# ---------------------------------------------------------------------------
echo ""
echo "--- Coverage Threshold Check ---"
if bash "${SCRIPT_DIR}/check_repo-engine_coverage.sh" 2>&1; then
    log_pass "Coverage thresholds met"
else
    log_fail "Coverage below threshold"
fi

# ---------------------------------------------------------------------------
# Check 3: Validate all scripts exit properly
# ---------------------------------------------------------------------------
echo ""
echo "--- Script Self-Validation ---"
for script in "$SCRIPT_DIR"/check_repo-engine_*.sh; do
    name=$(basename "$script")
    if [ -f "$script" ] && [ -x "$script" ]; then
        log_pass "Script is executable: $name"
    elif [ -f "$script" ]; then
        log_fail "Script is not executable: $name"
    fi
done

echo ""
echo "═══ Stage 22 Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo "Stage 22 FAILED."
    exit 1
fi

echo "Stage 22 PASSED."
exit 0
