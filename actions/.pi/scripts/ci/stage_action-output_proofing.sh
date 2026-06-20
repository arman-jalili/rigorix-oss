#!/usr/bin/env bash
# Action Output Proofing Stage
#
# Validates the action-output module's contract implementation and coverage.
# Called by run_hardening_stages.sh.
#
# Usage: bash .pi/scripts/ci/stage_action-output_proofing.sh [--verbose]
#
# Exit codes:
#   0 — All checks pass
#   1 — One or more checks failed

set -euo pipefail

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "  action-output Contract Implementation Check..."
if $SCRIPT_DIR/check_action-output_contracts.sh ${VERBOSE:+--verbose}; then
    echo "  ✅ Contracts check passed"
else
    echo "  ❌ Contracts check failed"
    exit 1
fi

echo ""
echo "  action-output Coverage Check..."
if $SCRIPT_DIR/check_action-output_coverage.sh ${VERBOSE:+--verbose}; then
    echo "  ✅ Coverage check passed"
else
    echo "  ❌ Coverage check failed"
    exit 1
fi

echo ""
echo "✅ action-output proofing passed"
exit 0
