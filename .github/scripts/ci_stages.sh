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
CI_MANDATORY_STAGES="lint build test audit conformance shell wasm-gates"
# Reported but not required — whether they should be is a separate decision.
CI_REPORTED_STAGES="docs integration"

ci_stage_result() {
    # Reads RESULT_<STAGE> from the environment. Job ids may contain hyphens
    # (`wasm-gates`), which are not legal in a shell variable name, so they map to
    # underscores: wasm-gates -> RESULT_WASM_GATES. Without this the indirect expansion
    # fails and the stage reads as `missing` — a green job reported as not-run, which is a
    # false verdict in the one place that decides whether a change may merge.
    local var="RESULT_$(printf '%s' "$1" | tr '[:lower:]-' '[:upper:]_')"
    printf '%s' "${!var:-missing}"
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
    echo "| Stage | Result | Mandatory |"
    echo "|-------|--------|-----------|"
    local stage result required
    for stage in $CI_MANDATORY_STAGES; do
        result=$(ci_stage_result "$stage")
        printf '| %s | %s %s | yes |\n' "$stage" "$(ci_stage_icon "$result")" "$result"
    done
    for stage in $CI_REPORTED_STAGES; do
        result=$(ci_stage_result "$stage")
        printf '| %s | %s %s | no |\n' "$stage" "$(ci_stage_icon "$result")" "$result"
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

    if [ "$failures" -ne 0 ]; then
        echo "❌ ci_stages self-test failed ($failures)" >&2
        return 1
    fi
    echo "✅ ci_stages self-test: stage names and result lookups agree"
}

if [ "${1:-}" = "--self-test" ]; then
    ci_stages_selftest
    exit $?
fi
