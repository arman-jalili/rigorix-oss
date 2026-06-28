#!/usr/bin/env bash
# ============================================================================
# check_plan-validation_contracts.sh
#
# Validates that every contract interface from the plan-validation module has a
# concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_plan-validation_contracts.sh [--help]
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

PV_DIR="$SRC_DIR/plan_validation"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Plan-Validation Contract Implementation Check ═══"
echo "Source: $PV_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Domain Contracts
# ---------------------------------------------------------------------------
echo "--- Domain Contracts ---"

for entity in ValidationLoopConfig ValidationState ValidationOutcome ValidationReport \
              ValidationIterationReport ValidationLoopError; do
    if grep -q "pub \\(enum\\|struct\\) $entity" "$PV_DIR/domain/error.rs" 2>/dev/null || \
       grep -q "pub \\(enum\\|struct\\) $entity" "$PV_DIR/domain/loop_config.rs" 2>/dev/null || \
       grep -q "pub \\(enum\\|struct\\) $entity" "$PV_DIR/domain/state.rs" 2>/dev/null || \
       grep -q "pub \\(enum\\|struct\\) $entity" "$PV_DIR/domain/outcome.rs" 2>/dev/null || \
       grep -q "pub \\(enum\\|struct\\) $entity" "$PV_DIR/domain/report.rs" 2>/dev/null; then
        log_pass "$entity exists"
    else
        log_fail "$entity not found in domain/"
    fi
done

# ValidationEvent enum
if grep -q 'pub enum ValidationEvent' "$PV_DIR/domain/event/mod.rs" 2>/dev/null; then
    log_pass "ValidationEvent enum exists"
    VARIANT_COUNT=$(grep -cE '    (ValidationStarted|IterationStarted|IterationFailed|ValidationSucceeded|ValidationFailed|BudgetExhausted|ValidationCancelled)' "$PV_DIR/domain/event/mod.rs" 2>/dev/null || echo 0)
    log_pass "ValidationEvent variants: $VARIANT_COUNT (expected 7+)"
else
    log_fail "ValidationEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Service Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Contracts ---"

if grep -q 'pub trait ValidationLoopService' "$PV_DIR/application/service.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*ValidationLoopService' "$PV_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        log_pass "ValidationLoopService → $(basename "$(echo "$ANY_IMPL" | head -1)")"
    else
        log_fail "ValidationLoopService trait has no implementation"
    fi
else
    log_fail "ValidationLoopService trait not found"
fi

if grep -q 'pub trait QualityGateEvaluationService' "$PV_DIR/application/service.rs" 2>/dev/null; then
    log_pass "QualityGateEvaluationService trait exists"
else
    log_fail "QualityGateEvaluationService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait ValidationLoopFactory' "$PV_DIR/application/factory.rs" 2>/dev/null; then
    log_pass "ValidationLoopFactory trait exists"
else
    log_fail "ValidationLoopFactory trait not found"
fi

if grep -q 'pub struct ValidationLoopConfigBuilder' "$PV_DIR/application/factory.rs" 2>/dev/null; then
    log_pass "ValidationLoopConfigBuilder exists"
else
    log_fail "ValidationLoopConfigBuilder not found"
fi

if grep -q 'pub struct ValidationLoopConfigPresets' "$PV_DIR/application/factory.rs" 2>/dev/null; then
    log_pass "ValidationLoopConfigPresets exists"
else
    log_fail "ValidationLoopConfigPresets not found"
fi

# ---------------------------------------------------------------------------
# Check 4: ContextAugmenter
# ---------------------------------------------------------------------------
echo ""
echo "--- ContextAugmenter ---"

if grep -q 'pub struct ContextAugmenter' "$PV_DIR/application/context_augmenter.rs" 2>/dev/null; then
    for method in augment_intent check_repeated_failures; do
        if grep -q "fn $method" "$PV_DIR/application/context_augmenter.rs" 2>/dev/null; then
            log_pass "ContextAugmenter::$method() exists"
        else
            log_fail "ContextAugmenter::$method() not found"
        fi
    done
else
    log_fail "ContextAugmenter not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTOs
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

for dto in ValidateInput ValidateOutput ClassifyNodesInput ClassifyNodesOutput \
           RetryGenerativeNodesInput RetryGenerativeNodesOutput \
           AugmentIntentInput AugmentIntentOutput \
           EvaluateIterationInput EvaluateIterationOutput; do
    if grep -q "pub struct $dto" "$PV_DIR/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

for repo in ValidationReportRepository ValidatedTemplateRepository; do
    if grep -q "pub trait $repo" "$PV_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
        log_pass "$repo trait exists"
    else
        log_fail "$repo trait not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 7: HTTP Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

for endpoint in VALIDATE_PATH REPORT_PATH REPORTS_LIST_PATH RETRY_PATH; do
    if grep -q "pub const $endpoint" "$PV_DIR/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

for req_resp in ValidateRequest ValidateResponse RetryRequest RetryResponse \
                ReportResponse ReportsListResponse ReportSummary \
                ValidationApiError; do
    if grep -q "pub struct $req_resp" "$PV_DIR/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "$req_resp exists"
    else
        log_fail "$req_resp not found"
    fi
done

if grep -q 'pub mod error_codes' "$PV_DIR/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
fi

# ---------------------------------------------------------------------------
# Check 8: Tests exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"

TEST_COUNT=$(grep -r "#\[test\]" "$PV_DIR" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
if [ "$TEST_COUNT" -ge 40 ]; then
    log_pass "$TEST_COUNT tests found (threshold: 40)"
else
    log_fail "Only $TEST_COUNT tests found (requires >= 40)"
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
    echo "Some plan-validation contracts are missing implementations."
    exit 1
fi

echo "All plan-validation contracts have implementations."
exit 0
