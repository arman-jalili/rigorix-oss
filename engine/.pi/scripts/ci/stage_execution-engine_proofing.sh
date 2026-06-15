#!/usr/bin/env bash
# ============================================================================
# stage_execution-engine_proofing.sh
#
# CI stage wrapper that runs all execution-engine proofing checks.
# Called by run_hardening_stages.sh.
#
# Usage: bash .pi/scripts/ci/stage_execution-engine_proofing.sh [--help]
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STAGE_NAME="execution-engine_proofing"

echo ""
echo "═══ Stage: $STAGE_NAME ═══"
echo ""

checks_passed=0
checks_failed=0

# Run all execution-engine proofing checks
for check in \
    "$SCRIPT_DIR/check_execution-engine_contracts.sh" \
    "$SCRIPT_DIR/check_execution-engine_coverage.sh"; do

    check_name="$(basename "$check" .sh)"
    echo "─── $check_name ───"

    if [ ! -f "$check" ]; then
        echo "  SKIP: $check not found"
        continue
    fi

    if bash "$check"; then
        echo "  CHECK PASSED: $check_name"
        ((checks_passed++))
    else
        echo "  CHECK FAILED: $check_name"
        ((checks_failed++))
    fi
    echo ""
done

# Summary
echo "═══ Stage Summary: $STAGE_NAME ═══"
echo "  Passed: $checks_passed"
echo "  Failed: $checks_failed"
echo ""

if [ "$checks_failed" -gt 0 ]; then
    echo "Stage $STAGE_NAME FAILED."
    exit 1
fi

echo "Stage $STAGE_NAME PASSED."
exit 0
