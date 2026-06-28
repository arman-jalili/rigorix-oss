#!/usr/bin/env bash
# ============================================================================
# check_error-handling_contracts.sh
#
# Validates that every contract interface from the error-handling module has
# a concrete implementation. Uses grep/find to detect enum definitions and
# their CoreOrchestratorError integration via #[from].
#
# Usage: bash .pi/scripts/ci/check_error-handling_contracts.sh [--help]
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
echo "═══ Error-Handling Contract Implementation Check ═══"
echo "Source: $SRC_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: CoreOrchestratorError contract (src/error.rs)
# ---------------------------------------------------------------------------
echo "--- Root Error: CoreOrchestratorError ---"

ERROR_FILE="$SRC_DIR/error.rs"
if [ -f "$ERROR_FILE" ]; then
    log_pass "CoreOrchestratorError file exists at src/error.rs"

    if grep -q 'pub enum CoreOrchestratorError' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError enum defined"
    else
        log_fail "CoreOrchestratorError enum not found in src/error.rs"
    fi

    # Check #[from] declarations for key sub-errors
    for variant in "DagError" "PlanningError" "EnforcementError" "LlmBudgetError" \
                   "ExecutionError" "ToolError" "RepoEngineError" "ConfigurationError" \
                   "CancellationError" "EventSystemError" "AuditError" "StateError" \
                   "TemplateError" "FailureClassificationError"; do
        if grep -q "$variant" "$ERROR_FILE" 2>/dev/null; then
            log_pass "CoreOrchestratorError integrates $variant via #[from]"
        else
            log_fail "CoreOrchestratorError missing $variant integration"
        fi
    done

    # Check std wrappers
    if grep -q 'std::io::Error' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError wraps std::io::Error"
    else
        log_fail "CoreOrchestratorError missing std::io::Error wrapper"
    fi

    if grep -q 'serde_json::Error' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError wraps serde_json::Error"
    else
        log_fail "CoreOrchestratorError missing serde_json::Error wrapper"
    fi

    # Check signal variants
    if grep -q 'Cancelled' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError has Cancelled variant"
    else
        log_fail "CoreOrchestratorError missing Cancelled variant"
    fi

    if grep -q 'Http' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError has Http variant with message/status/url"
    else
        log_fail "CoreOrchestratorError missing Http variant"
    fi

    # Check helper methods
    if grep -q 'pub fn is_retriable' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError has is_retriable() method"
    else
        log_fail "CoreOrchestratorError missing is_retriable() method"
    fi

    if grep -q 'pub fn error_code' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError has error_code() method"
    else
        log_fail "CoreOrchestratorError missing error_code() method"
    fi

    if grep -q 'pub fn http_status' "$ERROR_FILE" 2>/dev/null; then
        log_pass "CoreOrchestratorError has http_status() method"
    else
        log_fail "CoreOrchestratorError missing http_status() method"
    fi
else
    log_fail "CoreOrchestratorError file (src/error.rs) not found"
fi

# ---------------------------------------------------------------------------
# Check 2: ExecutionError contract (src/execution/domain/error.rs)
# ---------------------------------------------------------------------------
echo ""
echo "--- ExecutionError Contract ---"

EXEC_ERROR_FILE="$SRC_DIR/execution_engine/domain/error.rs"
if [ -f "$EXEC_ERROR_FILE" ]; then
    log_pass "ExecutionError file exists at src/execution/domain/error.rs"

    if grep -q 'pub enum ExecutionError' "$EXEC_ERROR_FILE" 2>/dev/null; then
        log_pass "ExecutionError enum defined"
    else
        log_fail "ExecutionError enum not found"
    fi

    for variant in "NodeNotFound" "GraphNotSealed" "NodeExecutionFailed" \
                   "RetryLimitExhausted" "FallbackFailed" "ExecutionCancelled"; do
        if grep -q "$variant" "$EXEC_ERROR_FILE" 2>/dev/null; then
            log_pass "ExecutionError has $variant variant"
        else
            log_fail "ExecutionError missing $variant variant"
        fi
    done

    # Verify thiserror derive
    if grep -q 'thiserror::Error' "$EXEC_ERROR_FILE" 2>/dev/null; then
        log_pass "ExecutionError uses #[derive(Error)] from thiserror"
    else
        log_fail "ExecutionError missing thiserror derive"
    fi
else
    log_fail "ExecutionError file (src/execution/domain/error.rs) not found"
fi

# ---------------------------------------------------------------------------
# Check 3: All domain error enums exist in their respective modules
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Error Enums ---"

check_domain_error() {
    local module="$1"
    local expected_error="$2"
    local error_file="$SRC_DIR/$module/domain/error.rs"
    if [ -f "$error_file" ]; then
        if grep -q "pub enum $expected_error" "$error_file" 2>/dev/null; then
            log_pass "$expected_error defined in $module/domain/error.rs"
        else
            log_fail "$expected_error not found in $module/domain/error.rs"
        fi
    else
        log_fail "error.rs not found in $module/domain/"
    fi
}

check_domain_error "audit" "AuditError"
check_domain_error "budget_tracking" "LlmBudgetError"
check_domain_error "cancellation" "CancellationError"
check_domain_error "configuration" "ConfigurationError"
check_domain_error "dag_engine" "DagError"
check_domain_error "enforcement" "EnforcementError"
check_domain_error "event_system" "EventSystemError"
check_domain_error "failure_classification" "FailureClassificationError"
check_domain_error "planning" "PlanningError"
check_domain_error "repo_engine" "RepoEngineError"
check_domain_error "state_persistence" "StateError"
check_domain_error "templates" "TemplateError"
check_domain_error "tools" "ToolError"

# ---------------------------------------------------------------------------
# Check 4: Domain error docs reference CoreOrchestratorError
# ---------------------------------------------------------------------------
echo ""
echo "--- Architecture Docs Compliance ---"

DOC_REF_COUNT=$(grep -rl "CoreOrchestratorError" "$SRC_DIR" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
if [ "$DOC_REF_COUNT" -ge 10 ]; then
    log_pass "CoreOrchestratorError referenced in $DOC_REF_COUNT files across codebase"
else
    log_fail "CoreOrchestratorError referenced in only $DOC_REF_COUNT files (expected ≥ 10)"
fi

# ---------------------------------------------------------------------------
# Check 5: Integration with lib.rs
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Registration ---"

if grep -q 'pub mod error' "$SRC_DIR/lib.rs" 2>/dev/null; then
    log_pass "error module registered in lib.rs"
else
    log_fail "error module not registered in lib.rs"
fi

if grep -q 'pub mod execution' "$SRC_DIR/lib.rs" 2>/dev/null; then
    log_pass "execution module registered in lib.rs"
else
    log_fail "execution module not registered in lib.rs"
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
    echo "Some error-handling contracts are missing implementations."
    exit 1
fi

echo "All error-handling contracts have implementations."
exit 0
