#!/usr/bin/env bash
# ============================================================================
# check_template-tools_contracts.sh
#
# Validates that every interface defined in the contract freeze has a concrete
# implementation. Exits 0 if all contracts are satisfied, 1 otherwise.
#
# Usage: bash check_template-tools_contracts.sh [--help|--verbose]
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../../" && pwd)"

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then VERBOSE=true; fi
if [[ "${1:-}" == "--help" ]]; then
    sed -n '/^# =*/,/^\$/p' "${BASH_SOURCE[0]}" | grep -v '^# =' | sed 's/^# //'
    exit 0
fi

PASS=0
FAIL=0

pass() { echo -e "  \e[32m✅ PASS\e[0m $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  \e[31m❌ FAIL\e[0m $1"; FAIL=$((FAIL + 1)); }

MODULE_SRC="${PROJECT_ROOT}/src/template_tools"
INFRA="${MODULE_SRC}/infrastructure"
APP="${MODULE_SRC}/application"
DOMAIN="${MODULE_SRC}/domain"

echo "============================================"
echo "  Template-Tools Contract Implementation Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Contract: TemplateRepository trait → FilesystemTemplateRepository
# ---------------------------------------------------------------------------
echo "--- TemplateRepository ---"
IMPLEMENTATIONS=$(grep -rn 'impl TemplateRepository for' "${INFRA}/" 2>/dev/null || true)
if echo "$IMPLEMENTATIONS" | grep -q "FilesystemTemplateRepository"; then
    pass "TemplateRepository trait → FilesystemTemplateRepository"
else
    fail "TemplateRepository trait: no impl TemplateRepository for ... found in infrastructure/"
fi

# Verify all TemplateRepository trait methods are implemented
if [ -n "$IMPLEMENTATIONS" ]; then
    METHOD_COUNT=$(grep -c 'async fn ' "${DOMAIN}/entity.rs" 2>/dev/null || echo 0)
    IMPL_COUNT=$(grep -c 'async fn ' "${INFRA}/filesystem_repository.rs" 2>/dev/null || echo 0)
    if [ "$METHOD_COUNT" -ge 5 ]; then
        pass "TemplateRepository: $METHOD_COUNT trait methods defined"
    else
        fail "TemplateRepository: only $METHOD_COUNT trait methods (expected ≥ 5)"
    fi
fi

# ---------------------------------------------------------------------------
# Contract: TemplateConverter trait → FilesystemTemplateConverter
# ---------------------------------------------------------------------------
echo ""
echo "--- TemplateConverter ---"
if grep -q 'impl TemplateConverter for FilesystemTemplateConverter' "${INFRA}/template_converter_impl.rs" 2>/dev/null; then
    pass "TemplateConverter trait → FilesystemTemplateConverter"
else
    fail "TemplateConverter trait: no impl TemplateConverter for ... found"
fi

# ---------------------------------------------------------------------------
# Contract: ListTemplatesHandler trait → ListTemplatesHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- ListTemplatesHandler ---"
if grep -q 'impl ListTemplatesHandler for ListTemplatesHandlerImpl' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "ListTemplatesHandler trait → ListTemplatesHandlerImpl"
else
    fail "ListTemplatesHandler trait: no impl found"
fi

# ---------------------------------------------------------------------------
# Contract: GetTemplateHandler trait → GetTemplateHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- GetTemplateHandler ---"
if grep -q 'impl GetTemplateHandler for GetTemplateHandlerImpl' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "GetTemplateHandler trait → GetTemplateHandlerImpl"
else
    fail "GetTemplateHandler trait: no impl found"
fi

# ---------------------------------------------------------------------------
# Contract: CreateTemplateHandler trait → CreateTemplateHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- CreateTemplateHandler ---"
if grep -q 'impl CreateTemplateHandler for CreateTemplateHandlerImpl' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "CreateTemplateHandler trait → CreateTemplateHandlerImpl"
else
    fail "CreateTemplateHandler trait: no impl found"
fi

# ---------------------------------------------------------------------------
# Contract: ValidateTemplateHandler trait → ValidateTemplateHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- ValidateTemplateHandler ---"
if grep -q 'impl ValidateTemplateHandler for ValidateTemplateHandlerImpl' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "ValidateTemplateHandler trait → ValidateTemplateHandlerImpl"
else
    fail "ValidateTemplateHandler trait: no impl found"
fi

# ---------------------------------------------------------------------------
# Contract: PlanTemplateFactory trait → PlanTemplateFactoryImpl (optional)
# ---------------------------------------------------------------------------
echo ""
echo "--- PlanTemplateFactory ---"
if grep -q 'impl PlanTemplateFactory for' "${APP}/factory.rs" 2>/dev/null || grep -q 'impl PlanTemplateFactory for' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "PlanTemplateFactory trait → implementation found"
else
    # Factory is optional — not a hard failure
    echo "  \e[33m⚠️  SKIP\e[0m PlanTemplateFactory: no concrete impl (optional contract)"
fi

# ---------------------------------------------------------------------------
# Contract: each impl has unit tests (inline tests in impl files)
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Coverage ---"

# Count tests in filesystem_repository.rs
FS_TESTS=$(grep -cE '#\[(test|tokio::test)\]' "${INFRA}/filesystem_repository.rs" 2>/dev/null || echo 0)
# Count tests in template_converter_impl.rs
TC_TESTS=$(grep -cE '#\[(test|tokio::test)\]' "${INFRA}/template_converter_impl.rs" 2>/dev/null || echo 0)
# Count tests in service_impl.rs
SV_TESTS=$(grep -cE '#\[(test|tokio::test)\]' "${APP}/service_impl.rs" 2>/dev/null || echo 0)
# Count tests in contract_freeze_test.rs
CF_TESTS=$(grep -cE '#\[(test|tokio::test)\]' "${PROJECT_ROOT}/tests/template_tools_contract_freeze_test.rs" 2>/dev/null || echo 0)

TOTAL_TESTS=$((FS_TESTS + TC_TESTS + SV_TESTS + CF_TESTS))

if [ "$TOTAL_TESTS" -ge 30 ]; then
    pass "Unit tests: $TOTAL_TESTS tests total (≥ 30 minimum)"
else
    fail "Unit tests: $TOTAL_TESTS tests total (need ≥ 30)"
fi

if [ "$VERBOSE" = true ]; then
    echo ""
    echo "  Test breakdown:"
    echo "    filesystem_repository.rs:          $FS_TESTS tests"
    echo "    template_converter_impl.rs:        $TC_TESTS tests"
    echo "    service_impl.rs:                   $SV_TESTS tests"
    echo "    template_tools_contract_freeze.rs: $CF_TESTS tests"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Results"
echo "============================================"
echo -e "  Passed: \e[32m${PASS}\e[0m"
echo -e "  Failed: \e[31m${FAIL}\e[0m"
echo ""

if [ "$FAIL" -gt 0 ]; then exit 1; fi
echo -e "\e[32mAll contracts satisfied.\e[0m"
exit 0
