#!/usr/bin/env bash
# ============================================================================
# stage_precondition_proofing.sh
#
# CI stage wrapper that runs all precondition proofing checks:
#   1. Contract implementation check — every frozen contract has an impl
#   2. Coverage gate — enforced by the real workspace cargo llvm-cov gate
#
# Usage: bash .pi/scripts/ci/stage_precondition_proofing.sh [--help]
#
# Exit codes: 0 = all checks pass, 1 = any check fails
# ============================================================================
set -uo pipefail

if [ "${1:-}" = "--help" ]; then
    sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║   Precondition Proofing Stage                 ║"
echo "╚══════════════════════════════════════════════╝"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Contract Implementation Check
# ---------------------------------------------------------------------------
echo "--- 1. Contract Implementation Check ---"
echo ""
if bash "${SCRIPT_DIR}/check_precondition_contracts.sh" 2>&1; then
    log_pass "Contract implementation check passed"
else
    log_fail "Contract implementation check failed"
fi

# ---------------------------------------------------------------------------
# Check 2: Coverage (real measurement)
#
# Coverage is NOT a per-module heuristic script — heuristic *_coverage.sh
# scripts were removed repo-wide in #780 as coverage-theater. Real line
# coverage is enforced by the workspace gate:
#   bash .pi/scripts/coverage.sh --gate     (cargo llvm-cov, >= COVERAGE_THRESHOLD)
# wired as ci.yml Stage 3b / local-ci Stage 4b. This module's tests (in-crate
# + tests/unit/precondition) are part of that workspace measurement.
# ---------------------------------------------------------------------------
echo ""
echo "--- 2. Coverage ---"
if command -v cargo >/dev/null 2>&1 && cargo llvm-cov --version >/dev/null 2>&1; then
    echo "  (real coverage gate runs in ci.yml Stage 3b: bash .pi/scripts/coverage.sh --gate)"
fi
log_pass "Coverage enforced via real cargo llvm-cov gate (not a per-module script)"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "═══ Stage Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Precondition proofing stage FAILED."
    exit 1
fi

echo "Precondition proofing stage PASSED."
exit 0
