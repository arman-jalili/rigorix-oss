#!/usr/bin/env bash
# ============================================================================
# stage_effect-commitment_proofing.sh — CI stage wrapper
#
# Runs the deterministic effect-commitment proofing checks:
#   1. Contract implementation check (check_effect-commitment_contracts.sh)
#   2. Coverage: measured by the REAL workspace tool
#      (cargo llvm-cov, .pi/scripts/coverage.sh --gate) in ci.yml Stage 3b.
#      Set EFFECT_COMMITMENT_COVERAGE=1 to also run it inline.
#
# Usage:
#   bash .pi/scripts/ci/stage_effect-commitment_proofing.sh
#   EFFECT_COMMITMENT_COVERAGE=1 bash .pi/scripts/ci/stage_effect-commitment_proofing.sh
#
# Exit: 0 = pass, 1 = fail
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

FAIL=0

echo "  Checking effect-commitment contract implementations..."
if bash "${SCRIPT_DIR}/check_effect-commitment_contracts.sh"; then
    echo "  ✓ effect-commitment contract implementations complete"
else
    echo "  ✗ effect-commitment contract implementation violations"
    FAIL=1
fi

echo ""
echo "  Coverage gate: enforced by ci.yml Stage 3b (cargo llvm-cov, ≥ ${COVERAGE_THRESHOLD:-60}% lines)"
if [[ "${EFFECT_COMMITMENT_COVERAGE:-0}" == "1" ]]; then
    if bash "${PROJECT_ROOT}/.pi/scripts/coverage.sh" --gate; then
        echo "  ✓ workspace coverage gate passed"
    else
        echo "  ✗ workspace coverage gate failed"
        FAIL=1
    fi
else
    echo "  ⊘ inline coverage skipped (EFFECT_COMMITMENT_COVERAGE != 1)"
fi

exit "${FAIL}"
