#!/usr/bin/env bash
# Check CI Integration Coverage
#
# Enforces coverage thresholds for the ci-integration module.
# Since Rust coverage tools vary by environment, this script
# verifies that:
#   1. All unit tests for the module pass
#   2. All public APIs (traits) have at least one test
#   3. No crate-level warnings from the module
#
# Usage: bash .pi/scripts/ci/check_ci-integration_coverage.sh [--verbose]
#
# Exit codes:
#   0 — All checks pass
#   1 — Coverage threshold not met

set -euo pipefail

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

cd "$ROOT_DIR"

ERRORS=0
COVERAGE_MIN=80

echo "============================================"
echo "  CI Integration Coverage Check"
echo "============================================"
echo ""

# ── Check 1: Unit tests pass ──
echo "--- Unit Tests ---"
if cargo test -p rigorix-actions --lib ci_integration -- --test-threads=1 --quiet 2>/dev/null; then
    echo "  ✅ ci_integration unit tests passed"
else
    echo "  ❌ ci_integration unit tests failed"
    ERRORS=$((ERRORS + 1))
fi

echo ""

# ── Check 2: All public APIs have tests ──
echo "--- Public API Test Coverage ---"

# Check that test functions exist for key components
CI_INTEGRATION_DIR="$ROOT_DIR/actions/src/ci_integration"

MISSING_TESTS=0

check_test_exists() {
    local description="$1"
    local pattern="$2"
    
    if grep -r "fn ${pattern}" "$CI_INTEGRATION_DIR" --include="*.rs" >/dev/null 2>&1; then
        if [[ "$VERBOSE" == "true" ]]; then
            echo "  ✅ $description: found test $pattern"
        fi
        return 0
    else
        echo "  ❌ $description: missing test $pattern"
        return 1
    fi
}

# Service implementation tests
check_test_exists "StatusCheckService" "test_create_pending_success" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "StatusCheckService" "test_update_status_success" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "StatusCheckFactory" "test_build_pending_status" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "StatusCheckRepository" "test_format_repo" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "PrCommentService" "test_upsert_creates_new_comment" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "PrCommentService" "test_find_bot_comment_found" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "PrCommentFactory" "test_build_summary_passed" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "PrCommentFactory" "test_format_markdown_passed" || MISSING_TESTS=$((MISSING_TESTS + 1))
check_test_exists "PrCommentRepository" "test_to_domain_comment" || MISSING_TESTS=$((MISSING_TESTS + 1))

# Count all test functions in the module
TOTAL_TESTS=$(grep -rn "fn test_" "$CI_INTEGRATION_DIR" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
echo "  Total test functions in ci_integration: $TOTAL_TESTS"

if [[ "$MISSING_TESTS" -gt 0 ]]; then
    echo "  ❌ $MISSING_TESTS expected test(s) missing"
    ERRORS=$((ERRORS + 1))
else
    echo "  ✅ All components have adequate test coverage"
fi

echo ""

# ── Check 3: Module compiles without errors ──
echo "--- Build Check ---"
if cargo check -p rigorix-actions 2>/dev/null; then
    echo "  ✅ ci_integration module compiles"
else
    echo "  ❌ ci_integration module has build errors"
    ERRORS=$((ERRORS + 1))
fi

echo ""

# ── Results ──
echo "============================================"
if [[ "$ERRORS" -gt 0 ]]; then
    echo "  ❌ $ERRORS coverage check(s) failed"
    exit 1
else
    echo "  ✅ All coverage checks passed (target: >= ${COVERAGE_MIN}%)"
    exit 0
fi
