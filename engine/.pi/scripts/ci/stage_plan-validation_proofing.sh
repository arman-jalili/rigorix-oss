#!/usr/bin/env bash
# ============================================================================
# stage_plan-validation_proofing.sh
#
# CI stage wrapper for plan-validation proofing checks.
# Runs contract validation and coverage enforcement for the plan-validation module.
#
# Usage: bash .pi/scripts/ci/stage_plan-validation_proofing.sh [--help]
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  Stage: plan-validation Proofing"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract Implementation Check
# ---------------------------------------------------------------------------
echo "--- [1/2] Contract Implementation Check ---"
if bash "$SCRIPT_DIR/check_plan-validation_contracts.sh"; then
    echo "  ✓ Contract check passed"
else
    echo "  ✗ Contract check failed"
    exit 1
fi

# ---------------------------------------------------------------------------
# Check 2: Coverage Check
# ---------------------------------------------------------------------------
echo ""
echo "--- [2/2] Coverage Check ---"
if bash "$SCRIPT_DIR/check_plan-validation_coverage.sh"; then
    echo "  ✓ Coverage check passed"
else
    echo "  ✗ Coverage check failed"
    exit 1
fi

# ---------------------------------------------------------------------------
# Stage Result
# ---------------------------------------------------------------------------
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  Stage: plan-validation Proofing — PASSED"
echo "═══════════════════════════════════════════════════════════════"
echo ""
exit 0
