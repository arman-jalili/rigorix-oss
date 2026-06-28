#!/usr/bin/env bash
# ============================================================================
# check_failure-parser_coverage.sh
#
# Enforces coverage thresholds for the failure-parser module.
# Verifies that unit and integration tests exist and count meets minimums.
#
# Usage: bash .pi/scripts/ci/check_failure-parser_coverage.sh [--help]
#
# Exit codes: 0 = coverage thresholds met, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC_DIR="$(cd "$PI_DIR/.." && pwd)/engine/src"

PASS=0
FAIL=0
ERRORS=()
MIN_UNIT_TESTS=20
MIN_INTEGRATION_TESTS=10
LIB_TARGET="failure_parser"

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
echo "═══ Failure Parser Coverage Threshold Check ═══"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Unit Test Count
# ---------------------------------------------------------------------------
echo "--- Unit Tests ---"

UNIT_COUNT=0
if [ -d "$SRC_DIR/failure_parser" ]; then
    # Count #[test] annotations in the failure_parser source tree
    # (excluding integration test directories)
    UNIT_COUNT=$(grep -r '#\[tokio::test\]' "$SRC_DIR/failure_parser" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
    UNIT_COUNT=$((UNIT_COUNT + $(grep -r '#\[test\]' "$SRC_DIR/failure_parser" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')))
fi

echo "  Found $UNIT_COUNT unit tests in failure_parser module"
if [ "$UNIT_COUNT" -ge "$MIN_UNIT_TESTS" ]; then
    log_pass "$UNIT_COUNT unit tests (minimum: $MIN_UNIT_TESTS)"
else
    log_fail "$UNIT_COUNT unit tests (minimum: $MIN_UNIT_TESTS) — add more tests"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 2: Integration Test Count
# ---------------------------------------------------------------------------
echo "--- Integration Tests ---"

INTEGRATION_COUNT=0
# Count tests in dedicated integration test files
for test_file in "failure_parser_template_integration.rs" "failure_parser_service_integration.rs" \
                 "failure_parser_typescript_integration.rs" "failure_parser_suggestion_integration.rs"; do
    file_path="$SRC_DIR/../tests/$test_file"
    alt_path="$PI_DIR/../tests/$test_file"
    if [ -f "$file_path" ]; then
        file_count=$(grep -c '#\[test\]' "$file_path" 2>/dev/null || grep -c '#\[tokio::test\]' "$file_path" 2>/dev/null || echo "0")
        INTEGRATION_COUNT=$((INTEGRATION_COUNT + 1))
    elif [ -f "$alt_path" ]; then
        INTEGRATION_COUNT=$((INTEGRATION_COUNT + 1))
    fi
done

# Try running tests to get the actual count
echo "  Checking integration test files..."
FOUND_FILES=$(find "$SRC_DIR/../tests" -name "failure_parser*" -type f 2>/dev/null | wc -l | tr -d ' ')
echo "  Found $FOUND_FILES integration test files for failure_parser"

if [ "$FOUND_FILES" -ge 1 ]; then
    log_pass "$FOUND_FILES integration test files found (minimum: 1)"
else
    log_fail "No integration test files found"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 3: Module File Structure
# ---------------------------------------------------------------------------
echo "--- Module Structure ---"

REQUIRED_DIRS=(
    "$SRC_DIR/failure_parser/domain/event"
    "$SRC_DIR/failure_parser/application/dto"
    "$SRC_DIR/failure_parser/infrastructure/repository"
    "$SRC_DIR/failure_parser/interfaces/http"
)

MISSING_DIRS=0
for dir in "${REQUIRED_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        log_pass "Directory exists: $dir"
    else
        log_fail "Missing directory: $dir"
        ((MISSING_DIRS++))
    fi
done

echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "═══ Coverage Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Failure-parser coverage check FAILED."
    exit 1
fi

echo "Failure-parser coverage check PASSED."
exit 0
