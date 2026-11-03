#!/usr/bin/env bash
# run-host-verification.sh — criterion 13, with the SDK, in a process that never loads wasm.
#
# The independence claim has two halves, and today only one of them is provable:
#
#   1. the crate's artifact can be read by a process that has never loaded the module, and
#      the three-valued vocabulary survives the round trip;
#   2. a *genuine* signature verifies.
#
# (2) needs a real signed envelope, which needs the engine (FR-011). Until then the CI
# assertion is (1) plus the fail-closed case: an artifact whose key the host does not hold
# must be reported `unverifiable(key_not_held)` — NOT `not_verified`. That distinction is the
# whole point of ADR-013, so it is worth asserting even before the engine lands.
#
# Set RIGORIX_HMAC_KEY to exercise the signature path (a real envelope is needed for
# `verified`); EXPECT selects the verdict the caller requires.
set -uo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_DIR="$CRATE_DIR/tests/host_verification"
ARTIFACT="${1:-$CRATE_DIR/target/host-verification-artifact.json}"
KEY="${RIGORIX_HMAC_KEY:-}"
EXPECT="${EXPECT:-unverifiable}"

cd "$HOST_DIR"

if ! command -v npm >/dev/null 2>&1; then
    echo "❌ npm is unavailable: the independence check cannot run" >&2
    exit 1
fi

if [ ! -d node_modules ]; then
    echo "== installing rigorix-verifier =="
    if ! npm install --silent --no-audit --no-fund; then
        echo "❌ the SDK could not be installed: the check cannot run" >&2
        exit 1
    fi
fi

mkdir -p "$(dirname "$ARTIFACT")"
if [ ! -f "$ARTIFACT" ]; then
    echo "== producing an artifact set from the wasm seam =="
    if ! cargo run --quiet --manifest-path "$CRATE_DIR/Cargo.toml" \
        --example determinism_probe -- \
        "$CRATE_DIR/tests/conformance/vectors/vec-0001.json" >"$ARTIFACT"; then
        echo "❌ could not produce an artifact set" >&2
        exit 1
    fi
fi

echo "== verifying with the independent SDK (no wasm loaded) =="
OUTPUT=$(node verify.mjs "$ARTIFACT" "$KEY" 2>&1)
STATUS=$?
echo "   $OUTPUT"
echo "   exit $STATUS"

VERDICT=$(printf '%s' "$OUTPUT" | sed -n 's/.*"verdict":"\([a-z_]*\)".*/\1/p')
if [ -z "$VERDICT" ]; then
    echo "❌ the verifier produced no verdict — a check that cannot speak is not a check" >&2
    exit 1
fi

case "$VERDICT" in
    verified)     [ "$STATUS" -eq 0 ] || { echo "❌ verdict/exit mismatch" >&2; exit 1; } ;;
    not_verified) [ "$STATUS" -eq 1 ] || { echo "❌ verdict/exit mismatch" >&2; exit 1; } ;;
    unverifiable) [ "$STATUS" -eq 2 ] || { echo "❌ verdict/exit mismatch" >&2; exit 1; } ;;
    *) echo "❌ unknown verdict: $VERDICT" >&2; exit 1 ;;
esac

if [ "$VERDICT" != "$EXPECT" ]; then
    echo "❌ expected '$EXPECT', the verifier reported '$VERDICT'" >&2
    if [ "$EXPECT" = "verified" ] && [ "$VERDICT" = "unverifiable" ]; then
        echo "   a 'verified' verdict needs a real signed envelope and the host's key (FR-011)" >&2
    fi
    exit 1
fi

echo "✅ $VERDICT (expected) — the record was read outside the producer"
