#!/usr/bin/env bash
# ============================================================================
# check_failure-classification_contracts.sh
#
# Validates that every contract interface from the failure-classification
# module has a concrete implementation. Uses grep/find to detect trait
# definitions and their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_failure-classification_contracts.sh [--help]
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
echo "═══ Failure Classification Contract Implementation Check ═══"
echo "Source: $SRC_DIR/failure_classification"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Domain Entities
# ---------------------------------------------------------------------------
echo "--- Domain Entities ---"

if grep -q 'pub enum FailureType' "$SRC_DIR/failure_classification/domain/failure_type.rs" 2>/dev/null; then
    log_pass "FailureType enum exists"
else
    log_fail "FailureType enum not found"
fi

if grep -q 'pub enum RetryStrategy' "$SRC_DIR/failure_classification/domain/retry_strategy.rs" 2>/dev/null; then
    log_pass "RetryStrategy enum exists"
else
    log_fail "RetryStrategy enum not found"
fi

if grep -q 'pub enum FailureClassificationError' "$SRC_DIR/failure_classification/domain/error.rs" 2>/dev/null; then
    log_pass "FailureClassificationError enum exists"
else
    log_fail "FailureClassificationError enum not found"
fi

if grep -q 'pub enum FailureClassificationEvent' "$SRC_DIR/failure_classification/domain/event/mod.rs" 2>/dev/null; then
    log_pass "FailureClassificationEvent enum exists"
else
    log_fail "FailureClassificationEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Service Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Contracts ---"

if grep -q 'pub trait FailureClassifierService' "$SRC_DIR/failure_classification/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*FailureClassifierService' "$SRC_DIR/failure_classification/application/failure_classifier_service_impl.rs" 2>/dev/null; then
        log_pass "FailureClassifierService → FailureClassifierServiceImpl"
    else
        log_fail "FailureClassifierService trait has no implementation"
    fi
else
    log_fail "FailureClassifierService trait not found"
fi

if grep -q 'pub trait FailureMappingService' "$SRC_DIR/failure_classification/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*FailureMappingService' "$SRC_DIR/failure_classification/application/failure_mapping_service_impl.rs" 2>/dev/null; then
        log_pass "FailureMappingService → FailureMappingServiceImpl"
    else
        log_fail "FailureMappingService trait has no implementation"
    fi
else
    log_fail "FailureMappingService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait StrategyFactory' "$SRC_DIR/failure_classification/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*StrategyFactory' "$SRC_DIR/failure_classification/application/strategy_factory_impl.rs" 2>/dev/null; then
        log_pass "StrategyFactory → StrategyFactoryImpl"
    else
        log_fail "StrategyFactory trait has no implementation"
    fi
else
    log_fail "StrategyFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait PatternRepository' "$SRC_DIR/failure_classification/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "PatternRepository trait defined"
else
    log_fail "PatternRepository trait not found"
fi

if grep -q 'pub trait ClassificationLogRepository' "$SRC_DIR/failure_classification/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "ClassificationLogRepository trait defined"
else
    log_fail "ClassificationLogRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 5: Standalone classify_failure() function
# ---------------------------------------------------------------------------
echo ""
echo "--- Standalone Function ---"

if grep -q 'pub fn classify_failure' "$SRC_DIR/failure_classification/application/classify.rs" 2>/dev/null; then
    log_pass "classify_failure() free function exists"
else
    log_fail "classify_failure() free function not found"
fi

# ---------------------------------------------------------------------------
# Check 6: DTO Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Contracts ---"

if grep -q 'pub struct ClassifyFailureInput' "$SRC_DIR/failure_classification/application/dto/mod.rs" 2>/dev/null; then
    log_pass "ClassifyFailureInput DTO exists"
else
    log_fail "ClassifyFailureInput DTO not found"
fi

if grep -q 'pub struct ClassifyFailureOutput' "$SRC_DIR/failure_classification/application/dto/mod.rs" 2>/dev/null; then
    log_pass "ClassifyFailureOutput DTO exists"
else
    log_fail "ClassifyFailureOutput DTO not found"
fi

if grep -q 'pub struct GetRetryStrategyInput' "$SRC_DIR/failure_classification/application/dto/mod.rs" 2>/dev/null; then
    log_pass "GetRetryStrategyInput DTO exists"
else
    log_fail "GetRetryStrategyInput DTO not found"
fi

if grep -q 'pub enum StrategySource' "$SRC_DIR/failure_classification/application/dto/mod.rs" 2>/dev/null; then
    log_pass "StrategySource enum exists"
else
    log_fail "StrategySource enum not found"
fi

# ---------------------------------------------------------------------------
# Check 7: HTTP API Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP API Contracts ---"

for endpoint in "CLASSIFY_PATH" "STRATEGY_PATH" "CHECK_ELIGIBILITY_PATH" "VALIDATE_CONFIG_PATH" "API_BASE_PATH"; do
    if grep -q "pub const $endpoint" "$SRC_DIR/failure_classification/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "API endpoint $endpoint defined"
    else
        log_fail "API endpoint $endpoint not found"
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
    echo "Some failure-classification contracts are missing implementations."
    exit 1
fi

echo "All failure-classification contracts have implementations."
exit 0
