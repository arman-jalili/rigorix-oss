#!/usr/bin/env bash
# CI Integration Proofing Stage
#
# Validates the ci-integration module's contract implementation and coverage.
# Called by run_hardening_stages.sh.
#
# Usage: bash .pi/scripts/ci/stage_ci-integration_proofing.sh [--verbose]
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

echo "  ci-integration Contract Implementation Check..."
if $SCRIPT_DIR/check_ci-integration_contracts.sh ${VERBOSE:+--verbose}; then
    echo "  ✅ Contracts check passed"
else
    echo "  ❌ Contracts check failed"
    exit 1
fi

echo ""
echo "  ci-integration Coverage Check..."
if $SCRIPT_DIR/check_ci-integration_coverage.sh ${VERBOSE:+--verbose}; then
    echo "  ✅ Coverage check passed"
else
    echo "  ❌ Coverage check failed"
    exit 1
fi

echo ""
echo "✅ ci-integration proofing passed"
exit 0
