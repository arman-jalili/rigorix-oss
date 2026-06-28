#!/usr/bin/env bash
# ============================================================================
# check_policy-engine_contracts.sh
#
# Validates that every contract interface from the policy-engine module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_policy-engine_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC_DIR="$(cd "$PI_DIR/.." && pwd)/engine/src"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

# Determine source directory
if [ ! -d "$SRC_DIR" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
fi
if [ ! -d "$SRC_DIR" ]; then
    log_fail "Source directory not found"
    exit 1
fi

echo ""
echo "═══ Policy Engine Contract Implementation Check ═══"
echo "Source: $SRC_DIR/policy_engine"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Module Structure — all layers exist
# ---------------------------------------------------------------------------
echo "--- Module Structure ---"

for layer in domain application infrastructure interfaces; do
    if [ -f "$SRC_DIR/policy_engine/$layer/mod.rs" ]; then
        log_pass "policy_engine/$layer/ exists"
    else
        log_fail "policy_engine/$layer/ missing"
    fi
done

# ---------------------------------------------------------------------------
# Check 2: Domain Entities
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct PolicyRule' "$SRC_DIR/policy_engine/domain/rule.rs" 2>/dev/null; then
    log_pass "PolicyRule struct exists"
else
    log_fail "PolicyRule struct not found"
fi

if grep -q 'pub enum PolicyCondition' "$SRC_DIR/policy_engine/domain/condition.rs" 2>/dev/null; then
    log_pass "PolicyCondition enum exists"
else
    log_fail "PolicyCondition enum not found"
fi

if grep -q 'pub enum PolicyAction' "$SRC_DIR/policy_engine/domain/action.rs" 2>/dev/null; then
    log_pass "PolicyAction enum exists"
else
    log_fail "PolicyAction enum not found"
fi

if grep -q 'pub enum ReconcileReason' "$SRC_DIR/policy_engine/domain/action.rs" 2>/dev/null; then
    log_pass "ReconcileReason enum exists"
else
    log_fail "ReconcileReason enum not found"
fi

if grep -q 'pub struct LaneContext' "$SRC_DIR/policy_engine/domain/context.rs" 2>/dev/null; then
    log_pass "LaneContext struct exists"
else
    log_fail "LaneContext struct not found"
fi

for entity in LaneBlocker ReviewStatus DiffScope; do
    if grep -q "pub enum $entity" "$SRC_DIR/policy_engine/domain/context.rs" 2>/dev/null; then
        log_pass "$entity enum exists"
    else
        log_fail "$entity enum not found"
    fi
done

if grep -q 'pub struct PolicyConfig' "$SRC_DIR/policy_engine/domain/config.rs" 2>/dev/null; then
    log_pass "PolicyConfig struct exists"
else
    log_fail "PolicyConfig struct not found"
fi

if grep -q 'pub struct RuleDefinition' "$SRC_DIR/policy_engine/domain/config.rs" 2>/dev/null; then
    log_pass "RuleDefinition struct exists"
else
    log_fail "RuleDefinition struct not found"
fi

if grep -q 'pub enum PolicyEngineError' "$SRC_DIR/policy_engine/domain/error.rs" 2>/dev/null; then
    log_pass "PolicyEngineError enum exists"
else
    log_fail "PolicyEngineError enum not found"
fi

if grep -q 'pub enum PolicyEvent' "$SRC_DIR/policy_engine/domain/event/mod.rs" 2>/dev/null; then
    log_pass "PolicyEvent enum exists"
else
    log_fail "PolicyEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain logic methods exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Logic ---"

if grep -q 'fn matches' "$SRC_DIR/policy_engine/domain/condition.rs" 2>/dev/null; then
    log_pass "PolicyCondition::matches() exists"
else
    log_fail "PolicyCondition::matches() not found"
fi

if grep -q 'fn flatten_into' "$SRC_DIR/policy_engine/domain/action.rs" 2>/dev/null; then
    log_pass "PolicyAction::flatten_into() exists"
else
    log_fail "PolicyAction::flatten_into() not found"
fi

if grep -q 'fn into_rules' "$SRC_DIR/policy_engine/domain/config.rs" 2>/dev/null; then
    log_pass "PolicyConfig::into_rules() exists"
else
    log_fail "PolicyConfig::into_rules() not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Service Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Contracts ---"

if grep -q 'pub trait PolicyEngineService' "$SRC_DIR/policy_engine/application/engine.rs" 2>/dev/null; then
    if grep -q 'impl.*PolicyEngineService' "$SRC_DIR/policy_engine/application/engine_impl.rs" 2>/dev/null; then
        log_pass "PolicyEngineService → PolicyEngineServiceImpl"
    else
        log_fail "PolicyEngineService trait has no implementation"
    fi
else
    log_fail "PolicyEngineService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 5: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait PolicyEngineFactory' "$SRC_DIR/policy_engine/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*PolicyEngineFactory' "$SRC_DIR/policy_engine/application/factory_impl.rs" 2>/dev/null; then
        log_pass "PolicyEngineFactory → PolicyEngineFactoryImpl"
    else
        log_fail "PolicyEngineFactory trait has no implementation"
    fi
else
    log_fail "PolicyEngineFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 6: DTO schemas exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"

DTO_COUNT=$(grep -c 'pub struct.*\(Input\|Output\)' "$SRC_DIR/policy_engine/application/dto/mod.rs" 2>/dev/null || echo 0)
if [ "$DTO_COUNT" -ge 4 ]; then
    log_pass "DTO schemas exist ($DTO_COUNT input/output DTOs)"
else
    log_fail "Fewer than 4 DTO schemas found ($DTO_COUNT)"
fi

# ---------------------------------------------------------------------------
# Check 7: API contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"

if grep -q 'pub const.*_PATH' "$SRC_DIR/policy_engine/interfaces/http/mod.rs" 2>/dev/null; then
    ENDPOINT_COUNT=$(grep -c 'pub const.*_PATH' "$SRC_DIR/policy_engine/interfaces/http/mod.rs" || echo 0)
    log_pass "API endpoint contracts exist ($ENDPOINT_COUNT endpoints)"
else
    log_fail "No API endpoint contracts found"
fi

for response_type in EvaluatePolicyResponse GetRulesResponse LoadRulesResponse ReloadRulesResponse; do
    if grep -q "pub struct $response_type" "$SRC_DIR/policy_engine/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "$response_type struct exists"
    else
        log_fail "$response_type struct not found"
    fi
done

if grep -q 'pub struct ApiErrorResponse' "$SRC_DIR/policy_engine/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error response format defined"
else
    log_fail "Error response format not found"
fi

# ---------------------------------------------------------------------------
# Check 8: Repository contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait PolicyRepository' "$SRC_DIR/policy_engine/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl.*PolicyRepository' "$SRC_DIR/policy_engine/infrastructure/default_policy_repository.rs" 2>/dev/null; then
        log_pass "PolicyRepository → DefaultPolicyRepository"
    else
        log_fail "PolicyRepository trait has no implementation"
    fi
else
    log_fail "PolicyRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 9: Event schemas
# ---------------------------------------------------------------------------
echo ""
echo "--- Event Schemas ---"

for variant in RuleMatched ActionsDispatched ConfigLoaded EvaluationPerformed; do
    if grep -q "$variant" "$SRC_DIR/policy_engine/domain/event/mod.rs" 2>/dev/null; then
        log_pass "PolicyEvent::$variant variant exists"
    else
        log_fail "PolicyEvent::$variant variant not found"
    fi
done

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "═══ Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Some policy-engine contracts are missing implementations."
    exit 1
fi

echo "All policy-engine contracts have implementations."
exit 0
