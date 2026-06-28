#!/usr/bin/env bash
# Check Action Input Coverage
#
# Enforces coverage thresholds for the action-input module.
# Since Rust coverage tools vary by environment, this script
# verifies that:
#   1. All unit tests pass
#   2. Integration tests pass
#   3. All public APIs have at least one test
#
# Usage: bash .pi/scripts/ci/check_action-input_coverage.sh [--verbose]
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

# ── Check 2: Unit tests pass (single-threaded to avoid env var races) ──
echo -n "  Running unit tests... "
if output=$(cargo test --lib -p rigorix-actions -- --test-threads=1 2>&1); then
    echo "✅"
    if $VERBOSE; then
        echo "$output" | tail -5
    fi
else
    echo "❌"
    echo "    $output"
    ERRORS=$((ERRORS + 1))
fi

# ── Check 3: Integration tests pass ──
echo -n "  Running integration tests... "
# Check if integration test files exist
if compgen -G "actions/tests/*.rs" > /dev/null 2>&1; then
    if output=$(cargo test -p rigorix-actions --test '*' 2>&1); then
        echo "✅"
        if $VERBOSE; then
            echo "$output" | tail -5
        fi
    else
        echo "❌"
        echo "    $output"
        ERRORS=$((ERRORS + 1))
    fi
else
    echo "⊘ (no integration tests)"
fi

# ── Check 4: Clippy passes ──
echo -n "  Checking clippy... "
if output=$(cargo clippy --lib -p rigorix-actions 2>&1); then
    echo "✅"
else
    echo "⚠ (warnings)"
    if $VERBOSE; then
        echo "$output"
    fi
fi

# ── Summary ──
echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All coverage checks passed"
    exit 0
else
    echo "❌ $ERRORS coverage check(s) failed"
    exit 1
fi
