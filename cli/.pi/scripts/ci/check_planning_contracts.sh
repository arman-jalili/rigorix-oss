#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SRC_DIR="${REPO_ROOT}/cli/src/planning"
PASS=0; FAIL=0; MISSING=()
RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); MISSING+=("$1"); }

echo "=== Planning Contract Check ===" && echo ""
echo "--- Service Contracts ---"
grep -q "pub trait PlanCommandService" "${SRC_DIR}/application/service.rs" 2>/dev/null && pass "PlanCommandService trait defined" || fail "PlanCommandService missing"

echo "" && echo "--- Domain Contracts ---"
[ -f "${SRC_DIR}/domain/error.rs" ] && grep -q "pub enum PlanningCliError" "${SRC_DIR}/domain/error.rs" && pass "PlanningCliError defined" || fail "PlanningCliError missing"
[ -f "${SRC_DIR}/domain/event/mod.rs" ] && grep -q "pub enum PlanningCliEvent" "${SRC_DIR}/domain/event/mod.rs" && pass "PlanningCliEvent defined" || fail "PlanningCliEvent missing"

echo "" && echo "--- DTO Contracts ---"
[ -f "${SRC_DIR}/application/dto/mod.rs" ] && DTO_COUNT=$(grep -c "pub struct" "${SRC_DIR}/application/dto/mod.rs" 2>/dev/null || true) && pass "${DTO_COUNT} DTO structs defined" || fail "DTO module missing"

echo "" && echo "--- Repository Contracts ---"
grep -q "pub trait PlanningRepository" "${SRC_DIR}/infrastructure/repository/mod.rs" 2>/dev/null && pass "PlanningRepository defined" || fail "PlanningRepository missing"

echo "" && echo "--- API Contracts ---"
[ -f "${SRC_DIR}/interfaces/http/mod.rs" ] && API_COUNT=$(grep -c "pub const.*_PATH:" "${SRC_DIR}/interfaces/http/mod.rs" 2>/dev/null || true) && pass "${API_COUNT} API endpoints defined" || fail "HTTP contracts missing"

echo "" && echo "--- Module Structure ---"
for layer in domain application infrastructure interfaces; do
    [ -f "${SRC_DIR}/${layer}/mod.rs" ] && pass "planning/${layer}/mod.rs exists" || fail "planning/${layer}/mod.rs missing"
done

grep -q "pub mod planning" "${REPO_ROOT}/cli/src/lib.rs" 2>/dev/null && pass "Module registered in lib.rs" || fail "Module not in lib.rs"

echo "" && echo "=== Summary ===" && echo -e "  Passed: ${GREEN}${PASS}${NC}   Failed: ${RED}${FAIL}${NC}"
[ "$FAIL" -gt 0 ] && echo -e "${RED}FAILURES:${NC}" && printf '  - %s\n' "${MISSING[@]}" && exit 1 || echo -e "${GREEN}All satisfied.${NC}"
