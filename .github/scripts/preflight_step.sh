#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Run one preflight step under its own budget (#1286).
#
#   preflight_step.sh <name> <budget-seconds> -- <command…>
#
# Why this exists: the workflow's job-level `timeout-minutes` is a single
# number for the whole job, so when it fires the job is reported as a
# failure of the *change*, the artifact upload is cut in half (making the
# next run start cold again), and nothing says which step was slow.
#
# A step budget fails the step it belongs to, names the step, and leaves
# the job's remaining budget for the post-step cache save — which is what
# keeps the next run warm.
#
# Every run appends a row to $RUNNER_TEMP/preflight-steps.tsv so the
# summary can report each step's outcome and elapsed time:
#
#   name <TAB> budget <TAB> elapsed <TAB> pass|fail|timeout
# ─────────────────────────────────────────────────────────────────────

set -uo pipefail

if [ "$#" -lt 3 ]; then
    echo "usage: $0 <name> <budget-seconds> -- <command…>" >&2
    exit 2
fi

name="$1"
budget="$2"
shift 2
if [ "${1:-}" = "--" ]; then
    shift
fi

report="${RUNNER_TEMP:-/tmp}/preflight-steps.tsv"
start=$SECONDS

# GNU coreutils `timeout` on the runner; macOS has no `timeout` by default, so
# the fallback enforces the same budget with `kill` — a missing coreutil must not
# silently turn the budget off.
if command -v timeout > /dev/null 2>&1; then
    timeout --signal=TERM "${budget}s" "$@"
    code=$?
else
    "$@" &
    child=$!
    started=$SECONDS
    code=0
    while kill -0 "$child" 2> /dev/null; do
        if [ $((SECONDS - started)) -ge "$budget" ]; then
            kill -TERM "$child" 2> /dev/null
            wait "$child" 2> /dev/null
            code=124
            break
        fi
        sleep 1
    done
    if [ "$code" -ne 124 ]; then
        wait "$child"
        code=$?
    fi
fi

elapsed=$((SECONDS - start))

case "$code" in
    0) status=pass ;;
    124) status=timeout ;;
    *) status=fail ;;
esac

printf '%s\t%s\t%s\t%s\n' "$name" "$budget" "$elapsed" "$status" >> "$report"

if [ "$status" = timeout ]; then
    # Distinguish "the job ran out of time" from "the change is broken":
    # exit 124 is a budget verdict, and the message says so.
    echo "::error title=Preflight budget exceeded::${name} exceeded its $((budget / 60))m $((budget % 60))s budget (ran ${elapsed}s). This is a budget failure, not a verdict on the change."
fi

exit "$code"
