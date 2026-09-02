#!/usr/bin/env bash
# ============================================================================
# stage_execution-tools_proofing.sh
#
# CI stage wrapper: runs all execution-tools proofing checks.
# Called by run_hardening_stages.sh
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "============================================"
echo "  Stage: execution-tools Proofing"
echo "============================================"
echo ""

# 1. Contract implementation check
echo ">>> [1/2] Contract Implementation Check"
bash "${SCRIPT_DIR}/check_execution-tools_contracts.sh" || {
    echo -e "\e[31m❌ Contract check failed\e[0m"
    exit 1
}
echo ""

# (heuristic *_coverage.sh removed — real coverage via .pi/scripts/coverage.sh)

echo -e "\e[32m✅ All execution-tools proofing checks passed.\e[0m"
exit 0
