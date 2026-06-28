#!/usr/bin/env bash
# ============================================================================
# check_orchestrator_contracts.sh
#
# Validates that every contract interface from the orchestrator module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_orchestrator_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
if [ -d "$PI_DIR/../engine/src" ]; then
    SRC_DIR="$PI_DIR/../engine/src"
elif [ -d "$PI_DIR/../src" ]; then
    SRC_DIR="$PI_DIR/../src"
else
    echo "ERROR: Source directory not found"
    exit 1
fi

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Orchestrator Contract Implementation Check ═══"
echo "Source: $SRC_DIR/orchestrator"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

# OrchestratorService trait → OrchestratorServiceImpl
if grep -q 'pub trait OrchestratorService' "$SRC_DIR/orchestrator/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*OrchestratorService.*for.*OrchestratorServiceImpl' "$SRC_DIR/orchestrator/application/orchestrator_impl.rs" 2>/dev/null; then
        log_pass "OrchestratorService → OrchestratorServiceImpl"
    else
        log_fail "OrchestratorService trait: no OrchestratorServiceImpl found"
    fi
else
    log_fail "OrchestratorService trait not found"
fi

# OrchestratorBuilder trait → OrchestratorBuilderImpl
if grep -q 'pub trait OrchestratorBuilder' "$SRC_DIR/orchestrator/application/builder.rs" 2>/dev/null; then
    if grep -q 'impl.*OrchestratorBuilder.*for.*OrchestratorBuilderImpl' "$SRC_DIR/orchestrator/application/builder_impl.rs" 2>/dev/null; then
        log_pass "OrchestratorBuilder → OrchestratorBuilderImpl"
    else
        log_fail "OrchestratorBuilder trait: no OrchestratorBuilderImpl found"
    fi
else
    log_fail "OrchestratorBuilder trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Domain Entities
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct ExecutionRecord' "$SRC_DIR/orchestrator/domain/record.rs" 2>/dev/null; then
    log_pass "ExecutionRecord struct exists"
else
    log_fail "ExecutionRecord struct not found"
fi

if grep -q 'pub struct PlanningMetadata' "$SRC_DIR/orchestrator/domain/record.rs" 2>/dev/null; then
    log_pass "PlanningMetadata struct exists"
else
    log_fail "PlanningMetadata struct not found"
fi

if grep -q 'pub struct TaskResult' "$SRC_DIR/orchestrator/domain/record.rs" 2>/dev/null; then
    log_pass "TaskResult struct exists"
else
    log_fail "TaskResult struct not found"
fi

if grep -q 'pub struct ExecutionContext' "$SRC_DIR/orchestrator/domain/record.rs" 2>/dev/null; then
    log_pass "ExecutionContext struct exists"
else
    log_fail "ExecutionContext struct not found"
fi

if grep -q 'pub enum ExecutionStatus' "$SRC_DIR/orchestrator/domain/record.rs" 2>/dev/null; then
    log_pass "ExecutionStatus enum exists"
else
    log_fail "ExecutionStatus enum not found"
fi

if grep -q 'pub enum TaskStatus' "$SRC_DIR/orchestrator/domain/record.rs" 2>/dev/null; then
    log_pass "TaskStatus enum exists"
else
    log_fail "TaskStatus enum not found"
fi

if grep -q 'pub struct OrchestratorConfig' "$SRC_DIR/orchestrator/domain/config.rs" 2>/dev/null; then
    log_pass "OrchestratorConfig struct exists"
else
    log_fail "OrchestratorConfig struct not found"
fi

if grep -q 'pub enum OrchestratorError' "$SRC_DIR/orchestrator/domain/error.rs" 2>/dev/null; then
    log_pass "OrchestratorError enum exists"
else
    log_fail "OrchestratorError enum not found"
fi

if grep -q 'pub enum OrchestratorEvent' "$SRC_DIR/orchestrator/domain/event/mod.rs" 2>/dev/null; then
    log_pass "OrchestratorEvent enum exists"
else
    log_fail "OrchestratorEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 3: DTOs
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

for dto in RunInput RunOutput PlanOnlyInput PlanOnlyOutput CancelInput CancelOutput StatusOutput; do
    if grep -q "pub struct $dto" "$SRC_DIR/orchestrator/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 4: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait ExecutionRecordRepository' "$SRC_DIR/orchestrator/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "ExecutionRecordRepository trait defined"
else
    log_fail "ExecutionRecordRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 5: HTTP Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP API Contracts ---"

for path_var in RUN_PATH PLAN_PATH CANCEL_PATH STATUS_PATH; do
    if grep -q "${path_var}" "$SRC_DIR/orchestrator/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "Endpoint $path_var defined"
    else
        log_fail "Endpoint $path_var not found"
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
    echo "Some orchestrator contracts are missing implementations."
    exit 1
fi

echo "All orchestrator contracts have implementations."
exit 0
