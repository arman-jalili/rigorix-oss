#!/usr/bin/env bash
# ============================================================================
# check_quality-gates_contracts.sh
#
# Validates that every contract interface from the quality-gates module
# has a concrete implementation.
#
# Usage: bash .pi/scripts/ci/check_quality-gates_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [ -d "$(cd "$PI_DIR/.." && pwd)/engine/src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/engine/src"
elif [ -d "$(cd "$PI_DIR/.." && pwd)/src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
else
    echo "ERROR: Source directory not found"
    exit 1
fi

QUALITY_DIR="$SRC_DIR/quality_gates"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Quality Gates Contract Implementation Check ═══"
echo "Source: $QUALITY_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait QualityGateService' "$QUALITY_DIR/application/service.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*QualityGateService' "$QUALITY_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "QualityGateService → $IMPL_FILE"
    else
        log_fail "QualityGateService trait has no implementation"
    fi
else
    log_fail "QualityGateService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait QualityGateConfigRepository' "$QUALITY_DIR/infrastructure/repository.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*QualityGateConfigRepository' "$QUALITY_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "QualityGateConfigRepository → $IMPL_FILE"
    else
        log_fail "QualityGateConfigRepository trait has no implementation"
    fi
else
    log_fail "QualityGateConfigRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

for entity in "pub enum QualityLevel" "pub struct GreenContract" \
              "pub enum QualityGateOutcome" "pub struct QualityGateConfig" \
              "pub enum QualityGateError" "pub enum QualityGateEvent"; do
    NAME=$(echo "$entity" | awk '{print $3}')
    if grep -q "$entity" "$QUALITY_DIR/domain/"*.rs 2>/dev/null; then
        log_pass "$NAME exists"
    else
        log_fail "$NAME not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 4: QualityLevel methods
# ---------------------------------------------------------------------------
echo ""
echo "--- QualityLevel ---"

for method in as_str description typical_command as_u8; do
    if grep -q "fn $method" "$QUALITY_DIR/domain/level.rs" 2>/dev/null; then
        log_pass "QualityLevel::$method() exists"
    else
        log_fail "QualityLevel::$method() not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 5: GreenContract methods
# ---------------------------------------------------------------------------
echo ""
echo "--- GreenContract ---"

for method in new evaluate description; do
    if grep -q "fn $method" "$QUALITY_DIR/domain/contract.rs" 2>/dev/null; then
        log_pass "GreenContract::$method() exists"
    else
        log_fail "GreenContract::$method() not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: QualityGateOutcome methods
# ---------------------------------------------------------------------------
echo ""
echo "--- QualityGateOutcome ---"

for method in is_satisfied is_unsatisfied required_level observed_level gap summary; do
    if grep -q "fn $method" "$QUALITY_DIR/domain/outcome.rs" 2>/dev/null; then
        log_pass "QualityGateOutcome::$method() exists"
    else
        log_fail "QualityGateOutcome::$method() not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 7: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

for dto in EvaluateGateInput EvaluateGateOutput ClassifyTestScopeInput ClassifyTestScopeOutput \
           GetContractInput GetContractOutput ValidateConfigInput ValidateConfigOutput; do
    if grep -q "pub struct $dto" "$QUALITY_DIR/application/dto.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

for source in ContractSource; do
    if grep -q "pub enum $source" "$QUALITY_DIR/application/dto.rs" 2>/dev/null; then
        log_pass "$source enum exists"
    else
        log_fail "$source enum not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 8: HTTP Contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

for endpoint in EVALUATE_PATH CLASSIFY_PATH CONTRACT_PATH; do
    if grep -q "pub const $endpoint" "$QUALITY_DIR/interfaces/http.rs" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

for req_resp in EvaluateGateRequest EvaluateGateResponse ClassifyScopeRequest ClassifyScopeResponse \
                ContractResponse ApiErrorResponse; do
    if grep -q "pub struct $req_resp" "$QUALITY_DIR/interfaces/http.rs" 2>/dev/null; then
        log_pass "$req_resp exists"
    else
        log_fail "$req_resp not found"
    fi
done

if grep -q 'pub mod error_codes' "$QUALITY_DIR/interfaces/http.rs" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
fi

# ---------------------------------------------------------------------------
# Check 9: Event payloads exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Event Payloads ---"

for event in GateEvaluated GateSatisfied GateUnsatisfied ConfigUpdated; do
    if grep -q "$event" "$QUALITY_DIR/domain/event.rs" 2>/dev/null; then
        log_pass "QualityGateEvent::$event exists"
    else
        log_fail "QualityGateEvent::$event not found"
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
    echo "Some quality-gates contracts are missing implementations."
    exit 1
fi

echo "All quality-gates contracts have implementations."
exit 0
