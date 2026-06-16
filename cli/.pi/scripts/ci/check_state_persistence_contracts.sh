#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SRC_DIR="${REPO_ROOT}/cli/src/state_persistence"
PASS=0; FAIL=0
RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); }

echo "=== State Persistence Contract Check ===" && echo ""
grep -q "pub trait StatePersistenceCommandService" "${SRC_DIR}/application/service.rs" 2>/dev/null && pass "Trait defined" || fail "Missing"
[ -f "${SRC_DIR}/domain/error.rs" ] && grep -q "pub enum StatePersistenceCliError" "${SRC_DIR}/domain/error.rs" && pass "Error defined" || fail "Missing"
[ -f "${SRC_DIR}/domain/event/mod.rs" ] && grep -q "pub enum StatePersistenceCliEvent" "${SRC_DIR}/domain/event/mod.rs" && pass "Event defined" || fail "Missing"
[ -f "${SRC_DIR}/application/dto/mod.rs" ] && DTO_COUNT=$(grep -c "pub struct" "${SRC_DIR}/application/dto/mod.rs" 2>/dev/null || true) && pass "${DTO_COUNT} DTOs" || fail "Missing"
grep -q "pub trait StatePersistenceRepository" "${SRC_DIR}/infrastructure/repository/mod.rs" 2>/dev/null && pass "Repository defined" || fail "Missing"
[ -f "${SRC_DIR}/interfaces/http/mod.rs" ] && API_COUNT=$(grep -c "pub const.*_PATH:" "${SRC_DIR}/interfaces/http/mod.rs" 2>/dev/null || true) && pass "${API_COUNT} API endpoints" || fail "Missing"
for layer in domain application infrastructure interfaces; do
    [ -f "${SRC_DIR}/${layer}/mod.rs" ] && pass "${layer}/mod.rs exists" || fail "Missing"
done
grep -q "pub mod state_persistence" "${REPO_ROOT}/cli/src/lib.rs" 2>/dev/null && pass "Registered" || fail "Not registered"
echo "" && echo "=== Summary ===" && echo -e "  Passed: ${GREEN}${PASS}${NC}   Failed: ${RED}${FAIL}${NC}"
[ "$FAIL" -gt 0 ] && exit 1 || echo -e "${GREEN}All satisfied.${NC}"
