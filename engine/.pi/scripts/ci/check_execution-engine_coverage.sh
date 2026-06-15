#!/usr/bin/env bash
# ============================================================================
# check_execution-engine_coverage.sh
#
# Checks that the execution-engine module has sufficient test coverage.
# Verifies that test files exist and contain a minimum number of test
# functions (indicating reasonable coverage).
#
# Usage: bash .pi/scripts/ci/check_execution-engine_coverage.sh [--help]
#
# Exit codes: 0 = coverage threshold met, 1 = insufficient coverage
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
MIN_TESTS=30            # Minimum number of test functions expected
MIN_TEST_FILES=1        # Minimum number of test files expected
PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

echo ""
echo "═══ execution-engine Coverage Check ═══"
echo "Source: $MODULE_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Test files exist
# ---------------------------------------------------------------------------
echo "--- Test Files ---"
test_file="$MODULE_DIR/tests.rs"
if [ -f "$test_file" ]; then
    test_count=$(grep -c '^\s*#\[test\]' "$test_file" 2>/dev/null || echo 0)
    tokio_test_count=$(grep -c '^\s*#\[tokio::test\]' "$test_file" 2>/dev/null || echo 0)
    # Ensure numeric values (handle empty output)
    test_count=${test_count:-0}
    tokio_test_count=${tokio_test_count:-0}
    test_count=$((test_count + 0))
    tokio_test_count=$((tokio_test_count + 0))
    total_tests=$((test_count + tokio_test_count))
    log_pass "tests.rs exists with $total_tests test functions"

    if [ "$total_tests" -ge "$MIN_TESTS" ]; then
        log_pass "Test count ($total_tests) meets minimum threshold ($MIN_TESTS)"
    else
        log_fail "Test count ($total_tests) below minimum threshold ($MIN_TESTS)"
    fi
else
    log_fail "tests.rs not found in $MODULE_DIR"
fi

# ---------------------------------------------------------------------------
# Check 2: Domain entities are tested
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entity Coverage ---"
entities_tested=0
entities_total=0

for entity in "ParallelExecutorConfig" "NodeExecutionState" "NodeStatus" "TaskResult" "ExecutionResult" \
              "RetryPolicy" "RetryStrategy" "BackoffStrategy" "RetryDecision" "FailureContext" \
              "ExecutionError" "ExecutionEngineEvent"; do
    ((entities_total++))
    # Check both direct reference and import via use statements
    if grep -q "$entity" "$test_file" 2>/dev/null || \
       grep -q "$entity" "$MODULE_DIR/tests.rs" 2>/dev/null; then
        ((entities_tested++))
        log_pass "$entity is covered in tests"
    else
        log_fail "$entity has no test coverage"
    fi
done

# ---------------------------------------------------------------------------
# Check 3: Service operations are tested
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Operation Coverage ---"
ops_tested=0
ops_total=0

for op in "execute_graph" "execute_node" "get_execution_state" "pause_execution" \
          "resume_execution" "abort_execution" "evaluate_retry" "compute_backoff" \
          "validate_policy" "is_failure_retriable" "decide" "on_progress"; do
    ((ops_total++))
    if grep -q "$op" "$test_file" 2>/dev/null; then
        ((ops_tested++))
        log_pass "$op is tested"
    else
        log_fail "$op has no test coverage"
    fi
done

# ---------------------------------------------------------------------------
# Check 4: Module is compiled by cargo test
# ---------------------------------------------------------------------------
echo ""
echo "--- Integration Check ---"
if [ -f "$MODULE_DIR/mod.rs" ]; then
    if grep -q '#\[cfg(test)\]' "$MODULE_DIR/mod.rs" 2>/dev/null || grep -q 'pub(crate) mod tests' "$MODULE_DIR/mod.rs" 2>/dev/null; then
        log_pass "Tests are registered in mod.rs"
    else
        log_fail "Tests NOT registered in mod.rs"
    fi
else
    log_fail "mod.rs not found"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "═══ Summary ═══"
echo "  Entities tested: $entities_tested/$entities_total"
echo "  Operations tested: $ops_tested/$ops_total"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    for err in "${ERRORS[@]}"; do
        echo "  FAIL: $err"
    done
    echo ""
    echo "Coverage threshold not met."
    exit 1
fi

echo "execution-engine coverage threshold met."
exit 0
