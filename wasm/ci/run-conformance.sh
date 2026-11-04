#!/usr/bin/env bash
# run-conformance.sh — the conformance harness as a gate (ADR-009).
#
# Two things must hold, and they are different things:
#
#   1. every committed vector replays and matches what it says it should;
#   2. the deliberately altered vector **does not** match.
#
# (2) is what proves the comparator can fail. A harness without it is a test that passes
# because it compares nothing, which is the failure mode this gate exists to catch.
#
# Fail-closed: zero tests run is a failure. A skipped vector is a failure. There is no
# "conformance skipped" exit path.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VECTOR_DIR="$CRATE_DIR/tests/conformance/vectors"
cd "$CRATE_DIR"

if [ ! -d "$VECTOR_DIR" ]; then
    echo "❌ no conformance vectors — the gate cannot look, so it does not pass" >&2
    exit 1
fi

VECTORS=$(find "$VECTOR_DIR" -name '*.json' | wc -l | tr -d ' ')
echo "== $VECTORS committed vector(s) =="

# Every vector must say where its expected bytes came from. An unlabelled golden is a
# claim nobody can audit (ADR-009).
if ! grep -L '"gate"' "$VECTOR_DIR"/*.json >/dev/null 2>&1; then
    UNLABELLED=$(grep -L '"gate"' "$VECTOR_DIR"/*.json || true)
    if [ -n "$UNLABELLED" ]; then
        echo "❌ vector without provenance: $UNLABELLED" >&2
        exit 1
    fi
fi

# No vector may set `normalised_comparison: true` (ADR-014, #1367). The permission that
# allowed it — "while FR-011 is unimplemented" — lost its precondition when ADR-014
# superseded FR-011, and what it permits is the one thing this harness exists to catch: an
# unpinned envelope is 884 B and a pinned one 877 B for the same decision, so a comparator
# that normalises timestamps reports two different byte strings as equal.
NORMALISED=$(grep -l '"normalised_comparison"[[:space:]]*:[[:space:]]*true' "$VECTOR_DIR"/*.json 2>/dev/null || true)
if [ -n "$NORMALISED" ]; then
    echo "❌ vector(s) set normalised_comparison: true — that is a comparison of the thing" >&2
    echo "   normalised, not of the artifact (ADR-014, #1367):" >&2
    printf '   %s\n' $NORMALISED >&2
    exit 1
fi

# And at least one must be the must-fail vector.
if ! grep -l '"seeded_divergence": true' "$VECTOR_DIR"/*.json >/dev/null 2>&1; then
    echo "❌ no seeded-divergence vector: the comparator is unproven" >&2
    exit 1
fi

echo "== replaying vectors =="
OUTPUT="$(cargo test --quiet --test unit conformance 2>&1 || true)"
printf '%s\n' "$OUTPUT" | tail -5

PASSED=$(printf '%s\n' "$OUTPUT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' | head -1 || echo 0)
if [ "${PASSED:-0}" -eq 0 ]; then
    echo "❌ no conformance test ran — a skip is a failure" >&2
    exit 1
fi

if printf '%s\n' "$OUTPUT" | grep -qE '[1-9][0-9]* failed'; then
    echo "❌ conformance failed" >&2
    exit 1
fi

echo
echo "Conformance OK: $PASSED test(s) ran, and the seeded vector diverged as required."
