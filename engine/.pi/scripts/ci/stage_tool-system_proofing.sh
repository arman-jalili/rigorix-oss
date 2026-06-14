#!/usr/bin/env bash
# ============================================================================
# stage_tool-system_proofing.sh
#
# CI stage wrapper that runs all tool-system proofing checks.
# Designed to be called from run_hardening_stages.sh.
#
# Usage: bash .pi/scripts/ci/stage_tool-system_proofing.sh
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo ""
echo "=============================================="
echo "  Tool-System Proofing Stage"
echo "=============================================="

# Run contract check
echo ""
echo "--- Stage 1/2: Contract Implementation Check ---"
if bash "$SCRIPT_DIR/check_tool-system_contracts.sh"; then
    echo "  ✓ Contract check passed"
else
    echo "  ✗ Contract check FAILED"
    exit 1
fi

# Run coverage check
echo ""
echo "--- Stage 2/2: Coverage Threshold Check ---"
if bash "$SCRIPT_DIR/check_tool-system_coverage.sh"; then
    echo "  ✓ Coverage check passed"
else
    echo "  ✗ Coverage check FAILED"
    exit 1
fi

echo ""
echo "=============================================="
echo "  Tool-System Proofing: ALL CHECKS PASSED"
echo "=============================================="
exit 0
