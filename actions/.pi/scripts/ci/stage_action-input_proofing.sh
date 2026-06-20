#!/usr/bin/env bash
# Action Input Proofing Stage
#
# Validates the action-input module's contract implementation and coverage.
# Called by run_hardening_stages.sh.
#
# Usage: bash .pi/scripts/ci/stage_action-input_proofing.sh [--verbose]
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

echo "  action-input Contract Implementation Check..."
if $SCRIPT_DIR/check_action-input_contracts.sh ${VERBOSE:+--verbose}; then
    echo "  ✅ Contracts check passed"
else
    echo "  ❌ Contracts check failed"
    exit 1
fi

echo ""
echo "  action-input Coverage Check..."
if $SCRIPT_DIR/check_action-input_coverage.sh ${VERBOSE:+--verbose}; then
    echo "  ✅ Coverage check passed"
else
    echo "  ❌ Coverage check failed"
    exit 1
fi

echo ""
echo "✅ action-input proofing passed"
exit 0
