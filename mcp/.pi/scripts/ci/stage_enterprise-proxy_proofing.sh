#!/usr/bin/env bash
# ============================================================================
# stage_enterprise-proxy_proofing.sh
#
# CI stage that runs contract implementation and coverage checks for the
# enterprise-proxy module.
#
# Usage: bash stage_enterprise-proxy_proofing.sh
# Exit:  0 if all checks pass, 1 otherwise
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "============================================"
echo "  Stage: enterprise-proxy Proofing"
echo "============================================"
echo ""

# Run contract check
echo ">>> Contract Implementation Check"
if bash "${SCRIPT_DIR}/check_enterprise-proxy_contracts.sh"; then
    echo ""
    echo ">>> Coverage Check"
    if bash "${SCRIPT_DIR}/check_enterprise-proxy_coverage.sh"; then
        echo ""
        echo -e "\e[32m✅ Stage enterprise-proxy proofing PASSED\e[0m"
        exit 0
    fi
fi

echo ""
echo -e "\e[31m❌ Stage enterprise-proxy proofing FAILED\e[0m"
exit 1
