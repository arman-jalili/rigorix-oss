#!/usr/bin/env bash
# ============================================================================
# check_risk-gating_contracts.sh
#
# Validates that every contract interface from the risk-gating module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_risk-gating_contracts.sh [--help]
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

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

# Determine source directory
if [ ! -d "$SRC_DIR" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
fi
if [ ! -d "$SRC_DIR" ]; then
    log_fail "Source directory not found"
    exit 1
fi

echo ""
echo "═══ Risk-Gating Contract Implementation Check ═══"
echo "Source: $SRC_DIR/risk_gating"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait RiskGateService' "$SRC_DIR/risk_gating/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*RiskGateService' "$SRC_DIR/risk_gating/application/gate_service_impl.rs" 2>/dev/null; then
        log_pass "RiskGateService → RiskGateServiceImpl"
    else
        log_fail "RiskGateService trait has no implementation"
    fi
else
    log_fail "RiskGateService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait RiskGateFactory' "$SRC_DIR/risk_gating/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*RiskGateFactory' "$SRC_DIR/risk_gating/application/gate_factory_impl.rs" 2>/dev/null; then
        log_pass "RiskGateFactory → RiskGateFactoryImpl"
    else
        log_fail "RiskGateFactory trait has no implementation"
    fi
else
    log_fail "RiskGateFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub enum RiskLevel' "$SRC_DIR/risk_gating/domain/risk_level.rs" 2>/dev/null; then
    log_pass "RiskLevel enum exists"
else
    log_fail "RiskLevel enum not found"
fi

if grep -q 'pub enum GatingAction' "$SRC_DIR/risk_gating/domain/risk_level.rs" 2>/dev/null; then
    log_pass "GatingAction enum exists"
else
    log_fail "GatingAction enum not found"
fi

if grep -q 'pub trait RiskClassifier' "$SRC_DIR/risk_gating/domain/risk_classifier.rs" 2>/dev/null; then
    log_pass "RiskClassifier trait exists"

    # Check default implementation
    if grep -q 'impl RiskClassifier for DefaultClassifier' "$SRC_DIR/risk_gating/domain/default_classifier.rs" 2>/dev/null; then
        log_pass "RiskClassifier → DefaultClassifier implementation"
    else
        log_fail "RiskClassifier has no DefaultClassifier implementation"
    fi
else
    log_fail "RiskClassifier trait not found"
fi

if grep -q 'pub struct RiskConfig' "$SRC_DIR/risk_gating/domain/risk_config.rs" 2>/dev/null; then
    log_pass "RiskConfig struct exists"
else
    log_fail "RiskConfig struct not found"
fi

if grep -q 'pub enum RiskGatingError' "$SRC_DIR/risk_gating/domain/error.rs" 2>/dev/null; then
    log_pass "RiskGatingError enum exists"
else
    log_fail "RiskGatingError enum not found"
fi

if grep -q 'pub enum RiskGateEvent' "$SRC_DIR/risk_gating/domain/event/mod.rs" 2>/dev/null; then
    log_pass "RiskGateEvent enum exists"
else
    log_fail "RiskGateEvent enum not found"
fi

if grep -q 'pub struct ClassificationResult' "$SRC_DIR/risk_gating/domain/risk_classifier.rs" 2>/dev/null; then
    log_pass "ClassificationResult struct exists"
else
    log_fail "ClassificationResult struct not found"
fi

if grep -q 'pub struct GateStateRegistry' "$SRC_DIR/risk_gating/domain/gate_state.rs" 2>/dev/null; then
    log_pass "GateStateRegistry struct exists"
else
    log_fail "GateStateRegistry struct not found"
fi

# ---------------------------------------------------------------------------
# Check 4: DTO schemas exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"

DTO_COUNT=$(grep -c 'pub struct.*\(Input\|Output\|Summary\|Status\)' "$SRC_DIR/risk_gating/application/dto/mod.rs" 2>/dev/null || echo 0)
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

if grep -q 'pub const.*_PATH' "$SRC_DIR/risk_gating/interfaces/http/mod.rs" 2>/dev/null; then
    ENDPOINT_COUNT=$(grep -c 'pub const.*_PATH' "$SRC_DIR/risk_gating/interfaces/http/mod.rs" || echo 0)
    log_pass "API endpoint contracts exist ($ENDPOINT_COUNT endpoints)"
else
    log_fail "No API endpoint contracts found"
fi

if grep -q 'pub struct ApiErrorResponse' "$SRC_DIR/risk_gating/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error response format defined"
else
    log_fail "Error response format not found"
fi

if grep -q 'pub mod error_codes' "$SRC_DIR/risk_gating/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Standardized error codes defined"
else
    log_fail "Standardized error codes not found"
fi

# ---------------------------------------------------------------------------
# Check 6: Repository contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait RiskConfigRepository' "$SRC_DIR/risk_gating/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl.*RiskConfigRepository' "$SRC_DIR/risk_gating/infrastructure/default_config_repository.rs" 2>/dev/null; then
        log_pass "RiskConfigRepository → InMemoryConfigRepository"
    else
        log_fail "RiskConfigRepository trait has no implementation"
    fi
else
    log_fail "RiskConfigRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 7: Module structure
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Structure ---"

MODULES=(
    "risk_gating/mod.rs:pub mod"
    "risk_gating/domain/mod.rs:pub mod"
    "risk_gating/application/mod.rs:pub mod"
    "risk_gating/infrastructure/mod.rs:pub mod"
    "risk_gating/interfaces/mod.rs:pub mod"
)
for entry in "${MODULES[@]}"; do
    FILE="${entry%%:*}"
    EXPECTED="${entry##*:}"
    if grep -q "$EXPECTED" "$SRC_DIR/$FILE" 2>/dev/null; then
        log_pass "Module file exists: $FILE"
    else
        log_fail "Module file missing or invalid: $FILE"
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
    echo "Some risk-gating contracts are missing implementations."
    exit 1
fi

echo "All risk-gating contracts have implementations."
exit 0
