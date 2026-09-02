#!/usr/bin/env bash
# MCP Server Proofing Stage — Wrapper for contract and coverage checks
#
# Runs all MCP Server proofing scripts.
#
# Usage: bash .pi/scripts/ci/stage_mcp-server_proofing.sh
# Exit: 0 = all checks pass, 1 = any check fails

set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || echo ".")"
SCRIPTS_DIR="mcp/.pi/scripts/ci"

PASS=0
FAIL=0

run_check() {
    local script="$1"
    local name="$2"
    
    echo "[STAGE] $name"
    if bash "$script" 2>&1; then
        echo "  ✓ PASS: $name"
        PASS=$((PASS + 1))
    else
        echo "  ✗ FAIL: $name"
        FAIL=$((FAIL + 1))
    fi
    echo ""
}

echo "═══ MCP Server Proofing Stage ═══"
echo ""

run_check "${SCRIPTS_DIR}/check_mcp-server_contracts.sh" "Contract Implementation Check"
# (heuristic *_coverage.sh removed — real coverage via .pi/scripts/coverage.sh)

echo "═══ Stage Results: $PASS passed, $FAIL failed ═══"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
