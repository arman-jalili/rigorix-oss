#!/usr/bin/env bash
# ============================================================================
# check_precondition_contracts.sh
#
# Validates that every contract interface from the precondition module
# (the `precondition` bounded context) has a concrete implementation. Uses
# grep/find to detect trait definitions and their implementing structs — no
# frameworks, no dependencies.
#
# Usage: bash .pi/scripts/ci/check_precondition_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

if [ "${1:-}" = "--help" ]; then
    sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENGINE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
SRC_DIR="$ENGINE_ROOT/src"
PRE_DIR="$SRC_DIR/precondition"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

if [ ! -d "$PRE_DIR" ]; then
    echo "precondition module (src/precondition) not found at $PRE_DIR" >&2
    exit 1
fi

echo ""
echo "═══ Precondition Contract Implementation Check ═══"
echo "Source: $PRE_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Module registration
# ---------------------------------------------------------------------------
echo "--- Module Registration ---"
if grep -q 'pub mod precondition;' "$SRC_DIR/lib.rs" 2>/dev/null; then
    log_pass "precondition module registered in lib.rs"
else
    log_fail "pub mod precondition; missing from src/lib.rs"
fi

# ---------------------------------------------------------------------------
# Check 2: Service + gate contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Application Contracts ---"

if grep -q 'pub trait PreconditionService' "$PRE_DIR/application/service.rs" 2>/dev/null; then
    if grep -q 'impl PreconditionService for PreconditionServiceImpl' "$PRE_DIR/application/service_impl.rs" 2>/dev/null; then
        log_pass "PreconditionService → PreconditionServiceImpl"
    else
        log_fail "PreconditionService trait has no implementation in service_impl.rs"
    fi
    for method in 'async fn evaluate' 'async fn is_configured' 'async fn evaluate_with_findings'; do
        if grep -q "$method" "$PRE_DIR/application/service.rs" 2>/dev/null; then
            log_pass "PreconditionService declares $method"
        else
            log_fail "PreconditionService is missing $method"
        fi
    done
else
    log_fail "PreconditionService trait not found in application/service.rs"
fi

if grep -q 'pub trait DispatchGate' "$PRE_DIR/application/gate.rs" 2>/dev/null; then
    if grep -q 'impl DispatchGate for PreconditionDispatchGate' "$PRE_DIR/application/gate_impl.rs" 2>/dev/null; then
        log_pass "DispatchGate → PreconditionDispatchGate"
    else
        log_fail "PreconditionDispatchGate does not implement DispatchGate"
    fi
else
    log_fail "DispatchGate trait not found in application/gate.rs"
fi

if grep -q 'pub trait CompanionStepObligationService' "$PRE_DIR/application/companion.rs" 2>/dev/null; then
    if grep -q 'impl CompanionStepObligationService for CompanionStepObligationServiceImpl' "$PRE_DIR/application/companion_impl.rs" 2>/dev/null; then
        log_pass "CompanionStepObligationService → CompanionStepObligationServiceImpl"
    else
        log_fail "CompanionStepObligationServiceImpl does not implement CompanionStepObligationService"
    fi
else
    log_fail "CompanionStepObligationService trait not found in application/companion.rs"
fi

if grep -q 'pub trait PreconditionSurfaces' "$PRE_DIR/application/surfaces.rs" 2>/dev/null; then
    if grep -q 'impl PreconditionSurfaces for PreconditionSurfacesImpl' "$PRE_DIR/application/surfaces.rs" 2>/dev/null; then
        log_pass "PreconditionSurfaces → PreconditionSurfacesImpl"
    else
        log_fail "PreconditionSurfacesImpl does not implement PreconditionSurfaces"
    fi
else
    log_fail "PreconditionSurfaces trait not found in application/surfaces.rs"
fi

if grep -q 'pub trait PreconditionFactory' "$PRE_DIR/application/factory.rs" 2>/dev/null; then
    log_pass "PreconditionFactory trait defined"
else
    log_fail "PreconditionFactory trait not found"
fi

if grep -q 'pub struct HardeningConfig' "$PRE_DIR/application/hardening.rs" 2>/dev/null; then
    log_pass "HardeningConfig exists"
else
    log_fail "HardeningConfig struct not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository + runner contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Infrastructure Contracts ---"

if grep -q 'pub trait PreconditionRepository' "$PRE_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl PreconditionRepository for TomlPreconditionRepository' "$PRE_DIR/infrastructure/repository/toml_repository.rs" 2>/dev/null; then
        log_pass "PreconditionRepository → TomlPreconditionRepository"
    else
        log_fail "TomlPreconditionRepository does not implement PreconditionRepository"
    fi
else
    log_fail "PreconditionRepository trait not found"
fi

if grep -q 'pub trait PreconditionRunner' "$PRE_DIR/infrastructure/runner.rs" 2>/dev/null; then
    if grep -q 'impl PreconditionRunner for ProcessPreconditionRunner' "$PRE_DIR/infrastructure/runner.rs" 2>/dev/null; then
        log_pass "PreconditionRunner → ProcessPreconditionRunner"
    else
        log_fail "ProcessPreconditionRunner does not implement PreconditionRunner"
    fi
else
    log_fail "PreconditionRunner trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct Precondition ' "$PRE_DIR/domain/precondition.rs" 2>/dev/null; then
    log_pass "Precondition exists"
else
    log_fail "Precondition struct not found in domain/precondition.rs"
fi

for type in FailureAction SafetyCaps PreconditionConfig; do
    if grep -q "pub \(struct\|enum\) $type" "$PRE_DIR/domain/precondition.rs" 2>/dev/null; then
        log_pass "$type exists in domain/precondition.rs"
    else
        log_fail "$type not found in domain/precondition.rs"
    fi
done

if grep -q 'pub struct GatingMode' "$PRE_DIR/domain/gating.rs" 2>/dev/null; then
    log_pass "GatingMode exists"
else
    log_fail "GatingMode struct not found in domain/gating.rs"
fi

if grep -q 'pub enum PreconditionError' "$PRE_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "PreconditionError enum exists"
else
    log_fail "PreconditionError enum not found in domain/error.rs"
fi

for type in PreconditionOutcome PreconditionFinding PreconditionChecked; do
    if grep -q "pub \(struct\|enum\) $type" "$PRE_DIR/domain/finding.rs" 2>/dev/null; then
        log_pass "$type exists in domain/finding.rs"
    else
        log_fail "$type not found in domain/finding.rs"
    fi
done

if grep -q 'pub enum PreconditionVerdict' "$PRE_DIR/domain/verdict.rs" 2>/dev/null; then
    log_pass "PreconditionVerdict exists"
else
    log_fail "PreconditionVerdict enum not found in domain/verdict.rs"
fi

# ---------------------------------------------------------------------------
# Check 5: DTO contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Contracts ---"
for dto in DispatchStep PreconditionCheckInput; do
    if grep -q "pub struct $dto" "$PRE_DIR/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: Integration seams (event + envelope evidence)
# ---------------------------------------------------------------------------
echo ""
echo "--- Integration Seams ---"
if grep -q 'PreconditionChecked {' "$SRC_DIR/event_system/domain/event.rs" 2>/dev/null; then
    log_pass "ExecutionEvent::PreconditionChecked wired"
else
    log_fail "ExecutionEvent::PreconditionChecked missing from event.rs"
fi

if grep -q 'pub struct PreconditionFindingRef' "$SRC_DIR/audit/domain/envelope.rs" 2>/dev/null; then
    log_pass "PreconditionFindingRef exists"
else
    log_fail "PreconditionFindingRef not found in audit/domain/envelope.rs"
fi

if grep -q 'pub precondition_findings: Vec<PreconditionFindingRef>' "$SRC_DIR/audit/domain/envelope.rs" 2>/dev/null; then
    log_pass "envelope precondition_findings[] field exists"
else
    log_fail "envelope precondition_findings[] field not found"
fi

if grep -q 'max_failures_before_abort' "$SRC_DIR/configuration/domain/config.rs" 2>/dev/null; then
    log_pass "Config.max_failures_before_abort reachable"
else
    log_fail "Config.max_failures_before_abort not found"
fi

# ---------------------------------------------------------------------------
# Check 7: No frozen stubs left
# ---------------------------------------------------------------------------
echo ""
echo "--- Implementation Completeness ---"
STUBS=$(grep -rn 'todo!\|unimplemented!' "$PRE_DIR" --include="*.rs" 2>/dev/null | grep -v '//' | head -10)
if [ -z "$STUBS" ]; then
    log_pass "no todo!/unimplemented! stubs remain in the precondition module"
else
    log_fail "stubs remain — implementation issues not complete"
    echo "$STUBS"
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
    echo "Some precondition contracts are missing implementations."
    exit 1
fi

echo "All precondition contracts have implementations."
exit 0
