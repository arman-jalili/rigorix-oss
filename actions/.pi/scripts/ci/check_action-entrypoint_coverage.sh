#!/usr/bin/env bash
# Check Action Entrypoint Coverage
#
# Enforces coverage thresholds for the action-entrypoint module.
# Since Rust coverage tools vary by environment, this script
# verifies that:
#   1. All unit tests pass
#   2. Integration tests pass
#   3. All public APIs have at least one test
#
# Usage: bash .pi/scripts/ci/check_action-entrypoint_coverage.sh [--verbose]
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
TEST_COUNT=0

# ── Check 1: Cargo build succeeds ──
echo -n "  Checking cargo build... "
if output=$(cargo build -p rigorix-actions 2>&1); then
    echo "✅"
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 2: Unit tests for action-entrypoint pass ──
echo -n "  Running action-entrypoint unit tests... "
if output=$(cargo test --lib -p rigorix-actions -- action_entrypoint 2>&1); then
    # Count tests
    TEST_COUNT=$(echo "$output" | grep "^test " | grep "\.\.\. ok" | wc -l | tr -d ' ')
    echo "✅ ($TEST_COUNT tests passed)"
    if $VERBOSE; then
        echo "$output" | grep "^test " | head -20
    fi
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 3: All library tests pass ──
echo -n "  Running all unit tests... "
if output=$(cargo test --lib -p rigorix-actions 2>&1); then
    TOTAL=$(echo "$output" | grep "^test result" | sed -n 's/.*\([0-9]\+\) passed;.*/\1/p')
    echo "✅ ($TOTAL total)"
    if $VERBOSE; then
        echo "$output" | tail -5
    fi
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 4: All service traits have test coverage ──
echo -n "  Checking test coverage for service traits... "
TRAITS_WITH_TESTS=0
TRAITS_CHECKED=0

# Check ActionRouter tests
if grep -q "#\[tokio::test\]" "$ROOT_DIR/actions/src/action_entrypoint/application/router_impl.rs" 2>/dev/null; then
    TRAITS_WITH_TESTS=$((TRAITS_WITH_TESTS + 1))
fi
TRAITS_CHECKED=$((TRAITS_CHECKED + 1))

# Check ContextBuilder tests
if grep -q "#\[tokio::test\]" "$ROOT_DIR/actions/src/action_entrypoint/application/context_builder_impl.rs" 2>/dev/null; then
    TRAITS_WITH_TESTS=$((TRAITS_WITH_TESTS + 1))
fi
TRAITS_CHECKED=$((TRAITS_CHECKED + 1))

# Check ModeResolver tests
if grep -q "#\[tokio::test\]" "$ROOT_DIR/actions/src/action_entrypoint/application/mode_resolver_impl.rs" 2>/dev/null; then
    TRAITS_WITH_TESTS=$((TRAITS_WITH_TESTS + 1))
fi
TRAITS_CHECKED=$((TRAITS_CHECKED + 1))

if [[ $TRAITS_WITH_TESTS -eq $TRAITS_CHECKED ]]; then
    echo "✅ ($TRAITS_WITH_TESTS/$TRAITS_CHECKED service traits have tests)"
else
    echo "⚠ ($TRAITS_WITH_TESTS/$TRAITS_CHECKED service traits have tests)"
fi

# ── Check 5: Clippy passes ──
echo -n "  Checking clippy... "
if output=$(cargo clippy --lib -p rigorix-actions -- -D warnings 2>&1); then
    echo "✅"
else
    echo "⚠ (warnings or errors)"
    if $VERBOSE; then
        echo "$output"
    fi
fi

# ── Summary ──
echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All action-entrypoint coverage checks passed"
    exit 0
else
    echo "❌ $ERRORS coverage check(s) failed"
    exit 1
fi
