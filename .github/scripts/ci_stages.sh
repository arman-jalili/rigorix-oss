#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Shared stage table for the CI gate (#1289).
#
# Sourced by ci_report.sh and ci_verdict.sh. Reads the stage results from
# RESULT_<STAGE> environment variables (set from `needs.<job>.result`) and
# renders one table row per stage, so a reader always sees what each stage
# actually reported — `cancelled` included, rather than a bare red gate.
# ─────────────────────────────────────────────────────────────────────

# Mandatory stages: the gate has a verdict on these.
#
# `coverage` was in NEITHER this list nor the reported one (#1411): the workflow set
# RESULT_COVERAGE, the graph declared the stage, the job ran for 45 minutes, and the verdict
# ignored it — so a red coverage run could sit under a green gate. A stage the workflow reports
# must be a stage the table shows; the self-test below now checks that in the other direction
# too.
# `build` left this list with the job (#1438): it decided nothing, uploaded nothing, and five
# stages waited eleven minutes for it. Its two subjects live in `lint` (clippy and the
# conventions suite) and in the stages that compile what they need.
CI_MANDATORY_STAGES="lint test coverage portable audit conformance shell wasm-gates"
# Reported but not required — whether they should be is a separate decision.
CI_REPORTED_STAGES="docs integration"

# This file lives in <repo>/.github/scripts/, so the repo root is two levels up. Only the
# self-test uses it (to cross-check the workflow's env vars); the gate itself stays path-free.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." 2>/dev/null && pwd || echo "")"

ci_stage_result() {
    # Reads RESULT_<STAGE> from the environment. Job ids may contain hyphens
    # (`wasm-gates`), which are not legal in a shell variable name, so they map to
    # underscores: wasm-gates -> RESULT_WASM_GATES. Without this the indirect expansion
    # fails and the stage reads as `missing` — a green job reported as not-run, which is a
    # false verdict in the one place that decides whether a change may merge.
    local var="RESULT_$(printf '%s' "$1" | tr '[:lower:]-' '[:upper:]_')"
    printf '%s' "${!var:-missing}"
}

# ── The declared graph, and why a stage did not run (#1411) ──────────────────
#
# One lint edge hid seven stages: `build: needs: [workspace, lint]` meant a style finding
# skipped the build and everything downstream, and the verdict then accused all eight of
# failing. The fix in the workflow was to drop the edge; the fix here is to say *why* a stage
# did not run, using the graph the workflow is checked against — not a guess, and not the
# list of symptoms.
CI_GRAPH="${REPO_ROOT}/.github/ci-graph.tsv"

# ── what this diff requires (#1441, second half) ────────────────────────────
#
# `.github/ci-affected.tsv` says which subjects a change can reach, and `ci.yml` skips the rest
# through a job-level `if`. That makes "did not run" a *decision* — and a decision the verdict
# must be told about, or it reads a correctly-skipped stage as a failure and blames the change.
#
#   CI_ALL=true            every stage is required (no narrowing: lockfile, workflow, aliases)
#   CI_SELECTED=" a b "    the required set, space-padded so ` a ` cannot match ` ab `
#   neither set            fail closed: everything is required, which is today's behaviour
#
# Fail closed is the whole rule of this epic — a narrowing that cannot be read must not narrow.
CI_ALL="${CI_ALL:-true}"
CI_SELECTED="${CI_SELECTED:-}"

# Is `$1` required by this diff?
ci_stage_required() {
    case "${CI_ALL:-true}" in
        true|True|TRUE) return 0 ;;
    esac
    case "${CI_SELECTED:-}" in
        *" $1 "*) return 0 ;;
    esac
    return 1
}

# Why a stage did not run, when it did not run because this diff cannot reach it.
ci_stage_not_required() {
    ci_stage_is_stage "$1" || return 1
    ci_stage_required "$1" && return 1
    [ "$(ci_stage_result "$1")" = "skipped" ]
}

# Is this a stage the verdict has a result for? `workspace` is a job and a row in the graph,
# but it is a prerequisite rather than a stage: `ci.yml` does not pass its result to the gate.
# Attributing a skip to a node with no input would invent a cause.
ci_stage_is_stage() {
    case " ${CI_MANDATORY_STAGES} ${CI_REPORTED_STAGES} " in
        *" $1 "*) return 0 ;;
    esac
    return 1
}

# The declared `needs` of a stage, comma-separated, or `-`.
ci_stage_needs() {
    local needs=""
    if [ -f "$CI_GRAPH" ]; then
        needs="$(awk -F'\t' -v s="$1" '!/^#/ && $1 == s { print $2; exit }' "$CI_GRAPH")"
    fi
    printf '%s' "${needs:--}"
}

# The transitive `needs` of a stage, nearest first. Bounded: a cycle in the declared graph is
# a fact about the graph, not a reason to hang the merge gate.
ci_stage_upstream() {
    local frontier="$1" seen=" " out="" rounds=0 stage dep next
    while [ -n "$frontier" ] && [ "$rounds" -lt 32 ]; do
        next=""
        for stage in $frontier; do
            for dep in $(ci_stage_needs "$stage" | tr ',' ' '); do
                [ -z "$dep" ] && continue
                [ "$dep" = "-" ] && continue
                case "$seen" in *" $dep "*) continue ;; esac
                seen="$seen$dep "
                out="$out $dep"
                next="$next $dep"
            done
        done
        frontier="$next"
        rounds=$((rounds + 1))
    done
    printf '%s' "${out# }"
}

# The stage that explains `$1` not passing, or nothing when the graph does not explain it.
#   - a stage that failed (or never reported) is its own cause;
#   - a skipped stage is attributed to the nearest declared upstream stage that failed;
#   - anything else prints nothing, and the caller must say so rather than guess.
ci_stage_cause() {
    local stage="$1" dep dep_result
    case "$(ci_stage_result "$stage")" in
        success|cancelled) return 0 ;;
        failure|missing) printf '%s' "$stage"; return 0 ;;
    esac
    for dep in $(ci_stage_upstream "$stage"); do
        ci_stage_is_stage "$dep" || continue
        dep_result="$(ci_stage_result "$dep")"
        case "$dep_result" in
            failure|missing) printf '%s' "$dep"; return 0 ;;
        esac
    done
    return 0
}

ci_stage_icon() {
    case "$1" in
        success) printf '✅' ;;
        failure) printf '❌' ;;
        cancelled) printf '⏹️' ;;
        skipped) printf '⊘' ;;
        missing) printf '❓' ;;
        *) printf '❓' ;;
    esac
}

# print_stage_table <stream: summary|stdout>
ci_print_stage_table() {
    echo "## 🚪 CI stage results"
    echo ""
    echo "| Stage | Result | Mandatory | Required by this diff | Cause |"
    echo "|-------|--------|-----------|-----------------------|-------|"
    local stage result required cause needed
    for stage in $CI_MANDATORY_STAGES; do
        result=$(ci_stage_result "$stage")
        cause=$(ci_stage_cause "$stage")
        needed=$(ci_stage_required "$stage" && echo yes || echo no)
        ci_stage_not_required "$stage" && cause="not required: the diff cannot reach it"
        printf '| %s | %s %s | yes | %s | %s |\n' \
            "$stage" "$(ci_stage_icon "$result")" "$result" "$needed" "${cause:--}"
    done
    for stage in $CI_REPORTED_STAGES; do
        result=$(ci_stage_result "$stage")
        cause=$(ci_stage_cause "$stage")
        needed=$(ci_stage_required "$stage" && echo yes || echo no)
        ci_stage_not_required "$stage" && cause="not required: the diff cannot reach it"
        printf '| %s | %s %s | no | %s | %s |\n' \
            "$stage" "$(ci_stage_icon "$result")" "$result" "$needed" "${cause:--}"
    done
    echo ""
}

# ── Self-test ────────────────────────────────────────────────────────────────
# `bash .github/scripts/ci_stages.sh --self-test`
#
# Guards the one bug that produced a false verdict in the merge gate: a job id with a
# hyphen (`wasm-gates`) became `RESULT_WASM-GATES`, an illegal variable name, so an
# indirect expansion failed and a green job read as `missing` — reported as "did not pass"
# in the table that decides whether a change may merge.
ci_stages_selftest() {
    local failures=0

    expect() {
        local label="$1" want="$2" got="$3"
        if [ "$want" = "$got" ]; then
            echo "  ✅ $label"
        else
            echo "  ❌ $label: want '$want', got '$got'" >&2
            failures=$((failures + 1))
        fi
    }

    # Set first, then read: an assignment prefix is applied to the command *after* its
    # arguments are expanded, so `VAR=x expect ... "$(ci_stage_result ...)"` reads the old
    # value — the same class of mistake as the one this test guards.
    RESULT_WASM_GATES=success
    expect "wasm-gates maps to RESULT_WASM_GATES" success "$(ci_stage_result wasm-gates)"
    RESULT_LINT=failure
    expect "plain name still works" failure "$(ci_stage_result lint)"
    expect "absent stage reads as missing" missing "$(ci_stage_result nonexistent-stage)"

    # The half the value fixtures above cannot prove: that the *workflow* actually supplies
    # a variable for every mandatory stage, under the name this function computes. Without
    # this, a mismatch between a job id and its `RESULT_*` env var only shows up as a red
    # merge gate, reported as "never ran" — the symptom, not the bug.
    local workflow="$REPO_ROOT/.github/workflows/ci.yml" stage var
    if [ -f "$workflow" ]; then
        for stage in $CI_MANDATORY_STAGES; do
            var="RESULT_$(printf '%s' "$stage" | tr '[:lower:]-' '[:upper:]_')"
            if grep -qE "^[[:space:]]*${var}:" "$workflow"; then
                echo "  ✅ workflow supplies $var"
            else
                echo "  ❌ workflow never sets $var (stage '$stage' would read as missing)" >&2
                failures=$((failures + 1))
            fi
        done
    else
        echo "  ⚠️  no workflow to cross-check (not a repo checkout)"
    fi

    # The other half (#1411): every RESULT_* the workflow passes to the gate must belong to a
    # stage the table knows. `coverage` was passed, declared in the graph, and in neither list,
    # so the verdict could not see a red coverage run at all — a stage that runs and does not
    # gate is a report, and a report nobody reads is not one.
    if [ -f "$workflow" ]; then
        local supplied
        supplied="$(grep -oE '^[[:space:]]*RESULT_[A-Z_]+:' "$workflow" | tr -d ' :' | sed 's/^RESULT_//' | sort -u)"
        for var in $supplied; do
            stage="$(printf '%s' "$var" | tr '[:upper:]_' '[:lower:]-')"
            if ci_stage_is_stage "$stage"; then
                echo "  ✅ $var belongs to a stage the table shows ($stage)"
            else
                echo "  ❌ workflow passes $var but '$stage' is in neither stage list" >&2
                failures=$((failures + 1))
            fi
        done
    fi

    if [ "$failures" -ne 0 ]; then
        echo "❌ ci_stages self-test failed ($failures)" >&2
        return 1
    fi
    echo "✅ ci_stages self-test: stage names and result lookups agree"
}

# Only when this file is *run*, not sourced: `ci_verdict.sh --self-test` sources it, and a
# sourced file that reads the caller's `$1` would run the stage self-test and exit before the
# verdict. `BASH_SOURCE[0]` is this file; `$0` is the caller when sourced.
if [ "${BASH_SOURCE[0]}" = "$0" ] && [ "${1:-}" = "--self-test" ]; then
    ci_stages_selftest
    exit $?
fi
