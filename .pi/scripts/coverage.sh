#!/usr/bin/env bash
# ============================================================================
# coverage.sh — workspace line-coverage measurement (cargo-llvm-cov).
#
# Produces the REAL coverage number (not heuristic grep checks):
#   cargo llvm-cov --workspace --summary-only
#
# Usage:
#   bash .pi/scripts/coverage.sh            # measure + print summary
#   bash .pi/scripts/coverage.sh --lcov     # also write target/coverage.lcov
#   bash .pi/scripts/coverage.sh --gate     # fail if lines < COVERAGE_THRESHOLD
#
# Env:
#   COVERAGE_THRESHOLD  minimum line-coverage % (default: 60)
#
# Baseline: 78.92% line coverage (2026-08-31, workspace).
# ============================================================================
set -uo pipefail

COVERAGE_THRESHOLD="${COVERAGE_THRESHOLD:-60}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE=0
LCOV=0

for arg in "$@"; do
  case "$arg" in
    --gate) GATE=1 ;;
    --lcov) LCOV=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "cargo-llvm-cov not found — run .pi/scripts/install_coverage_tools.sh" >&2
  exit 1
fi

cd "$ROOT"

echo "═══ Workspace coverage (cargo llvm-cov) ═══"
if [ "$LCOV" -eq 1 ]; then
  mkdir -p target
  SUMMARY="$(cargo llvm-cov --workspace --summary-only --lcov --output-path target/coverage.lcov)"
else
  SUMMARY="$(cargo llvm-cov --workspace --summary-only)"
fi

# Print the per-file table + TOTAL row (summary tail), then extract the
# TOTAL line-coverage percentage from the final line.
echo "$SUMMARY" | tail -12
COVERAGE="$(echo "$SUMMARY" | tail -1 | grep -oE '[0-9]+\.[0-9]+%' | head -1 | sed 's/%//')"
echo ""
echo "Line coverage: ${COVERAGE}% (threshold: ${COVERAGE_THRESHOLD}%)"

if [ "$GATE" -eq 1 ]; then
  if awk "BEGIN { exit !($COVERAGE >= $COVERAGE_THRESHOLD) }"; then
    echo "✓ Coverage gate passed"
  else
    echo "✗ Coverage below threshold (${COVERAGE}% < ${COVERAGE_THRESHOLD}%)" >&2
    exit 1
  fi
fi
exit 0
