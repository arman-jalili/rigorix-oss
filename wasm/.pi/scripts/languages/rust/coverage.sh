#!/usr/bin/env bash
# ============================================================================
# coverage.sh — Rust: real line coverage via cargo-llvm-cov
#
# cargo-llvm-cov uses native LLVM instrumentation and works with incremental
# compilation — ~3x faster than cargo-tarpaulin's full instrumented
# recompilation. Produces the REAL workspace line-coverage number.
#
# Usage (normally via the .pi/scripts/coverage.sh dispatcher):
#   bash coverage.sh            # measure workspace coverage
#   bash coverage.sh --gate     # fail if lines < COVERAGE_THRESHOLD
#   bash coverage.sh --lcov     # also write target/coverage.lcov artifact
#
# Env:
#   COVERAGE_THRESHOLD  minimum line-coverage % (default: 60)
# ============================================================================
set -uo pipefail

COVERAGE_THRESHOLD="${COVERAGE_THRESHOLD:-60}"
GATE=0
LCOV=0

for arg in "$@"; do
    case "$arg" in
        --gate) GATE=1 ;;
        --lcov) LCOV=1 ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

# Walk up from CWD to the crate/workspace root (Cargo.toml).
ROOT="$(pwd)"
while [ "$ROOT" != "/" ] && [ ! -f "$ROOT/Cargo.toml" ]; do
    ROOT="$(dirname "$ROOT")"
done
if [ ! -f "$ROOT/Cargo.toml" ]; then
    echo "✗ No Cargo.toml found in this directory tree" >&2
    exit 1
fi
cd "$ROOT" || exit 1

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
    echo "cargo-llvm-cov not found — run .pi/scripts/install_coverage_tools.sh" >&2
    exit 1
fi

echo "═══ Workspace coverage (cargo llvm-cov) ═══"
# --summary-only always prints the table we parse. When --lcov is also
# requested, write the lcov artifact in a second (instrumentation-cached,
# fast) pass — the tool suppresses the console summary in lcov-output mode.
SUMMARY="$(cargo llvm-cov --workspace --summary-only)"
if [ "$LCOV" -eq 1 ]; then
    mkdir -p target
    cargo llvm-cov --workspace --lcov --output-path target/coverage.lcov >/dev/null 2>&1 \
        && echo "lcov artifact written: target/coverage.lcov" \
        || echo "⚠️  could not write target/coverage.lcov" >&2
fi

# Print the per-file table + TOTAL row (summary tail), then extract the
# TOTAL line-coverage percentage from the final line.
echo "$SUMMARY" | tail -12
COVERAGE="$(echo "$SUMMARY" | tail -1 | grep -oE '[0-9]+\.[0-9]+%' | head -1 | sed 's/%//')"
echo ""
echo "Line coverage: ${COVERAGE:-unavailable}% (threshold: ${COVERAGE_THRESHOLD}%)"

if [ -z "$COVERAGE" ]; then
    echo "✗ Could not extract coverage from cargo llvm-cov output" >&2
    exit 1
fi

if [ "$GATE" -eq 1 ]; then
    if awk "BEGIN { exit !($COVERAGE >= $COVERAGE_THRESHOLD) }"; then
        echo "✓ Coverage gate passed"
    else
        echo "✗ Coverage below threshold (${COVERAGE}% < ${COVERAGE_THRESHOLD}%)" >&2
        exit 1
    fi
fi
exit 0
