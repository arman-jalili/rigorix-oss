#!/usr/bin/env bash
# ============================================================================
# check_enforcement_contracts.sh
#
# Validates that every contract interface from the enforcement module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_enforcement_contracts.sh [--help]
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
echo "═══ Enforcement Contract Implementation Check ═══"
echo "Source: $SRC_DIR/enforcement"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait ExecutionEnforcer' "$SRC_DIR/enforcement/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*ExecutionEnforcer' "$SRC_DIR/enforcement/application/enforcer_impl.rs" 2>/dev/null; then
        log_pass "ExecutionEnforcer → ExecutionEnforcerImpl"
    else
        log_fail "ExecutionEnforcer trait has no implementation"
    fi
else
    log_fail "ExecutionEnforcer trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait ExecutionEnforcerFactory' "$SRC_DIR/enforcement/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*ExecutionEnforcerFactory' "$SRC_DIR/enforcement/application/enforcer_factory_impl.rs" 2>/dev/null; then
        log_pass "ExecutionEnforcerFactory → ExecutionEnforcerFactoryImpl"
    else
        log_fail "ExecutionEnforcerFactory trait has no implementation"
    fi
else
    log_fail "ExecutionEnforcerFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct EnforcementConfig' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "EnforcementConfig struct exists"
else
    log_fail "EnforcementConfig struct not found"
fi

if grep -q 'pub enum EnforcementPresetProfile' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "EnforcementPresetProfile enum exists"
else
    log_fail "EnforcementPresetProfile enum not found"
fi

if grep -q 'pub struct ResourceBudget' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "ResourceBudget struct exists"
else
    log_fail "ResourceBudget struct not found"
fi

if grep -q 'pub struct ToolPolicy' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "ToolPolicy struct exists"
else
    log_fail "ToolPolicy struct not found"
fi

if grep -q 'pub enum ToolRiskLevel' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "ToolRiskLevel enum exists"
else
    log_fail "ToolRiskLevel enum not found"
fi

if grep -q 'pub enum EnforcementError' "$SRC_DIR/enforcement/domain/error.rs" 2>/dev/null; then
    log_pass "EnforcementError enum exists"
else
    log_fail "EnforcementError enum not found"
fi

if grep -q 'pub enum EnforcementEvent' "$SRC_DIR/enforcement/domain/event/mod.rs" 2>/dev/null; then
    log_pass "EnforcementEvent enum exists"
else
    log_fail "EnforcementEvent enum not found"
fi

if grep -q 'pub struct SafetyCaps' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "SafetyCaps struct exists"
else
    log_fail "SafetyCaps struct not found"
fi

# ---------------------------------------------------------------------------
# Check 4: DTO schemas exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"

DTO_COUNT=$(grep -c 'pub struct.*\(Input\|Output\)' "$SRC_DIR/enforcement/application/dto/mod.rs" 2>/dev/null || echo 0)
if [ "$DTO_COUNT" -ge 6 ]; then
    log_pass "DTO schemas exist ($DTO_COUNT input/output DTOs)"
else
    log_fail "Fewer than 6 DTO schemas found ($DTO_COUNT)"
fi

# ---------------------------------------------------------------------------
# Check 5: API contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"

if grep -q 'pub const.*_PATH' "$SRC_DIR/enforcement/interfaces/http/mod.rs" 2>/dev/null; then
    ENDPOINT_COUNT=$(grep -c 'pub const.*_PATH' "$SRC_DIR/enforcement/interfaces/http/mod.rs" || echo 0)
    log_pass "API endpoint contracts exist ($ENDPOINT_COUNT endpoints)"
else
    log_fail "No API endpoint contracts found"
fi

if grep -q 'pub struct ApiErrorResponse' "$SRC_DIR/enforcement/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error response format defined"
else
    log_fail "Error response format not found"
fi

# ---------------------------------------------------------------------------
# Check 6: Repository contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait EnforcementPolicyRepository' "$SRC_DIR/enforcement/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl.*EnforcementPolicyRepository' "$SRC_DIR/enforcement/infrastructure/default_policy_repository.rs" 2>/dev/null; then
        log_pass "EnforcementPolicyRepository → DefaultPolicyRepository"
    else
        log_fail "EnforcementPolicyRepository trait has no implementation"
    fi
else
    log_fail "EnforcementPolicyRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 7: Preset builders exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Preset Builder Methods ---"

if grep -q 'fn standard()' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "EnforcementConfig::standard() preset builder exists"
else
    log_fail "EnforcementConfig::standard() not found"
fi

if grep -q 'fn strict()' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "EnforcementConfig::strict() preset builder exists"
else
    log_fail "EnforcementConfig::strict() not found"
fi

if grep -q 'fn maximum()' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "EnforcementConfig::maximum() preset builder exists"
else
    log_fail "EnforcementConfig::maximum() not found"
fi

if grep -q 'fn validate' "$SRC_DIR/enforcement/domain/config.rs" 2>/dev/null; then
    log_pass "EnforcementConfig::validate() method exists"
else
    log_fail "EnforcementConfig::validate() not found"
fi

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
    echo "Some enforcement contracts are missing implementations."
    exit 1
fi

echo "All enforcement contracts have implementations."
exit 0
