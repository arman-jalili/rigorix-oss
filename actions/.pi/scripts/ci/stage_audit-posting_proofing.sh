#!/usr/bin/env bash
# Stage: Audit Posting Proofing
#
# CI stage that validates the audit-posting module:
#   1. Contract implementation check
#   2. Coverage threshold check
#   3. Build verification
#
# Usage: bash .pi/scripts/ci/stage_audit-posting_proofing.sh [--verbose]
#
# Exit codes:
#   0 — All checks pass
#   1 — One or more checks fail

set -euo pipefail

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "═══════════════════════════════════════════════"
echo "  Stage: audit-posting Proofing"
echo "═══════════════════════════════════════════════"
echo ""

# ── Stage 1: Contract Implementation Check ──
echo "─── Stage 1: Contract Implementation Check ───"
if $VERBOSE; then
    bash "$SCRIPT_DIR/check_audit-posting_contracts.sh" --verbose
else
    bash "$SCRIPT_DIR/check_audit-posting_contracts.sh"
fi
CONTRACTS_RESULT=$?
echo ""

# ── Stage 2: Coverage Check ──
echo "─── Stage 2: Coverage Check ───"
if $VERBOSE; then
    bash "$SCRIPT_DIR/check_audit-posting_coverage.sh" --verbose
else
    bash "$SCRIPT_DIR/check_audit-posting_coverage.sh"
fi
COVERAGE_RESULT=$?
echo ""

# ── Stage 3: Build Verification ──
echo "─── Stage 3: Build Verification ───"
echo -n "  Checking cargo build... "
if output=$(cargo build -p rigorix-actions 2>&1); then
    echo "✅"
    BUILD_RESULT=0
else
    echo "❌"
    echo "    $output"
    BUILD_RESULT=1
fi
echo ""

# ── Summary ──
echo "═══════════════════════════════════════════════"
echo "  Stage Results"
echo "═══════════════════════════════════════════════"

TOTAL_FAILURES=0

if [[ $CONTRACTS_RESULT -eq 0 ]]; then
    echo "  ✅ Contract Implementation Check: PASSED"
else
    echo "  ❌ Contract Implementation Check: FAILED"
    TOTAL_FAILURES=$((TOTAL_FAILURES + 1))
fi

if [[ $COVERAGE_RESULT -eq 0 ]]; then
    echo "  ✅ Coverage Check: PASSED"
else
    echo "  ❌ Coverage Check: FAILED"
    TOTAL_FAILURES=$((TOTAL_FAILURES + 1))
fi

if [[ $BUILD_RESULT -eq 0 ]]; then
    echo "  ✅ Build Verification: PASSED"
else
    echo "  ❌ Build Verification: FAILED"
    TOTAL_FAILURES=$((TOTAL_FAILURES + 1))
fi

echo ""

if [[ $TOTAL_FAILURES -eq 0 ]]; then
    echo "✅ Stage 'audit-posting Proofing' PASSED"
    exit 0
else
    echo "❌ Stage 'audit-posting Proofing' FAILED ($TOTAL_FAILURES failure(s))"
    exit 1
fi
