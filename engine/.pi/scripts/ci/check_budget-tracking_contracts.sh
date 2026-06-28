#!/usr/bin/env bash
# ============================================================================
# check_budget-tracking_contracts.sh
#
# Validates that every contract interface from the budget-tracking module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_budget-tracking_contracts.sh [--help]
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
echo "═══ Budget-Tracking Contract Implementation Check ═══"
echo "Source: $SRC_DIR/budget_tracking"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait LlmBudgetService' "$SRC_DIR/budget_tracking/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmBudgetService' "$SRC_DIR/budget_tracking/application/llm_budget_impl.rs" 2>/dev/null; then
        log_pass "LlmBudgetService → LlmBudgetImpl"
    else
        log_fail "LlmBudgetService trait has no implementation"
    fi
else
    log_fail "LlmBudgetService trait not found"
fi

if grep -q 'pub trait LlmBudgetReservation' "$SRC_DIR/budget_tracking/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmBudgetReservation' "$SRC_DIR/budget_tracking/application/llm_budget_impl.rs" 2>/dev/null; then
        log_pass "LlmBudgetReservation → LlmBudgetReservationImpl"
    else
        log_fail "LlmBudgetReservation trait has no implementation"
    fi
else
    log_fail "LlmBudgetReservation trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait LlmBudgetFactory' "$SRC_DIR/budget_tracking/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmBudgetFactory' "$SRC_DIR/budget_tracking/application/llm_budget_factory_impl.rs" 2>/dev/null; then
        log_pass "LlmBudgetFactory → LlmBudgetFactoryImpl"
    else
        log_fail "LlmBudgetFactory trait has no implementation"
    fi
else
    log_fail "LlmBudgetFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct LlmBudget' "$SRC_DIR/budget_tracking/domain/budget.rs" 2>/dev/null; then
    log_pass "LlmBudget struct exists"
else
    log_fail "LlmBudget struct not found"
fi

if grep -q 'pub struct LlmBudgetReservationState' "$SRC_DIR/budget_tracking/domain/reservation.rs" 2>/dev/null; then
    log_pass "LlmBudgetReservationState struct exists"
else
    log_fail "LlmBudgetReservationState struct not found"
fi

if grep -q 'pub enum LlmBudgetError' "$SRC_DIR/budget_tracking/domain/error.rs" 2>/dev/null; then
    log_pass "LlmBudgetError enum exists"
else
    log_fail "LlmBudgetError enum not found"
fi

if grep -q 'pub enum BudgetEvent' "$SRC_DIR/budget_tracking/domain/event/mod.rs" 2>/dev/null; then
    log_pass "BudgetEvent enum exists"
else
    log_fail "BudgetEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 4: DTO schemas exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"

DTO_COUNT=$(grep -c 'pub struct.*\(Input\|Output\)' "$SRC_DIR/budget_tracking/application/dto/mod.rs" 2>/dev/null || echo 0)
if [ "$DTO_COUNT" -ge 3 ]; then
    log_pass "DTO schemas exist ($DTO_COUNT input/output DTOs)"
else
    log_fail "Fewer than 3 DTO schemas found ($DTO_COUNT)"
fi

# ---------------------------------------------------------------------------
# Check 5: API contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"

if grep -q 'pub const.*_PATH' "$SRC_DIR/budget_tracking/interfaces/http/mod.rs" 2>/dev/null; then
    ENDPOINT_COUNT=$(grep -c 'pub const.*_PATH' "$SRC_DIR/budget_tracking/interfaces/http/mod.rs" || echo 0)
    log_pass "API endpoint contracts exist ($ENDPOINT_COUNT endpoints)"
else
    log_fail "No API endpoint contracts found"
fi

if grep -q 'pub struct ApiErrorResponse' "$SRC_DIR/budget_tracking/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error response format defined"
else
    log_fail "Error response format not found"
fi

# ---------------------------------------------------------------------------
# Check 6: Repository contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait LlmBudgetRepository' "$SRC_DIR/budget_tracking/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "LlmBudgetRepository trait exists"
else
    log_fail "LlmBudgetRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 7: Domain helper methods exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Helper Methods ---"

if grep -q 'fn would_exceed_calls' "$SRC_DIR/budget_tracking/domain/budget.rs" 2>/dev/null; then
    log_pass "LlmBudget::would_exceed_calls() exists"
else
    log_fail "LlmBudget::would_exceed_calls() not found"
fi

if grep -q 'fn has_capacity' "$SRC_DIR/budget_tracking/domain/budget.rs" 2>/dev/null; then
    log_pass "LlmBudget::has_capacity() exists"
else
    log_fail "LlmBudget::has_capacity() not found"
fi

if grep -q 'fn remaining_calls' "$SRC_DIR/budget_tracking/domain/budget.rs" 2>/dev/null; then
    log_pass "LlmBudget::remaining_calls() exists"
else
    log_fail "LlmBudget::remaining_calls() not found"
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
    echo "Some budget-tracking contracts are missing implementations."
    exit 1
fi

echo "All budget-tracking contracts have implementations."
exit 0
