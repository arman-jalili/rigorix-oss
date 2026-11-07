#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Classify the CI stage results and give the gate its verdict (#1289, #1411).
#
#   failure  — a mandatory stage ran and failed, or was skipped when this
#              diff requires it: exit 1
#   cancelled — the run was superseded by a newer one on the same ref: this
#              is the scheduler's fact, not the change's. No verdict — the
#              gate job is skipped by `ci.yml`, and this branch is a guard for
#              the case where it still runs. exit 0, named reason, no
#              accusation.
#   success  — every mandatory stage passed: exit 0
#
# #1411: a failure names the **cause** first and the stages it took down second. The earlier
# line was `❌ Mandatory stage(s) did not pass: lint build test coverage audit …`, which told a
# reader that eight subjects were broken when one edge had skipped seven of them. The cause is
# resolved from `.github/ci-graph.tsv`, the same file the workflow is checked against; a skip
# the graph does not explain is reported as wiring, never attributed to a stage that had
# nothing to do with it.
#
# #1441: a stage this diff cannot reach is **not required**, and `ci.yml` skips it through a
# job-level `if`. The verdict reads the same selection the workflow did (`CI_ALL` /
# `CI_SELECTED`, from `.github/ci-affected.tsv`) and treats a not-required skip as a decision:
# named in the summary, absent from the failure list. A required stage that is skipped is still
# a failure — that is a wiring fault, not a narrowing.
#
# The table is printed either way, so `cancelled` is visible per stage
# instead of appearing only as a red gate.
#
# Usage:
#   .github/scripts/ci_verdict.sh              # the gate
#   .github/scripts/ci_verdict.sh --self-test  # check the verdict, with seeds
# ─────────────────────────────────────────────────────────────────────

set -uo pipefail

here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=ci_stages.sh
. "$here/ci_stages.sh"

# One line per cause, then the stages that did not run because of it. `stages` is the list a
# reader needs; the count is printed with it, because "eight stages" and "one stage" are
# different verdicts on different changes.
ci_verdict_print_failure() {
    local causes="$1" victims="$2" unexplained="$3"
    local cause down stage victim_cause

    for cause in $causes; do
        case "$(ci_stage_result "$cause")" in
            failure) echo "❌ ${cause} failed" ;;
            *)
                echo "❌ ${cause} reported no result"
                echo "   the workflow does not pass RESULT_$(printf '%s' "$cause" | tr '[:lower:]-' '[:upper:]_')"
                ;;
        esac
        down=""
        for entry in $victims; do
            victim_cause="${entry##*:}"
            [ "$victim_cause" = "$cause" ] || continue
            down="$down ${entry%:*}"
        done
        if [ -n "$down" ]; then
            echo "   did not run because of it:${down}"
        fi
    done

    if [ -n "$unexplained" ]; then
        echo "❌ did not run, with no declared failing upstream: ${unexplained}"
        echo "   no edge in .github/ci-graph.tsv attributes these to a failed stage, so the"
        echo "   graph and the run disagree — that is a wiring fault, not a verdict on the change"
    fi

    echo ""
    echo "A stage that ran and failed, or that never ran when this workflow requires it,"
    echo "is a verdict on the change."
}

ci_verdict_print_summary() {
    local causes="$1" victims="$2" unexplained="$3"
    local cause down entry victim_cause
    {
        echo "### ❌ CI verdict: $(printf '%s' "$causes" | wc -w | tr -d ' ') cause(s), $(printf '%s%s' "$victims" "$unexplained" | wc -w | tr -d ' ') stage(s) did not pass"
        echo ""
        for cause in $causes; do
            echo "- **${cause} failed**"
            down=""
            for entry in $victims; do
                victim_cause="${entry##*:}"
                [ "$victim_cause" = "$cause" ] || continue
                down="$down \`${entry%:*}\`"
            done
            [ -n "$down" ] && echo "  - did not run because of it:${down}"
        done
        [ -n "$unexplained" ] && echo "- **did not run, with no declared failing upstream:** ${unexplained}"
    } >>"${GITHUB_STEP_SUMMARY:-/dev/null}"
}

ci_verdict() {
    local failed=() cancelled=() passed=() causes=() victims=() unexplained=() not_required=()
    local stage cause

    for stage in $CI_MANDATORY_STAGES; do
        # A stage this diff cannot reach did not run, by decision. Named, never counted as
        # a pass and never counted as a failure (#1441).
        if ci_stage_not_required "$stage"; then
            not_required+=("$stage")
            continue
        fi
        case "$(ci_stage_result "$stage")" in
            success) passed+=("$stage") ;;
            cancelled) cancelled+=("$stage") ;;
            # `skipped` keeps today's policy: a mandatory stage that this diff *does* require
            # and that did not run is not a pass. `missing` means the wiring is broken.
            *) failed+=("$stage") ;;
        esac
    done

    # The table goes to stdout (the job log) and to the run summary — one source,
    # two readers.
    ci_print_stage_table
    ci_print_stage_table >>"${GITHUB_STEP_SUMMARY:-/dev/null}"

    if [ "${#not_required[@]}" -gt 0 ]; then
        echo "⊘ not required by this diff: ${not_required[*]}"
        echo "   .github/ci-affected.tsv says this change cannot reach them; that is a decision,"
        echo "   not a pass and not a failure — the report names it so a green run is readable"
        {
            echo ""
            echo "⊘ **not required by this diff:** $(printf '`%s` ' "${not_required[@]}")"
        } >>"${GITHUB_STEP_SUMMARY:-/dev/null}"
    fi

    if [ "${#failed[@]}" -gt 0 ]; then
        for stage in "${failed[@]}"; do
            cause="$(ci_stage_cause "$stage")"
            if [ "$cause" = "$stage" ]; then
                causes+=("$stage")
            elif [ -n "$cause" ]; then
                victims+=("${stage}:${cause}")
            else
                unexplained+=("$stage")
            fi
        done
        ci_verdict_print_failure "${causes[*]:-}" "${victims[*]:-}" "${unexplained[*]:-}"
        ci_verdict_print_summary "${causes[*]:-}" "${victims[*]:-}" "${unexplained[*]:-}"
        return 1
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
        } >>"${GITHUB_STEP_SUMMARY:-/dev/null}"
        return 0
    fi

    echo "✅ All mandatory stages this diff requires passed: ${passed[*]}"
    echo "### ✅ All mandatory stages passed" >>"${GITHUB_STEP_SUMMARY:-/dev/null}"
    return 0
}

# ── Self-test ────────────────────────────────────────────────────────────────
# `bash .github/scripts/ci_verdict.sh --self-test`
#
# The property #1411 exists for, asserted rather than described: one failed stage plus its
# skipped dependants must read as one cause and a list of stages that did not run — never as
# eight failures. The fixtures set every stage explicitly, so an ambient RESULT_* in the
# environment cannot make a seed pass for the wrong reason.
ci_verdict_selftest() {
    local failures=0 out status stage

    reset_results() {
        for stage in $CI_MANDATORY_STAGES $CI_REPORTED_STAGES; do
            export "RESULT_$(printf '%s' "$stage" | tr '[:lower:]-' '[:upper:]_')"=success
        done
    }

    expect() {
        local label="$1" want="$2" got="$3"
        if [ "$want" = "$got" ]; then
            echo "  ✅ $label"
        else
            echo "  ❌ $label: want '$want', got '$got'" >&2
            failures=$((failures + 1))
        fi
    }

    expect_absent() {
        local label="$1" needle="$2" haystack="$3"
        case "$haystack" in
            *"$needle"*)
                echo "  ❌ $label: output still contains '$needle'" >&2
                failures=$((failures + 1)) ;;
            *) echo "  ✅ $label" ;;
        esac
    }

    # Seed 1 — the shape of #1411: one failure, dependants skipped. The verdict names `lint`,
    # and does not put the stages that never ran in the failure list. `test` and `coverage` are
    # skipped without an edge that explains it (both need only `workspace`), which is the case
    # the verdict has to report as unexplained rather than blame on the failure.
    reset_results
    RESULT_LINT=failure
    RESULT_TEST=skipped
    RESULT_COVERAGE=skipped
    out="$(GITHUB_STEP_SUMMARY=/dev/null ci_verdict 2>&1)"
    status=$?
    expect "seed — lint failed plus two skips exits 1" 1 "$status"
    case "$out" in
        *"❌ lint failed"*) echo "  ✅ seed — the cause is named first" ;;
        *) echo "  ❌ seed — the verdict does not name lint as the cause" >&2
           failures=$((failures + 1)) ;;
    esac
    expect_absent "seed — the skipped stages are not accused of failing" "did not pass: lint test" "$out"
    case "$out" in
        *"test coverage"*|*"coverage test"*) echo "  ✅ seed — the skipped stages are named as skipped" ;;
        *) echo "  ❌ seed — the skipped stages are not named" >&2
           failures=$((failures + 1)) ;;
    esac

    # Seed 2 — a skip the graph *does* explain: `integration: needs: [test]`, so a failed test
    # is integration's cause. This is what `ci_stage_cause` exists for; seed 1 is the case where
    # the graph explains nothing and the verdict must say so instead of blaming the failure.
    # The edge moved here when `build` was deleted (#1438); the seed follows the graph.
    reset_results
    RESULT_TEST=failure
    RESULT_INTEGRATION=skipped
    out="$(GITHUB_STEP_SUMMARY=/dev/null ci_verdict 2>&1)"
    status=$?
    expect "seed — test failed plus integration skipped exits 1" 1 "$status"
    # `integration` is reported, not mandatory, so it never appears in the failure line — the
    # attribution shows in the table's Cause column, and that is what this asserts. Asserting
    # the failure line instead is how this seed first "passed" by matching nothing.
    case "$out" in
        *"integration |"*"skipped | no | yes | test"*) \
            echo "  ✅ seed — integration is attributed to test through the declared graph" ;;
        *) echo "  ❌ seed — the graph edge test -> integration is not used for attribution" >&2
           failures=$((failures + 1)) ;;
    esac

    # Seed 3 — a stage with no result at all: the wiring is broken, and the verdict says that
    # rather than calling it a failure of the change.
    reset_results
    unset RESULT_CONFORMANCE
    out="$(GITHUB_STEP_SUMMARY=/dev/null ci_verdict 2>&1)"
    expect "seed — a missing result exits 1" 1 $?
    case "$out" in
        *"reported no result"*) echo "  ✅ seed — a missing result is named as missing, not failed" ;;
        *) echo "  ❌ seed — a missing result reads as something else" >&2
           failures=$((failures + 1)) ;;
    esac

    # Seed 4 (#1441) — a stage this diff cannot reach that did not run is a decision, not a
    # failure: exit 0, named as not required, and absent from the failure list.
    reset_results
    RESULT_COVERAGE=skipped
    RESULT_CONFORMANCE=skipped
    CI_ALL=false CI_SELECTED=" lint test portable audit shell wasm-gates docs integration " \
        out="$(GITHUB_STEP_SUMMARY=/dev/null ci_verdict 2>&1)"
    status=$?
    expect "seed — a not-required skip exits 0" 0 "$status"
    case "$out" in
        *"not required by this diff: coverage conformance"*) \
            echo "  ✅ seed — the skipped stages are named as not required" ;;
        *) echo "  ❌ seed — the not-required stages are not named" >&2
           failures=$((failures + 1)) ;;
    esac
    expect_absent "seed — a not-required stage is not accused" "❌" "$out"

    # Seed 5 (#1441) — the same skip, but the diff *does* require the stage: that is a wiring
    # fault, and a narrowing must not be able to hide it.
    reset_results
    RESULT_COVERAGE=skipped
    CI_ALL=false CI_SELECTED=" lint test coverage portable audit conformance shell wasm-gates docs integration " \
        out="$(GITHUB_STEP_SUMMARY=/dev/null ci_verdict 2>&1)"
    status=$?
    expect "seed — a required stage that was skipped still exits 1" 1 "$status"

    # Seed 6 — clean pass still passes, so the seeds above are not passing for a broken reason.
    # Explicitly back to the fail-closed default: an unset selection would be unbound under
    # `set -u`, and a seed that relies on that reads as a verdict rather than a fixture.
    CI_ALL=true
    CI_SELECTED=""
    reset_results
    out="$(GITHUB_STEP_SUMMARY=/dev/null ci_verdict 2>&1)"
    expect "seed — all stages passing exits 0" 0 $?

    if [ "$failures" -ne 0 ]; then
        echo "❌ ci_verdict self-test failed ($failures)" >&2
        return 1
    fi
    echo "✅ ci_verdict self-test: 6 seeds fired, 1 clean pass"
}

if [ "${1:-}" = "--self-test" ]; then
    ci_verdict_selftest
    exit $?
fi

ci_verdict
exit $?
