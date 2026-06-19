#!/usr/bin/env bash
# ============================================================================
# check_recovery-recipes_contracts.sh
#
# Validates that every contract interface from the recovery-recipes module
# has a concrete implementation. Uses grep/find to detect trait definitions
# and their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_recovery-recipes_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Determine source directory
if [ -d "$(cd "$PI_DIR/.." && pwd)/engine/src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/engine/src"
elif [ -d "$(cd "$PI_DIR/.." && pwd)/src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
else
    echo "ERROR: Source directory not found"
    exit 1
fi

RECOVERY_DIR="$SRC_DIR/recovery_recipes"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

echo ""
echo "═══ Recovery Recipes Contract Implementation Check ═══"
echo "Source: $RECOVERY_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait RecoveryService' "$RECOVERY_DIR/application/service.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*RecoveryService' "$RECOVERY_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "RecoveryService → $IMPL_FILE"
    else
        log_fail "RecoveryService trait has no implementation"
    fi
else
    log_fail "RecoveryService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait RecoveryRecipeRepository' "$RECOVERY_DIR/infrastructure/repository.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*RecoveryRecipeRepository' "$RECOVERY_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "RecoveryRecipeRepository → $IMPL_FILE"
    else
        log_fail "RecoveryRecipeRepository trait has no implementation"
    fi
else
    log_fail "RecoveryRecipeRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub enum FailureScenario' "$RECOVERY_DIR/domain/scenario.rs" 2>/dev/null; then
    log_pass "FailureScenario enum exists"
    VARIANTS=$(grep -cE '    [A-Z]' "$RECOVERY_DIR/domain/scenario.rs" 2>/dev/null || echo 0)
    if [ "$VARIANTS" -ge 7 ]; then
        log_pass "All FailureScenario variants present (found: $VARIANTS)"
    else
        log_fail "Only $VARIANTS FailureScenario variants found (expected 7+)"
    fi
else
    log_fail "FailureScenario enum not found"
fi

for entity in "pub enum RecoveryStep" "pub struct RecoveryRecipe" \
              "pub enum EscalationPolicy" "pub enum RecoveryResult" \
              "pub enum RecoveryError" "pub enum RecoveryEvent"; do
    NAME=$(echo "$entity" | awk '{print $3}')
    if grep -q "$entity" "$RECOVERY_DIR/domain/"*.rs 2>/dev/null; then
        log_pass "$NAME exists"
    else
        log_fail "$NAME not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 4: RecoveryContext exists
# ---------------------------------------------------------------------------
echo ""
echo "--- RecoveryContext ---"

if grep -q 'pub struct RecoveryContext' "$RECOVERY_DIR/application/context.rs" 2>/dev/null; then
    log_pass "RecoveryContext struct exists"
    for method in can_attempt record_attempt attempt_count remaining_attempts events; do
        if grep -q "fn $method" "$RECOVERY_DIR/application/context.rs" 2>/dev/null; then
            log_pass "RecoveryContext::$method() exists"
        else
            log_fail "RecoveryContext::$method() not found"
        fi
    done
else
    log_fail "RecoveryContext struct not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

for dto in AttemptRecoveryInput AttemptRecoveryOutput RecipeForInput RecipeForOutput \
           CanAttemptInput CanAttemptOutput ValidateRecipeInput ValidateRecipeOutput; do
    if grep -q "pub struct $dto" "$RECOVERY_DIR/application/dto.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

for source in RecipeSource; do
    if grep -q "pub enum $source" "$RECOVERY_DIR/application/dto.rs" 2>/dev/null; then
        log_pass "$source enum exists"
    else
        log_fail "$source enum not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: HTTP Contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

for endpoint in ATTEMPT_PATH RECIPE_PATH CAN_ATTEMPT_PATH CATALOG_PATH; do
    if grep -q "pub const $endpoint" "$RECOVERY_DIR/interfaces/http.rs" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

for req_resp in AttemptRecoveryRequest AttemptRecoveryResponse RecipeForRequest RecipeForResponse \
                CanAttemptRequest CanAttemptResponse CatalogResponse ApiErrorResponse; do
    if grep -q "pub struct $req_resp" "$RECOVERY_DIR/interfaces/http.rs" 2>/dev/null; then
        log_pass "$req_resp exists"
    else
        log_fail "$req_resp not found"
    fi
done

if grep -q 'pub mod error_codes' "$RECOVERY_DIR/interfaces/http.rs" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
fi

# ---------------------------------------------------------------------------
# Check 7: Event payloads exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Event Payloads ---"

for event in RecoveryAttempted RecoverySucceeded RecoveryFailed Escalated RecipeNotFound; do
    if grep -q "$event" "$RECOVERY_DIR/domain/event.rs" 2>/dev/null; then
        log_pass "RecoveryEvent::$event exists"
    else
        log_fail "RecoveryEvent::$event not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 8: RecoveryRecipe methods exist
# ---------------------------------------------------------------------------
echo ""
echo "--- RecoveryRecipe Methods ---"

for method in new step_count has_remaining_steps default_catalog; do
    if grep -q "fn $method" "$RECOVERY_DIR/domain/recipe.rs" 2>/dev/null; then
        log_pass "RecoveryRecipe::$method() exists"
    else
        log_fail "RecoveryRecipe::$method() not found"
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
    echo "Some recovery-recipes contracts are missing implementations."
    exit 1
fi

echo "All recovery-recipes contracts have implementations."
exit 0
