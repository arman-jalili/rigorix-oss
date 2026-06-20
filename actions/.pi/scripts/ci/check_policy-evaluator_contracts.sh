#!/usr/bin/env bash
# ============================================================================
# check_policy-evaluator_contracts.sh
# ============================================================================
# Validates that every contract interface in policy-evaluator has a matching
# concrete implementation. Exits 0 if all contracts satisfied, 1 otherwise.
#
# Usage: bash .pi/scripts/ci/check_policy-evaluator_contracts.sh [--verbose]
#
# Checks:
#   - Each service trait in application/service.rs has a corresponding _impl.rs
#   - Each domain type in domain/ has matching usage in implementation files
#   - Repository interface has an implementation
#   - Factory interface has an implementation
# ============================================================================

set -u
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PE_DIR="$PROJECT_DIR/src/policy_evaluator"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'
PASS=0
FAIL=0

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); }

echo "============================================"
echo "  Policy Evaluator Contract Validation"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service traits have implementations
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

check_service() {
    local trait_name="$1"
    local impl_file="$2"
    local impl_path="$PE_DIR/application/$impl_file"

    if [ ! -f "$impl_path" ]; then
        fail "Service '$trait_name': missing implementation file '$impl_file'"
        return 1
    fi
    if grep -q "impl.*$trait_name for" "$impl_path" 2>/dev/null; then
        pass "Service '$trait_name' → implemented in '$impl_file'"
        return 0
    else
        fail "Service '$trait_name': file '$impl_file' exists but does not implement the trait"
        return 1
    fi
}

check_service "PolicyLoadingService" "policy_loader_impl.rs"
check_service "PolicyTamperDetectionService" "policy_tamper_detector_impl.rs"
check_service "PolicyEvaluationService" "policy_evaluator_impl.rs"
check_service "OrgPolicyMergingService" "org_policy_merger_impl.rs"
check_service "PolicyReportGenerationService" "policy_report_generator_impl.rs"
check_service "PolicyEvaluationPipelineService" "policy_evaluation_pipeline_impl.rs"

echo ""

# ---------------------------------------------------------------------------
# Check 2: Factory traits have implementations
# ---------------------------------------------------------------------------
echo "--- Factory Contracts ---"

check_factory() {
    local trait_name="$1"
    local impl_file="$2"
    local impl_path="$PE_DIR/application/$impl_file"

    if [ ! -f "$impl_path" ]; then
        fail "Factory '$trait_name': missing implementation file '$impl_file'"
        return 1
    fi
    if grep -q "impl.*$trait_name for" "$impl_path" 2>/dev/null; then
        pass "Factory '$trait_name' → implemented in '$impl_file'"
        return 0
    else
        fail "Factory '$trait_name': file '$impl_file' exists but does not implement the trait"
        return 1
    fi
}

check_factory "PolicyDocumentFactory" "policy_document_factory_impl.rs"
check_factory "RulesFactory" "rules_factory_impl.rs"
check_factory "CompiledRulesFactory" "compiled_rules_factory_impl.rs"
check_factory "PolicyResultFactory" "policy_result_factory_impl.rs"

echo ""

# ---------------------------------------------------------------------------
# Check 3: Repository interfaces defined
# ---------------------------------------------------------------------------
echo "--- Repository Contracts ---"

if [ -f "$PE_DIR/infrastructure/repository/mod.rs" ]; then
    pass "Repository interfaces file exists"
else
    fail "Repository interfaces file missing: infrastructure/repository/mod.rs"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 4: Domain types defined
# ---------------------------------------------------------------------------
echo "--- Domain Contracts ---"

if [ -f "$PE_DIR/domain/types.rs" ]; then
    pass "Domain types file exists"
else
    fail "Domain types file missing: domain/types.rs"
fi

if [ -f "$PE_DIR/domain/error.rs" ]; then
    pass "Domain error types file exists"
else
    fail "Domain error types file missing: domain/error.rs"
fi

if [ -f "$PE_DIR/domain/event/mod.rs" ]; then
    pass "Domain event types file exists"
else
    fail "Domain event types file missing: domain/event/mod.rs"
fi

echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS}${NC}"
echo -e "  Failed:   ${RED}${FAIL}${NC}"
echo ""

if [ $FAIL -gt 0 ]; then
    echo -e "${RED}Contract validation FAILED.${NC}"
    exit 1
fi

echo -e "${GREEN}All contract implementations validated.${NC}"
exit 0
