#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Classify the CI stage results and give the gate its verdict (#1289).
#
#   failure  — a mandatory stage ran and failed (or was skipped, which our
#              policy treats as not-run-and-required): exit 1
#   cancelled — the run was superseded by a newer one on the same ref: this
#              is the scheduler's fact, not the change's. No verdict — the
#              gate job is skipped by `ci.yml`, and this branch is a guard for
#              the case where it still runs. exit 0, named reason, no
#              accusation.
#   success  — every mandatory stage passed: exit 0
#
# The table is printed either way, so `cancelled` is visible per stage
# instead of appearing only as a red gate.
# ─────────────────────────────────────────────────────────────────────

set -uo pipefail

here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=ci_stages.sh
. "$here/ci_stages.sh"

failed=()
cancelled=()
passed=()

for stage in $CI_MANDATORY_STAGES; do
    result=$(ci_stage_result "$stage")
    case "$result" in
        success) passed+=("$stage") ;;
        cancelled) cancelled+=("$stage") ;;
        # `skipped` keeps today's policy: a mandatory stage that did not run
        # is not a pass. `missing` means the wiring is broken — also not a pass.
        *) failed+=("$stage") ;;
    esac
done

# The table goes to stdout (the job log) and to the run summary — one source,
# two readers.
ci_print_stage_table
ci_print_stage_table >> "${GITHUB_STEP_SUMMARY:-/dev/null}"

if [ "${#failed[@]}" -gt 0 ]; then
    echo "❌ Mandatory stage(s) did not pass: ${failed[*]}"
    echo ""
    echo "A stage that ran and failed, or that never ran when this workflow requires it,"
    echo "is a verdict on the change."
    echo "### ❌ Mandatory stage(s) did not pass: ${failed[*]}" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
    exit 1
fi

if [ "${#cancelled[@]}" -gt 0 ]; then
    echo "⏹️ No verdict: the run was superseded (cancelled: ${cancelled[*]})."
    echo "   That is a property of the scheduler, not of the change — a newer run on"
    echo "   the same ref carries the verdict."
    echo "::notice title=Run superseded::No verdict on this commit — ${cancelled[*]} were cancelled by a newer run on the same ref. The successor run carries the verdict."
    {
        echo "### ⏹️ No verdict — run superseded"
        echo ""
        echo "A newer run on the same ref cancelled: ${cancelled[*]}."
        echo "This is the scheduler's fact, not the change's; the successor run carries the verdict."
    } >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
    exit 0
fi

echo "✅ All mandatory stages passed: ${passed[*]}"
echo "### ✅ All mandatory stages passed" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
exit 0
