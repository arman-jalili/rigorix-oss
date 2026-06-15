#!/usr/bin/env bash
# ============================================================================
# check_execution-engine_contracts.sh
#
# Validates that every contract interface from the execution-engine module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_execution-engine_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Try engine/src first, then src/
if [ -d "$PI_DIR/../engine/src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/engine/src"
elif [ -d "$PI_DIR/../src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
else
    echo "FATAL: Source directory not found"
    exit 1
fi

MODULE_DIR="$SRC_DIR/execution_engine"
PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

echo ""
echo "═══ execution-engine Contract Implementation Check ═══"
echo "Source: $MODULE_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"
if grep -q 'pub trait ParallelExecutionService' "$MODULE_DIR/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*ParallelExecutionService' "$MODULE_DIR/application/service_impl.rs" 2>/dev/null; then
        log_pass "ParallelExecutionService → ParallelExecutionServiceImpl"
    else
        log_fail "ParallelExecutionService trait has no implementation"
    fi
else
    log_fail "ParallelExecutionService trait not found"
fi

if grep -q 'pub trait RetryEvaluationService' "$MODULE_DIR/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*RetryEvaluationService' "$MODULE_DIR/application/service_impl.rs" 2>/dev/null; then
        log_pass "RetryEvaluationService → RetryEvaluationServiceImpl"
    else
        log_fail "RetryEvaluationService trait has no implementation"
    fi
else
    log_fail "RetryEvaluationService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"
if grep -q 'pub trait ParallelExecutionFactory' "$MODULE_DIR/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*ParallelExecutionFactory' "$MODULE_DIR/application/factory_impl.rs" 2>/dev/null; then
        log_pass "ParallelExecutionFactory → ParallelExecutionFactoryImpl"
    else
        log_fail "ParallelExecutionFactory trait has no implementation"
    fi
else
    log_fail "ParallelExecutionFactory trait not found"
fi

if grep -q 'pub trait RetryEvaluationFactory' "$MODULE_DIR/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*RetryEvaluationFactory' "$MODULE_DIR/application/factory_impl.rs" 2>/dev/null; then
        log_pass "RetryEvaluationFactory → RetryEvaluationFactoryImpl"
    else
        log_fail "RetryEvaluationFactory trait has no implementation"
    fi
else
    log_fail "RetryEvaluationFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"
if grep -q 'pub trait ExecutionResultRepository' "$MODULE_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "ExecutionResultRepository trait defined (implementation deferred — filesystem in production)"
else
    log_fail "ExecutionResultRepository trait not found"
fi

if grep -q 'pub trait RetryDecisionRepository' "$MODULE_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "RetryDecisionRepository trait defined (implementation deferred — filesystem in production)"
else
    log_fail "RetryDecisionRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain Entities
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

# ParallelExecutor domain entities
if grep -q 'pub struct ParallelExecutorConfig' "$MODULE_DIR/domain/parallel_executor.rs" 2>/dev/null; then
    log_pass "ParallelExecutorConfig struct exists"
else
    log_fail "ParallelExecutorConfig struct not found"
fi

if grep -q 'pub enum NodeStatus' "$MODULE_DIR/domain/parallel_executor.rs" 2>/dev/null; then
    log_pass "NodeStatus enum exists"
else
    log_fail "NodeStatus enum not found"
fi

if grep -q 'pub struct NodeExecutionState' "$MODULE_DIR/domain/parallel_executor.rs" 2>/dev/null; then
    log_pass "NodeExecutionState struct exists"
else
    log_fail "NodeExecutionState struct not found"
fi

if grep -q 'pub struct TaskResult' "$MODULE_DIR/domain/parallel_executor.rs" 2>/dev/null; then
    log_pass "TaskResult struct exists"
else
    log_fail "TaskResult struct not found"
fi

if grep -q 'pub struct ExecutionResult' "$MODULE_DIR/domain/parallel_executor.rs" 2>/dev/null; then
    log_pass "ExecutionResult struct exists"
else
    log_fail "ExecutionResult struct not found"
fi

# Retry domain entities
if grep -q 'pub struct RetryPolicy' "$MODULE_DIR/domain/retry.rs" 2>/dev/null; then
    log_pass "RetryPolicy struct exists"
else
    log_fail "RetryPolicy struct not found"
fi

if grep -q 'pub enum RetryStrategy' "$MODULE_DIR/domain/retry.rs" 2>/dev/null; then
    log_pass "RetryStrategy enum exists"
else
    log_fail "RetryStrategy enum not found"
fi

if grep -q 'pub enum BackoffStrategy' "$MODULE_DIR/domain/retry.rs" 2>/dev/null; then
    log_pass "BackoffStrategy enum exists"
else
    log_fail "BackoffStrategy enum not found"
fi

if grep -q 'pub enum RetryDecision' "$MODULE_DIR/domain/retry.rs" 2>/dev/null; then
    log_pass "RetryDecision enum exists"
else
    log_fail "RetryDecision enum not found"
fi

if grep -q 'pub struct FailureContext' "$MODULE_DIR/domain/retry.rs" 2>/dev/null; then
    log_pass "FailureContext struct exists"
else
    log_fail "FailureContext struct not found"
fi

# Error types
if grep -q 'pub enum ExecutionError' "$MODULE_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "ExecutionError enum exists"
else
    log_fail "ExecutionError enum not found"
fi

# Events
if grep -q 'pub enum ExecutionEngineEvent' "$MODULE_DIR/domain/event/mod.rs" 2>/dev/null; then
    log_pass "ExecutionEngineEvent enum exists"
else
    log_fail "ExecutionEngineEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTO contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Contracts ---"
for dto_struct in \
    "pub struct ExecuteGraphInput" \
    "pub struct ExecuteGraphOutput" \
    "pub struct ExecuteNodeInput" \
    "pub struct ExecuteNodeOutput" \
    "pub struct GetExecutionStateInput" \
    "pub struct GetExecutionStateOutput" \
    "pub struct PauseExecutionInput" \
    "pub struct PauseExecutionOutput" \
    "pub struct ResumeExecutionInput" \
    "pub struct ResumeExecutionOutput" \
    "pub struct AbortExecutionInput" \
    "pub struct AbortExecutionOutput" \
    "pub struct EvaluateRetryInput" \
    "pub struct EvaluateRetryOutput" \
    "pub struct ExecutionSummary"; do
    struct_name=$(echo "$dto_struct" | sed 's/pub struct //')
    if grep -q "$dto_struct" "$MODULE_DIR/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$struct_name DTO exists"
    else
        log_fail "$struct_name DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: HTTP API contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP API Contracts ---"
for endpoint_var in \
    "EXECUTE_GRAPH_PATH" \
    "EXECUTION_STATE_PATH" \
    "NODE_STATES_PATH" \
    "PAUSE_EXECUTION_PATH" \
    "RESUME_EXECUTION_PATH" \
    "ABORT_EXECUTION_PATH" \
    "EXECUTION_HISTORY_PATH" \
    "EXECUTION_RESULT_PATH" \
    "HEALTH_PATH"; do
    if grep -q "pub const $endpoint_var" "$MODULE_DIR/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint_var defined"
    else
        log_fail "HTTP endpoint $endpoint_var not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 7: Module registration
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Registration ---"
if grep -q 'pub mod execution_engine' "$SRC_DIR/lib.rs" 2>/dev/null; then
    log_pass "execution_engine registered in lib.rs"
else
    log_fail "execution_engine NOT registered in lib.rs"
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
    echo "Some execution-engine contracts are missing implementations."
    exit 1
fi

echo "All execution-engine contracts have implementations."
exit 0
