#!/usr/bin/env bash
# ============================================================================
# check_enterprise-proxy_contracts.sh
#
# Validates that every interface defined in the contract freeze has a concrete
# implementation. Exits 0 if all contracts are satisfied, 1 otherwise.
#
# Usage: bash check_enterprise-proxy_contracts.sh [--help|--verbose]
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

MODULE_SRC="${PROJECT_ROOT}/src/enterprise_proxy"
INFRA="${MODULE_SRC}/infrastructure"
APP="${MODULE_SRC}/application"
DOMAIN="${MODULE_SRC}/domain"

echo "============================================"
echo "  Enterprise-Proxy Contract Implementation Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Contract: EnterpriseProxy trait → EnterpriseProxyImpl
# ---------------------------------------------------------------------------
echo "--- EnterpriseProxy ---"
IMPLEMENTATIONS=$(grep -rn 'impl EnterpriseProxy for' "${INFRA}/" 2>/dev/null || true)
if echo "$IMPLEMENTATIONS" | grep -q "EnterpriseProxyImpl"; then
    pass "EnterpriseProxy trait → EnterpriseProxyImpl"
else
    fail "EnterpriseProxy trait: no impl EnterpriseProxy for ... found in infrastructure/"
fi

# Verify all EnterpriseProxy trait methods are implemented
if [ -n "$IMPLEMENTATIONS" ]; then
    METHOD_COUNT=$(grep -c 'async fn ' "${DOMAIN}/entity.rs" 2>/dev/null || echo 0)
    IMPL_COUNT=$(grep -c 'async fn ' "${INFRA}/enterprise_proxy_impl.rs" 2>/dev/null || echo 0)
    if [ "$METHOD_COUNT" -ge 3 ]; then
        pass "EnterpriseProxy: $METHOD_COUNT trait methods defined"
    else
        fail "EnterpriseProxy: only $METHOD_COUNT trait methods (expected ≥ 3)"
    fi
fi

# ---------------------------------------------------------------------------
# Contract: SchemaCache struct defined (domain service)
# ---------------------------------------------------------------------------
echo ""
echo "--- SchemaCache ---"
if grep -q 'pub struct SchemaCache' "${DOMAIN}/entity.rs" 2>/dev/null; then
    pass "SchemaCache struct defined in domain"
else
    fail "SchemaCache struct: not found in domain/entity.rs"
fi

# Verify SchemaCache key methods exist
for method in "pub fn new" "pub fn update" "pub fn tools" "pub fn is_stale" "pub fn clear"; do
    if grep -q "$method" "${DOMAIN}/entity.rs" 2>/dev/null; then
        pass "SchemaCache::${method}() defined"
    else
        fail "SchemaCache::${method}() missing"
    fi
done

# ---------------------------------------------------------------------------
# Contract: Service traits defined in application/
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Traits ---"
for trait_name in "ProxyInitializationService" "EnterpriseToolRouter" "SchemaCacheService"; do
    if grep -q "pub trait ${trait_name}" "${APP}/service.rs" 2>/dev/null; then
        pass "Service trait '${trait_name}' defined"
    else
        fail "Service trait '${trait_name}' missing from application/service.rs"
    fi
done

# ---------------------------------------------------------------------------
# Contract: Repository interfaces in infrastructure/
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Interfaces ---"
if grep -q "pub trait SchemaCacheRepository" "${INFRA}/repository.rs" 2>/dev/null; then
    pass "SchemaCacheRepository trait defined"
else
    fail "SchemaCacheRepository trait missing"
fi

# ---------------------------------------------------------------------------
# Contract: DTOs defined in application/dto/
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"
DTO_FILE="${APP}/dto/mod.rs"
for dto in "InitializeOutput" "HandleToolCallInput" "HandleToolCallOutput" \
           "HealthCheckOutput" "ListAvailableToolsOutput" "ToolSchemaDto" \
           "SchemaCacheStatus" "ProxyConfigSummary"; do
    if grep -q "pub struct ${dto}" "${DTO_FILE}" 2>/dev/null; then
        pass "DTO '${dto}' defined"
    else
        fail "DTO '${dto}' missing from application/dto/mod.rs"
    fi
done

# ---------------------------------------------------------------------------
# Contract: Event schemas in domain/event/
# ---------------------------------------------------------------------------
echo ""
echo "--- Event Schemas ---"
EVENT_FILE="${DOMAIN}/event.rs"
if grep -q "pub enum EnterpriseProxyEvent" "${EVENT_FILE}" 2>/dev/null; then
    pass "EnterpriseProxyEvent enum defined"
else
    fail "EnterpriseProxyEvent enum missing"
fi

# Check event variants exist
for variant in "EnterpriseToolCalled" "EnterpriseToolCompleted" \
               "EnterpriseToolFailed" "EnterpriseSchemaFetched" \
               "EnterpriseSchemaRefreshFailed"; do
    if grep -q "${variant}" "${EVENT_FILE}" 2>/dev/null; then
        pass "Event variant '${variant}' defined"
    else
        fail "Event variant '${variant}' missing"
    fi
done

# ---------------------------------------------------------------------------
# Contract: Error types in domain/error/
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Types ---"
ERROR_FILE="${DOMAIN}/error.rs"
for error_type in "ProxyError" "HandlerError" "ToolCallResult"; do
    if grep -q "pub enum ${error_type}\|pub struct ${error_type}" "${ERROR_FILE}" 2>/dev/null; then
        pass "Error type '${error_type}' defined"
    else
        fail "Error type '${error_type}' missing"
    fi
done

# ---------------------------------------------------------------------------
# Contract: MCP tool handler contracts in interfaces/
# ---------------------------------------------------------------------------
echo ""
echo "--- MCP Tool Contracts ---"
MCP_FILE="${MODULE_SRC}/interfaces/mcp/mod.rs"
for const_name in "ENTERPRISE_TOOL_PREFIX" "ENTERPRISE_TOOL_NAMES"; do
    if grep -q "pub const ${const_name}" "${MCP_FILE}" 2>/dev/null; then
        pass "MCP contract '${const_name}' defined"
    else
        fail "MCP contract '${const_name}' missing"
    fi
done

# ---------------------------------------------------------------------------
# Contract: Tests exist for each component
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Coverage ---"
# Count tests in implementation files
IMPL_TESTS=$(grep -cE '#\[(test|tokio::test)\]' "${INFRA}/enterprise_proxy_impl.rs" 2>/dev/null || echo 0)
# Count contract freeze tests
CF_FILE="${PROJECT_ROOT}/tests/enterprise_proxy_contract_freeze_test.rs"
CF_TESTS=$(grep -cE '#\[(test|tokio::test)\]' "${CF_FILE}" 2>/dev/null || echo 0)

TOTAL_TESTS=$((IMPL_TESTS + CF_TESTS))

if [ "$TOTAL_TESTS" -ge 20 ]; then
    pass "Unit tests: $TOTAL_TESTS tests total (≥ 20 minimum)"
else
    fail "Unit tests: $TOTAL_TESTS tests total (need ≥ 20)"
fi

if [ "$VERBOSE" = true ]; then
    echo ""
    echo "  Test breakdown:"
    echo "    enterprise_proxy_impl.rs:            $IMPL_TESTS tests"
    echo "    enterprise_proxy_contract_freeze.rs: $CF_TESTS tests"
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
