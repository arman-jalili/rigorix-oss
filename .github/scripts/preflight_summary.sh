#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Render the preflight step report (#1286).
#
# Reads $RUNNER_TEMP/preflight-steps.tsv (written by preflight_step.sh)
# and prints a markdown table to stdout. The workflow appends it to
# $GITHUB_STEP_SUMMARY.
#
# A `timeout` row is rendered as a budget failure that names the step, not
# as a red check for the change — the distinction the job-level timeout
# could not make.
# ─────────────────────────────────────────────────────────────────────

set -uo pipefail

report="${RUNNER_TEMP:-/tmp}/preflight-steps.tsv"

if [ ! -s "$report" ]; then
    echo "## 🏃 Preflight Results"
    echo ""
    echo "_No steps ran — the job ended before the first one reported._"
    exit 0
fi

echo "## 🏃 Preflight Results"
echo ""
echo "| Step | Budget | Elapsed | Result |"
echo "|------|--------|---------|--------|"

timed_out=0
failed=0

while IFS=$'\t' read -r name budget elapsed status; do
    [ -n "${name:-}" ] || continue
    budget_m=$((budget / 60))
    elapsed_m=$((elapsed / 60))
    budget_s=$((budget % 60))
    elapsed_s=$((elapsed % 60))
    case "$status" in
        pass) icon="✅" ;;
        timeout) icon="⏱️ budget exceeded"; timed_out=1 ;;
        *) icon="❌"; failed=1 ;;
    esac
    printf '| %s | %dm %02ds | %dm %02ds | %s |\n' \
        "$name" "$budget_m" "$budget_s" "$elapsed_m" "$elapsed_s" "$icon"
done < "$report"

echo ""

if [ "$timed_out" -eq 1 ]; then
    echo "> ⏱️ **A step exceeded its budget.** That is a budget verdict, not a verdict on the"
    echo "> change: the step named above was slow, not wrong. Re-run the job; if the budget is"
    echo "> exceeded again, the budget is wrong (raise it in \`.github/workflows/preflight.yml\`)"
    echo "> rather than the change."
    echo ""
fi

if [ "$failed" -eq 1 ]; then
    echo "> ❌ **A step failed.** See the failing step's log above for the diagnostic."
    echo ""
fi
