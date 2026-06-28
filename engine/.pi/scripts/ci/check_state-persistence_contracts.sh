#!/usr/bin/env bash
# ============================================================================
# check_state-persistence_contracts.sh
#
# Validates that every contract interface from the state-persistence module
# has a concrete implementation. Uses grep/find to detect trait definitions
# and their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_state-persistence_contracts.sh [--help]
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

MODULE="state_persistence"

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ State-Persistence Contract Implementation Check ═══"
echo "Source: $SRC_DIR/$MODULE"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait StateManagerService' "$SRC_DIR/$MODULE/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*StateManagerService' "$SRC_DIR/$MODULE/application/state_manager_service_impl.rs" 2>/dev/null; then
        log_pass "StateManagerService → FileSystemStateManager"
    else
        log_fail "StateManagerService trait has no implementation"
    fi
else
    log_fail "StateManagerService trait not found"
fi

if grep -q 'pub trait GraphManagerService' "$SRC_DIR/$MODULE/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*GraphManagerService' "$SRC_DIR/$MODULE/application/graph_manager_service_impl.rs" 2>/dev/null; then
        log_pass "GraphManagerService → FileSystemGraphManager"
    else
        log_fail "GraphManagerService trait has no implementation"
    fi
else
    log_fail "GraphManagerService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait StateManagerFactory' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*StateManagerFactory' "$SRC_DIR/$MODULE/application/state_manager_factory_impl.rs" 2>/dev/null; then
        log_pass "StateManagerFactory → FileSystemStateManagerFactory"
    else
        log_fail "StateManagerFactory trait has no implementation"
    fi
else
    log_fail "StateManagerFactory trait not found"
fi

if grep -q 'pub trait GraphManagerFactory' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*GraphManagerFactory' "$SRC_DIR/$MODULE/application/graph_manager_factory_impl.rs" 2>/dev/null; then
        log_pass "GraphManagerFactory → FileSystemGraphManagerFactory"
    else
        log_fail "GraphManagerFactory trait has no implementation"
    fi
else
    log_fail "GraphManagerFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

REPO_FILE="$SRC_DIR/$MODULE/infrastructure/repository/mod.rs"

if grep -q 'pub trait StateRepository' "$REPO_FILE" 2>/dev/null; then
    if grep -q 'impl.*StateRepository' "$SRC_DIR/$MODULE/infrastructure/filesystem_state_repository.rs" 2>/dev/null; then
        log_pass "StateRepository → FileSystemStateRepository"
    else
        log_fail "StateRepository trait has no implementation"
    fi
else
    log_fail "StateRepository trait not found"
fi

if grep -q 'pub trait GraphRepository' "$REPO_FILE" 2>/dev/null; then
    if grep -q 'impl.*GraphRepository' "$SRC_DIR/$MODULE/infrastructure/filesystem_graph_repository.rs" 2>/dev/null; then
        log_pass "GraphRepository → FileSystemGraphRepository"
    else
        log_fail "GraphRepository trait has no implementation"
    fi
else
    log_fail "GraphRepository trait not found"
fi

if grep -q 'pub trait ExecutionRecordRepository' "$REPO_FILE" 2>/dev/null; then
    if grep -q 'impl.*ExecutionRecordRepository' "$SRC_DIR/$MODULE/infrastructure/filesystem_execution_record_repository.rs" 2>/dev/null; then
        log_pass "ExecutionRecordRepository → FileSystemExecutionRecordRepository"
    else
        log_fail "ExecutionRecordRepository trait has no implementation"
    fi
else
    log_fail "ExecutionRecordRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain Entities
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

DOMAIN_DIR="$SRC_DIR/$MODULE/domain"

if grep -q 'pub struct ExecutionState' "$DOMAIN_DIR/state.rs" 2>/dev/null; then
    log_pass "ExecutionState struct exists"
else
    log_fail "ExecutionState struct not found"
fi

if grep -q 'pub struct NodeState' "$DOMAIN_DIR/state.rs" 2>/dev/null; then
    log_pass "NodeState struct exists"
else
    log_fail "NodeState struct not found"
fi

if grep -q 'pub enum ExecutionStatus' "$DOMAIN_DIR/state.rs" 2>/dev/null; then
    log_pass "ExecutionStatus enum exists"
else
    log_fail "ExecutionStatus enum not found"
fi

if grep -q 'pub enum NodeStatus' "$DOMAIN_DIR/state.rs" 2>/dev/null; then
    log_pass "NodeStatus enum exists"
else
    log_fail "NodeStatus enum not found"
fi

if grep -q 'pub enum StateError' "$DOMAIN_DIR/error.rs" 2>/dev/null; then
    log_pass "StateError enum exists"
else
    log_fail "StateError enum not found"
fi

if grep -q 'pub enum StateEvent' "$DOMAIN_DIR/event/mod.rs" 2>/dev/null; then
    log_pass "StateEvent enum exists"
else
    log_fail "StateEvent enum not found"
fi

if grep -q 'pub struct ExecutionGraph' "$DOMAIN_DIR/graph.rs" 2>/dev/null; then
    log_pass "ExecutionGraph struct exists"
else
    log_fail "ExecutionGraph struct not found"
fi

if grep -q 'pub struct ExecutionRecord' "$DOMAIN_DIR/context.rs" 2>/dev/null; then
    log_pass "ExecutionRecord struct exists"
else
    log_fail "ExecutionRecord struct not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

DTO_FILE="$SRC_DIR/$MODULE/application/dto/mod.rs"

for dto in SaveStateInput SaveStateOutput LoadStateInput LoadStateOutput \
           NodeStateChangedInput NodeStateChangedOutput \
           ListExecutionsInput ListExecutionsOutput ExecutionSummary; do
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

for endpoint in API_BASE_PATH LIST_EXECUTIONS_PATH GET_EXECUTION_STATE_PATH \
                GET_NODE_STATE_PATH DELETE_EXECUTION_PATH LIST_GRAPHS_PATH \
                GET_GRAPH_PATH HEALTH_PATH; do
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
    echo "Some state-persistence contracts are missing implementations."
    exit 1
fi

echo "All state-persistence contracts have implementations."
exit 0
