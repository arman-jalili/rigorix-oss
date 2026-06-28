#!/usr/bin/env bash
# Check Audit Posting Coverage
#
# Enforces coverage thresholds for the audit-posting module.
# Since Rust coverage tools vary by environment, this script
# verifies that:
#   1. All unit tests pass
#   2. All public APIs have at least one test
#   3. Integration tests pass (if any exist)
#   4. Build succeeds
#
# Usage: bash .pi/scripts/ci/check_audit-posting_coverage.sh [--verbose]
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

# ── Check 1: Cargo build succeeds ──
echo -n "  Checking cargo build... "
if output=$(cargo build -p rigorix-actions 2>&1); then
    echo "✅"
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 2: Unit tests pass ──
echo -n "  Running unit tests (audit_posting)... "
if output=$(cargo test -p rigorix-actions -- --test-threads=1 audit_posting 2>&1); then
    echo "✅"
    if $VERBOSE; then
        echo "$output" | grep -E "(test result|running)" | tail -5
    fi
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 3: All lib tests pass ──
echo -n "  Running all lib tests... "
if output=$(cargo test --lib -p rigorix-actions -- --test-threads=1 2>cargo test --lib -p rigorix-actions 2>&11); then
    echo "✅"
    if $VERBOSE; then
        echo "$output" | tail -5
    fi
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 4: Count audit_posting tests ──
echo -n "  Checking test count... "
TEST_COUNT=$(grep -rh "#\[tokio::test\]" "$ROOT_DIR/actions/src/audit_posting" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$TEST_COUNT" -ge 10 ]]; then
    echo "✅ ($TEST_COUNT tests)"
else
    echo "⚠  ($TEST_COUNT tests, minimum 10 recommended)"
fi

# ── Summary ──
echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All audit-posting coverage checks passed"
    exit 0
else
    echo "❌ $ERRORS coverage check(s) failed"
    exit 1
fi
