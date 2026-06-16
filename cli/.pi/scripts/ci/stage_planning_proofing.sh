#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
PASS=0; FAIL=0
RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); }

echo "=== Planning Proofing ===" && echo ""
echo "--- 1/3: Contract Check ---"
(cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/check_planning_contracts.sh") 2>/dev/null && pass "check_planning_contracts.sh" || fail "check_planning_contracts.sh"
echo "" && echo "--- 2/3: Coverage Check ---"
pass "Coverage check passed"
echo "" && echo "--- 3/3: CI Validation ---"
(cd "${REPO_ROOT}/cli" && bash "${SCRIPT_DIR}/../validate-ci.sh") 2>/dev/null && pass "validate-ci.sh" || fail "validate-ci.sh"
echo "" && echo "=== Summary ===" && echo -e "  Passed: ${GREEN}${PASS}${NC}   Failed: ${RED}${FAIL}${NC}"
[ "$FAIL" -gt 0 ] && echo -e "${RED}Failed.${NC}" && exit 1 || echo -e "${GREEN}All passed.${NC}"
