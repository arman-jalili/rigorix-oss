#!/usr/bin/env bash
# run-wasm-gates.sh — every guarantee this crate makes, in one command.
#
# A crate whose value proposition is "the same bytes as the CLI, or the build fails" cannot
# have its gates scattered across ad-hoc shell history. This is the single entrypoint CI and
# a contributor both call.
#
# Each gate is fail-closed. A gate that could not look is a failure, not a skip — with one
# deliberate exception, `host-verification`, which needs npm and therefore needs a network:
# it is *labelled* as not run rather than reported as passing. A silent skip is the failure
# mode the rest of these gates exist to prevent.
#
# Usage:
#   bash wasm/ci/run-wasm-gates.sh            # everything except host verification
#   bash wasm/ci/run-wasm-gates.sh --with-npm # include host verification (needs npm)
set -uo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CRATE_DIR"

WITH_NPM=0
[ "${1:-}" = "--with-npm" ] && WITH_NPM=1

FAILED=0
RAN=0
SKIPPED=()

gate() {
    local name="$1"
    shift
    printf '\n── %s ─────────────────────────────────────────\n' "$name"
    if "$@"; then
        RAN=$((RAN + 1))
    else
        echo "❌ gate failed: $name"
        FAILED=$((FAILED + 1))
        RAN=$((RAN + 1))
    fi
}

gate "domain isolation"        bash ci/check-domain-isolation.sh
gate "bundle independence"     bash ci/check-bundle-independence.sh
gate "bundle size"             bash ci/check-bundle-size.sh
gate "failure tiers"           bash ci/check-failure-tiers.sh
gate "conformance"             bash ci/run-conformance.sh
gate "panic sites"             bash ../.pi/scripts/validate-panic-sites.sh
gate "unit + contract tests"   cargo test --quiet
gate "release build for wasm"  cargo build --quiet --release --target wasm32-unknown-unknown
gate "workspace pins"          bash ../.pi/scripts/validate-workspace-pins.sh

if [ "$WITH_NPM" -eq 1 ]; then
    gate "host verification (npm)" bash ci/run-host-verification.sh
else
    SKIPPED+=("host verification — needs npm; run with --with-npm (criterion 13)")
fi

printf '\n════════════════════════════════════════════════\n'
if [ "${#SKIPPED[@]}" -gt 0 ]; then
    echo "Not run (labelled, not silently passing):"
    for item in "${SKIPPED[@]}"; do echo "  · $item"; done
fi
if [ "$FAILED" -ne 0 ]; then
    echo "❌ $FAILED of $RAN gate(s) failed"
    exit 1
fi
echo "✅ $RAN gate(s) passed"
