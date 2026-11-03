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
    local var="RESULT_$(printf '%s' "$1" | tr '[:lower:]' '[:upper:]')"
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
