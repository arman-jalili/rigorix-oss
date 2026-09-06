#!/usr/bin/env bash
# ============================================================================
# check_execution-tools_contracts.sh
#
# Validates that every interface defined in the contract freeze has a concrete
# implementation. Exits 0 if all contracts are satisfied, 1 otherwise.
#
# Usage: bash check_execution-tools_contracts.sh [--help|--verbose]
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# SCRIPT_DIR = <root>/mcp/.pi/scripts/ci, so ../../../ = <root>/mcp
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

MODULE_SRC="${PROJECT_ROOT}/src/execution_tools"
INFRA="${MODULE_SRC}/infrastructure"
APP="${MODULE_SRC}/application"

echo "============================================"
echo "  Contract Implementation Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Contract: EngineFacade trait → EngineFacadeImpl
# ---------------------------------------------------------------------------
echo "--- EngineFacade ---"
IMPLEMENTATIONS=$(grep -rn 'impl EngineFacade for' "${INFRA}/" 2>/dev/null || true)
if echo "$IMPLEMENTATIONS" | grep -q "EngineFacadeImpl"; then
    pass "EngineFacade trait → EngineFacadeImpl"
else
    fail "EngineFacade trait: no impl EngineFacade for ... found in infrastructure/"
fi

# Verify all EngineFacade trait methods are implemented
if [ -n "$IMPLEMENTATIONS" ]; then
    METHOD_COUNT=$(grep -c 'async fn ' "${MODULE_SRC}/domain/entity.rs" 2>/dev/null || echo 0)
    # Count async fns INSIDE the `impl EngineFacade for` region only — a
    # raw whole-file grep also counts async test fns in the `mod tests`
    # module, which made the check spuriously fail as the test suite grew.
    IMPL_COUNT=$(sed -n '/^impl EngineFacade for EngineFacadeImpl {/,/^}/p' \
        "${INFRA}/engine_facade_impl.rs" | grep -c 'async fn ' || echo 0)
    if [ "$METHOD_COUNT" -eq "$IMPL_COUNT" ]; then
        pass "EngineFacade: $METHOD_COUNT trait methods → $IMPL_COUNT impl methods"
    else
        fail "EngineFacade: $METHOD_COUNT trait methods but $IMPL_COUNT impl methods"
    fi
fi

# ---------------------------------------------------------------------------
# Contract: ExecuteHandler trait → ExecuteHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- ExecuteHandler ---"
if grep -q 'impl ExecuteHandler for ExecuteHandlerImpl' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "ExecuteHandler trait → ExecuteHandlerImpl"
else
    fail "ExecuteHandler trait: no impl ExecuteHandler for ... found"
fi

# ---------------------------------------------------------------------------
# Contract: ValidatePlanHandler trait → ValidatePlanHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- ValidatePlanHandler ---"
if grep -q 'impl ValidatePlanHandler for ValidatePlanHandlerImpl' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "ValidatePlanHandler trait → ValidatePlanHandlerImpl"
else
    fail "ValidatePlanHandler trait: no impl found"
fi

# ---------------------------------------------------------------------------
# Contract: CheckEnforcementHandler trait → CheckEnforcementHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- CheckEnforcementHandler ---"
if grep -q 'impl CheckEnforcementHandler for CheckEnforcementHandlerImpl' "${APP}/service_impl.rs" 2>/dev/null; then
    pass "CheckEnforcementHandler trait → CheckEnforcementHandlerImpl"
else
    fail "CheckEnforcementHandler trait: no impl found"
fi

# ---------------------------------------------------------------------------
# Contract: ExecutionRepository trait → InMemoryExecutionRepository
# ---------------------------------------------------------------------------
echo ""
echo "--- ExecutionRepository ---"
if grep -q 'impl ExecutionRepository for InMemoryExecutionRepository' "${INFRA}/in_memory_repository.rs" 2>/dev/null; then
    pass "ExecutionRepository trait → InMemoryExecutionRepository"
else
    fail "ExecutionRepository trait: no impl found"
fi

# ---------------------------------------------------------------------------
# Contract: each impl has unit tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Coverage ---"
TEST_FILE="${PROJECT_ROOT}/src/execution_tools/tests.rs"
if [ -f "$TEST_FILE" ]; then
    TEST_COUNT=$(grep -c '#\[test\]' "$TEST_FILE" 2>/dev/null || echo 0)
    TOKIO_TEST_COUNT=$(grep -c '#\[tokio::test\]' "$TEST_FILE" 2>/dev/null || echo 0)
    TOTAL_TESTS=$((TEST_COUNT + TOKIO_TEST_COUNT))
    if [ "$TOTAL_TESTS" -ge 10 ]; then
        pass "Unit tests: $TOTAL_TESTS tests found (≥ 10 minimum)"
    else
        fail "Unit tests: only $TOTAL_TESTS tests found (need ≥ 10)"
    fi
else
    fail "No test file found at ${TEST_FILE}"
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
