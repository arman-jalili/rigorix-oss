#!/usr/bin/env bash
# ============================================================================
# stage_template-tools_proofing.sh
#
# CI stage wrapper: runs all template-tools proofing checks.
# Called by run_hardening_stages.sh
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "============================================"
echo "  Stage: template-tools Proofing"
echo "============================================"
echo ""

# 1. Contract implementation check
echo ">>> [1/2] Contract Implementation Check"
bash "${SCRIPT_DIR}/check_template-tools_contracts.sh" || {
    echo -e "\e[31m❌ Contract check failed\e[0m"
    exit 1
}
echo ""

# 2. Coverage check
echo ">>> [2/2] Coverage Threshold Check"
bash "${SCRIPT_DIR}/check_template-tools_coverage.sh" || {
    echo -e "\e[31m❌ Coverage check failed\e[0m"
    exit 1
}
echo ""

echo -e "\e[32m✅ All template-tools proofing checks passed.\e[0m"
exit 0
