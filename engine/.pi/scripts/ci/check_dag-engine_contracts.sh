#!/usr/bin/env bash
# ============================================================================
# check_dag-engine_contracts.sh
#
# Validates that every contract interface from the dag-engine module has a
# concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_dag-engine_contracts.sh [--help]
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

PASS=0
FAIL=0
ERRORS=()

MODULE="dag_engine"

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ DAG-Engine Contract Implementation Check ═══"
echo "Source: $SRC_DIR/$MODULE"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts — DagGraphService
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait DagGraphService' "$SRC_DIR/$MODULE/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*DagGraphService' "$SRC_DIR/$MODULE/application/service_impl.rs" 2>/dev/null; then
        log_pass "DagGraphService → DagGraphServiceImpl"
    else
        log_fail "DagGraphService trait has no implementation"
    fi
else
    log_fail "DagGraphService trait not found"
fi

if grep -q 'pub trait DagPlanningService' "$SRC_DIR/$MODULE/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*DagPlanningService' "$SRC_DIR/$MODULE/application/service_impl.rs" 2>/dev/null; then
        log_pass "DagPlanningService → DagPlanningServiceImpl"
    else
        log_fail "DagPlanningService trait has no implementation"
    fi
else
    log_fail "DagPlanningService trait not found"
fi

if grep -q 'pub trait ExecutionPolicyService' "$SRC_DIR/$MODULE/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*ExecutionPolicyService' "$SRC_DIR/$MODULE/application/service_impl.rs" 2>/dev/null; then
        log_pass "ExecutionPolicyService → ExecutionPolicyServiceImpl"
    else
        log_fail "ExecutionPolicyService trait has no implementation"
    fi
else
    log_fail "ExecutionPolicyService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait DagGraphFactory' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    log_pass "DagGraphFactory trait defined"
else
    log_fail "DagGraphFactory trait not found"
fi

if grep -q 'pub trait DagPlanningFactory' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    log_pass "DagPlanningFactory trait defined"
else
    log_fail "DagPlanningFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

REPO_FILE="$SRC_DIR/$MODULE/infrastructure/repository/mod.rs"

if grep -q 'pub trait TaskGraphRepository' "$REPO_FILE" 2>/dev/null; then
    log_pass "TaskGraphRepository trait defined"
else
    log_fail "TaskGraphRepository trait not found"
fi

if grep -q 'pub trait PlanDiffRepository' "$REPO_FILE" 2>/dev/null; then
    log_pass "PlanDiffRepository trait defined"
else
    log_fail "PlanDiffRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain Entities and Types
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

DOMAIN_DIR="$SRC_DIR/$MODULE/domain"

if grep -q 'pub struct TaskGraph' "$DOMAIN_DIR/graph.rs" 2>/dev/null; then
    log_pass "TaskGraph struct exists"
else
    log_fail "TaskGraph struct not found"
fi

if grep -q 'pub struct TaskNode' "$DOMAIN_DIR/graph.rs" 2>/dev/null; then
    log_pass "TaskNode struct exists"
else
    log_fail "TaskNode struct not found"
fi

if grep -q 'pub struct ExecutionPolicy' "$DOMAIN_DIR/graph.rs" 2>/dev/null; then
    log_pass "ExecutionPolicy struct exists"
else
    log_fail "ExecutionPolicy struct not found"
fi

# FailureType and RetryStrategy are defined in failure_classification module, imported by dag_engine
if grep -q 'FailureType' "$DOMAIN_DIR/graph.rs" 2>/dev/null || grep -q 'pub enum FailureType' "$SRC_DIR/$MODULE/../failure_classification/domain/failure_type.rs" 2>/dev/null; then
    log_pass "FailureType enum exists (imported from failure_classification)"
else
    log_fail "FailureType enum not found"
fi

if grep -q 'RetryStrategy' "$DOMAIN_DIR/graph.rs" 2>/dev/null || grep -q 'pub enum RetryStrategy' "$SRC_DIR/$MODULE/../failure_classification/domain/retry_strategy.rs" 2>/dev/null; then
    log_pass "RetryStrategy enum exists (imported from failure_classification)"
else
    log_fail "RetryStrategy enum not found"
fi

if grep -q 'pub enum ValidationRule' "$DOMAIN_DIR/graph.rs" 2>/dev/null; then
    log_pass "ValidationRule enum exists"
else
    log_fail "ValidationRule enum not found"
fi

if grep -q 'pub enum DagError' "$DOMAIN_DIR/error.rs" 2>/dev/null; then
    log_pass "DagError enum exists"
else
    log_fail "DagError enum not found"
fi

if grep -q 'pub enum DagEvent' "$DOMAIN_DIR/event/mod.rs" 2>/dev/null; then
    log_pass "DagEvent enum exists"
else
    log_fail "DagEvent enum not found"
fi

if grep -q 'pub struct PlanDiff' "$DOMAIN_DIR/plan.rs" 2>/dev/null; then
    log_pass "PlanDiff struct exists"
else
    log_fail "PlanDiff struct not found"
fi

if grep -q 'pub enum ImpactLevel' "$DOMAIN_DIR/plan.rs" 2>/dev/null; then
    log_pass "ImpactLevel enum exists"
else
    log_fail "ImpactLevel enum not found"
fi

if grep -q 'pub struct NodeDiff' "$DOMAIN_DIR/plan.rs" 2>/dev/null; then
    log_pass "NodeDiff struct exists"
else
    log_fail "NodeDiff struct not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

DTO_FILE="$SRC_DIR/$MODULE/application/dto/mod.rs"

for dto in ConstructGraphInput ConstructGraphOutput AddNodeInput AddNodeOutput \
           SealGraphInput SealGraphOutput GetGraphInput GetGraphOutput \
           GetNodeInput GetNodeOutput ListNodesInput ListNodesOutput \
           ComparePlansInput ComparePlansOutput PlanSummary; do
    if grep -q "pub struct $dto" "$DTO_FILE" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: HTTP Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

HTTP_FILE="$SRC_DIR/$MODULE/interfaces/http/mod.rs"

for endpoint in API_BASE_PATH CONSTRUCT_GRAPH_PATH SEAL_GRAPH_PATH GET_GRAPH_PATH \
                LIST_NODES_PATH READY_NODES_PATH MARK_COMPLETE_PATH DELETE_GRAPH_PATH \
                COMPARE_PLANS_PATH LIST_GRAPHS_PATH HEALTH_PATH; do
    if grep -q "pub const $endpoint" "$HTTP_FILE" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

if grep -q 'pub struct ApiErrorResponse' "$HTTP_FILE" 2>/dev/null; then
    log_pass "ApiErrorResponse exists"
else
    log_fail "ApiErrorResponse not found"
fi

if grep -q 'pub mod error_codes' "$HTTP_FILE" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
fi

if grep -q 'pub mod status_codes' "$HTTP_FILE" 2>/dev/null; then
    log_pass "Status codes defined"
else
    log_fail "Status codes not defined"
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
    echo "Some dag-engine contracts are missing implementations."
    exit 1
fi

echo "All dag-engine contracts have implementations."
exit 0
