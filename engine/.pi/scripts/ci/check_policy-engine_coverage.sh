#!/usr/bin/env bash
# ============================================================================
# check_policy-engine_coverage.sh
#
# Validates that the policy-engine module has adequate test coverage.
# Checks that test files exist for each layer and module.
#
# Usage: bash .pi/scripts/ci/check_policy-engine_coverage.sh [--help]
#
# Exit codes: 0 = coverage adequate, 1 = insufficient coverage
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
echo "═══ Policy Engine Coverage Check ═══"
echo "Source: $SRC_DIR/policy_engine"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Each domain file has tests
# ---------------------------------------------------------------------------
echo "--- Domain Tests ---"

for file in rule condition action context config error; do
    FILE_PATH="$SRC_DIR/policy_engine/domain/$file.rs"
    if [ -f "$FILE_PATH" ]; then
        TEST_COUNT=$(grep -c '#\[test\]' "$FILE_PATH" 2>/dev/null || echo 0)
        if [ "$TEST_COUNT" -ge 1 ]; then
            log_pass "$file.rs has $TEST_COUNT tests"
        else
            log_fail "$file.rs has no tests"
        fi
    else
        log_fail "$file.rs not found"
    fi
done

if [ -f "$SRC_DIR/policy_engine/domain/event/mod.rs" ]; then
    TEST_COUNT=$(grep -c '#\[test\]' "$SRC_DIR/policy_engine/domain/event/mod.rs" 2>/dev/null || echo 0)
    if [ "$TEST_COUNT" -ge 1 ]; then
        log_pass "event/mod.rs has $TEST_COUNT tests"
    else
        log_fail "event/mod.rs has no tests"
    fi
else
    log_fail "event/mod.rs not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Application layer tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Application Tests ---"

for file in engine_impl factory_impl; do
    FILE_PATH="$SRC_DIR/policy_engine/application/$file.rs"
    if [ -f "$FILE_PATH" ]; then
        TEST_COUNT=$(grep -c '#\[test\]' "$FILE_PATH" 2>/dev/null)
        TOKIO_COUNT=$(grep -c '#\[tokio::test\]' "$FILE_PATH" 2>/dev/null)
        TOTAL=$((TEST_COUNT + TOKIO_COUNT))
        if [ "$TOTAL" -ge 1 ]; then
            log_pass "$file.rs has $TOTAL tests"
        else
            log_fail "$file.rs has no tests"
        fi
    else
        log_fail "$file.rs not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 3: Infrastructure tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Infrastructure Tests ---"

if [ -f "$SRC_DIR/policy_engine/infrastructure/default_policy_repository.rs" ]; then
    TEST_COUNT=$(grep -c '#\[test\]' "$SRC_DIR/policy_engine/infrastructure/default_policy_repository.rs" 2>/dev/null)
    TOKIO_COUNT=$(grep -c '#\[tokio::test\]' "$SRC_DIR/policy_engine/infrastructure/default_policy_repository.rs" 2>/dev/null)
    TOTAL=$((TEST_COUNT + TOKIO_COUNT))
    if [ "$TOTAL" -ge 1 ]; then
        log_pass "default_policy_repository.rs has $TOTAL tests"
    else
        log_fail "default_policy_repository.rs has no tests"
    fi
fi

# ---------------------------------------------------------------------------
# Check 4: DTO tests
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Tests ---"

if [ -f "$SRC_DIR/policy_engine/application/dto/mod.rs" ]; then
    TEST_COUNT=$(grep -c '#\[test\]' "$SRC_DIR/policy_engine/application/dto/mod.rs" 2>/dev/null || echo 0)
    if [ "$TEST_COUNT" -ge 1 ]; then
        log_pass "dto/mod.rs has $TEST_COUNT tests"
    else
        log_fail "dto/mod.rs has no tests"
    fi
fi

# ---------------------------------------------------------------------------
# Check 5: HTTP contract tests
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contract Tests ---"

if [ -f "$SRC_DIR/policy_engine/interfaces/http/mod.rs" ]; then
    TEST_COUNT=$(grep -c '#\[test\]' "$SRC_DIR/policy_engine/interfaces/http/mod.rs" 2>/dev/null || echo 0)
    if [ "$TEST_COUNT" -ge 1 ]; then
        log_pass "http/mod.rs has $TEST_COUNT tests"
    else
        log_fail "http/mod.rs has no tests"
    fi
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "═══ Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

TOTAL_TESTS=$(grep -r '#\[test\]\|#\[tokio::test\]' "$SRC_DIR/policy_engine/" 2>/dev/null | wc -l | tr -d ' ')
echo "  Total tests in policy_engine module: $TOTAL_TESTS"

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo ""
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Coverage inadequate."
    exit 1
fi

echo ""
echo "Coverage adequate."
exit 0
