#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SRC_DIR="${REPO_ROOT}/cli/src/template_generation"

PASS=0; FAIL=0; MISSING=()
RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); MISSING+=("$1"); }

echo "=== Template Generation Contract Check ==="
echo ""

echo "--- Service Contracts ---"
if grep -q "pub trait GenerateCommandService" "${SRC_DIR}/application/service.rs" 2>/dev/null; then
    pass "GenerateCommandService trait defined"
else
    fail "GenerateCommandService trait missing"
fi

echo "" && echo "--- Domain Contracts ---"
if [ -f "${SRC_DIR}/domain/error.rs" ] && grep -q "pub enum GenerationCliError" "${SRC_DIR}/domain/error.rs"; then
    pass "GenerationCliError enum defined"
else
    fail "GenerationCliError missing"
fi
if [ -f "${SRC_DIR}/domain/event/mod.rs" ] && grep -q "pub enum TemplateGenerationCliEvent" "${SRC_DIR}/domain/event/mod.rs"; then
    pass "TemplateGenerationCliEvent enum defined"
else
    fail "TemplateGenerationCliEvent missing"
fi

echo "" && echo "--- DTO Contracts ---"
if [ -f "${SRC_DIR}/application/dto/mod.rs" ]; then
    DTO_COUNT=$(grep -c "pub struct" "${SRC_DIR}/application/dto/mod.rs" 2>/dev/null || true)
    pass "${DTO_COUNT} DTO structs defined"
else
    fail "DTO module missing"
fi

echo "" && echo "--- Repository Contracts ---"
if grep -q "pub trait TemplateGenerationRepository" "${SRC_DIR}/infrastructure/repository/mod.rs" 2>/dev/null; then
    REPO_METHODS=$(grep -c "async fn" "${SRC_DIR}/infrastructure/repository/mod.rs" 2>/dev/null || true)
    pass "TemplateGenerationRepository trait defined (${REPO_METHODS} methods)"
else
    fail "TemplateGenerationRepository missing"
fi

echo "" && echo "--- API Contracts ---"
if [ -f "${SRC_DIR}/interfaces/http/mod.rs" ]; then
    API_COUNT=$(grep -c "pub const.*_PATH:" "${SRC_DIR}/interfaces/http/mod.rs" 2>/dev/null || true)
    pass "${API_COUNT} API endpoint paths defined"
else
    fail "HTTP contracts missing"
fi

echo "" && echo "--- Module Structure ---"
for layer in domain application infrastructure interfaces; do
    if [ -f "${SRC_DIR}/${layer}/mod.rs" ]; then
        pass "template_generation/${layer}/mod.rs exists"
    else
        fail "template_generation/${layer}/mod.rs missing"
    fi
done

echo "" && echo "--- Module Registration ---"
if grep -q "pub mod template_generation" "${REPO_ROOT}/cli/src/lib.rs" 2>/dev/null; then
    pass "Module registered in lib.rs"
else
    fail "Module not in lib.rs"
fi

echo "" && echo "=== Summary ==="
echo -e "  Passed: ${GREEN}${PASS}${NC}   Failed: ${RED}${FAIL}${NC}"
if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}FAILURES:${NC}"
    for m in "${MISSING[@]}"; do echo "  - $m"; done
    exit 1
fi
echo -e "${GREEN}All contracts satisfied.${NC}"
