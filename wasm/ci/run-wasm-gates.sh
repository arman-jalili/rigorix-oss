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
# `host-verification` is still a *declared* gate: it sits inside the `--with-npm` branch, so a
# grep for the unconditional declarations cannot see it. It is counted anyway, and the
# summary always states the fraction that ran (#1421 — the local run said "10 gates passed"
# while the eleventh, the one that was failing in CI, had never been given a denominator).
#
# Usage:
#   bash wasm/ci/run-wasm-gates.sh            # 10 of 11: everything except host verification
#   bash wasm/ci/run-wasm-gates.sh --with-npm # 11 of 11: + host verification (needs npm) — CI's form
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

# The runbook states how many gates this script has, and a stated count is the cheapest
# claim to leave behind (#1368): `runbook.md` said "eight" while ten ran. Checked before
# the gates run, not as a gate — counting the gates must not change the count.
SELF="$CRATE_DIR/ci/run-wasm-gates.sh"
RUNBOOK="$CRATE_DIR/.pi/architecture/runbook.md"
# Every `gate "` declaration, conditional ones included: the `--with-npm` branch declares a
# gate too, and a count that only sees unconditional lines understates the suite by one.
DECLARED_GATES=$(grep -cE '^[[:space:]]*gate[[:space:]]+"' "$SELF")
STATED_WORD=$(sed -n 's/.*one command, \([a-z][a-z]*\) gates.*/\1/p' "$RUNBOOK" 2>/dev/null | head -1)
if [ -z "$STATED_WORD" ]; then
    echo "❌ cannot read the gate count from $RUNBOOK — a count nobody can find is a count nobody checks" >&2
    exit 1
fi
STATED_NUM=""
n=0
for word in one two three four five six seven eight nine ten eleven twelve; do
    n=$((n + 1))
    [ "$word" = "$STATED_WORD" ] && STATED_NUM="$n"
done
if [ -z "$STATED_NUM" ]; then
    echo "❌ runbook.md states '${STATED_WORD} gates', which this check cannot convert to a number" >&2
    exit 1
fi
if [ "$STATED_NUM" -ne "$DECLARED_GATES" ]; then
    echo "❌ runbook.md says '${STATED_WORD} gates' (${STATED_NUM}); this script declares ${DECLARED_GATES}" >&2
    echo "   fix the sentence or add the gate — a count nobody compares is a count that drifts" >&2
    exit 1
fi
echo "runbook gate count: ${DECLARED_GATES} (matches runbook.md)"

gate "domain isolation"        bash ci/check-domain-isolation.sh
gate "bundle independence"     bash ci/check-bundle-independence.sh
gate "failure tiers"           bash ci/check-failure-tiers.sh
gate "conformance"             bash ci/run-conformance.sh
gate "panic sites"             bash ../.pi/scripts/validate-panic-sites.sh
gate "unit + contract tests"   cargo test --quiet
gate "release build for wasm"  bash "$CRATE_DIR/ci/wasm-build.sh"
# #1349: the build gate above proves the crate compiles; this one proves the
# artifact is callable. It packages the release module the way a host consumes it,
# loads it in Node, and calls all four exports — so a module with no exported
# symbol fails here instead of passing every gate.
gate "module smoke (a host can call it)" bash ci/run-module-smoke.sh
# After the smoke gate, because it produces the packaged artifact this measures:
# a size ceiling needs a subject, and the subject is what a host loads.
gate "bundle size"             bash ci/check-bundle-size.sh
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
# The denominator, always. The old summary stated a bare count, which was true and
# misleading: the suite has eleven subjects, and the missing one was the gate failing in CI
# (#1421). A reader could not tell full coverage from a fraction.
if [ "$FAILED" -ne 0 ]; then
    echo "❌ $FAILED of $DECLARED_GATES gate(s) failed"
    exit 1
fi
echo "✅ $RAN of $DECLARED_GATES gate(s) ran"
