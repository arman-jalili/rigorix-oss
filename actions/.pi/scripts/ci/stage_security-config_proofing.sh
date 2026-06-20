#!/usr/bin/env bash
# Security Config Proofing Stage
#
# Validates the security-config module's contract implementation and coverage.
#
# Usage: bash .pi/scripts/ci/stage_security-config_proofing.sh [--verbose]
#
# Exit codes:
#   0 — All checks pass
#   1 — One or more checks failed

set -euo pipefail

VERBOSE=false
[[ "${1:-}" == "--verbose" ]] && VERBOSE=true

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "  Security Config Contract Implementation Check..."
$SCRIPT_DIR/check_security-config_contracts.sh ${VERBOSE:+--verbose} || exit 1

echo ""
echo "  Security Config Coverage Check..."
$SCRIPT_DIR/check_security-config_coverage.sh ${VERBOSE:+--verbose} || exit 1

echo ""
echo "✅ security-config proofing passed"
