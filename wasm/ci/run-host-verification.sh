#!/usr/bin/env bash
# run-host-verification.sh — criterion 13, with the SDK, in a process that never loads wasm.
#
# Fail-closed on tooling: if npm cannot install or the SDK is absent, this exits non-zero.
# The Node script itself reports a missing SDK as `unverifiable(verifier_unavailable)` — a
# record we could not check is not a record we called wrong, and it is certainly not a pass.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_DIR="$CRATE_DIR/tests/host_verification"
ARTIFACT="${1:-$CRATE_DIR/target/host-verification-artifact.json}"

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

if [ ! -f "$ARTIFACT" ]; then
    echo "== producing an artifact set from the wasm seam =="
    mkdir -p "$(dirname "$ARTIFACT")"
    if ! cargo run --quiet --manifest-path "$CRATE_DIR/Cargo.toml" \
        --example determinism_probe -- \
        "$CRATE_DIR/tests/conformance/vectors/vec-0001.json" >"$ARTIFACT"; then
        echo "❌ could not produce an artifact set" >&2
        exit 1
    fi
fi

echo "== verifying with the independent SDK =="
node verify.mjs "$ARTIFACT"
