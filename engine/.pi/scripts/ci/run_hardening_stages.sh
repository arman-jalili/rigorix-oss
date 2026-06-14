#!/usr/bin/env bash
# Hardening Stage Runner
#
# Runs all 10 hardening stages as defined in the architecture pipeline.
# Each stage runs its scripts and reports pass/fail.
# A stage failure prevents MR merge (allow_failure: No).
#
# Usage: bash .pi/scripts/ci/run_hardening_stages.sh [--stages <list>] [--verbose]
#
# Stages:
#   1. docs_policy          — MR traceability, docs sync guard
#   2. architecture_conformance — 11+ architectural contract checks
#   3. lint                 — ruff/format/coverage
#   4. static_analysis      — mypy/sonar/import-boundaries/sanity
#   5. unit                 — domain/app/contract/verification tests
#   6. integration          — integration tests
#   7. security             — SBOM/Trivy/secret scan/deps audit
#   8. migration_verify     — Migration checks (conditional)
#   9. package_build        — Docker build (conditional, main only)
#   10. release_readiness   — Runbook/observability/policy checks

set -euo pipefail

PI_DIR=".pi"
if [ ! -d "$PI_DIR" ] && [ -d "engine/.pi" ]; then
    PI_DIR="engine/.pi"
fi
SCRIPTS_DIR="${PI_DIR}/scripts/ci"
CI_DIR="${PI_DIR}/ci"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

VERBOSE=false
ALL_STAGES=true
SELECTED_STAGES=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --stages)
            ALL_STAGES=false
            IFS=',' read -ra SELECTED_STAGES <<< "$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

should_run_stage() {
    local stage="$1"
    if [[ "$ALL_STAGES" == "true" ]]; then
        return 0
    fi
    for s in "${SELECTED_STAGES[@]}"; do
        if [[ "$s" == "$stage" ]]; then
            return 0
        fi
    done
    return 1
}

log_stage_start() {
    echo ""
    echo -e "${BLUE}═══ Stage $1: $2 ═══${NC}"
}

log_stage_pass() {
    echo -e "  ${GREEN}✓ Stage $1 PASSED${NC}"
}

log_stage_fail() {
    echo -e "  ${RED}✗ Stage $1 FAILED${NC}"
    echo "  $2"
}

log_stage_skip() {
    echo -e "  ${YELLOW}⊘ Stage $1 SKIPPED${NC} — $2"
}

TOTAL_STAGES=0
PASSED_STAGES=0
FAILED_STAGES=0
SKIPPED_STAGES=0

declare -a STAGE_RESULTS=()

run_stage() {
    local stage_num="$1"
    local stage_name="$2"
    local stage_script="$3"
    local conditional="$4"  # "always", "main_only", "migration_only", "sonar_required"

    if ! should_run_stage "$stage_name"; then
        return 0
    fi

    ((TOTAL_STAGES++))

    # Check conditional
    case "$conditional" in
        main_only)
            if [[ "${CI_COMMIT_BRANCH:-}" != "main" && "${GITHUB_REF_NAME:-}" != "main" ]]; then
                log_stage_skip "$stage_name" "Not on main branch"
                ((SKIPPED_STAGES++))
                return 0
            fi
            ;;
        migration_only)
            local has_migration=false
            if command -v git &>/dev/null; then
                if git diff --name-only HEAD~1 HEAD 2>/dev/null | grep -qE "(alembic|migrations?|database.*schema)"; then
                    has_migration=true
                fi
            fi
            if [[ "$has_migration" == "false" ]]; then
                log_stage_skip "$stage_name" "No migration changes detected"
                ((SKIPPED_STAGES++))
                return 0
            fi
            ;;
        sonar_required)
            if [[ -z "${SONAR_TOKEN:-}" || -z "${SONAR_HOST_URL:-}" ]]; then
                log_stage_skip "$stage_name" "SONAR_TOKEN or SONAR_HOST_URL not set"
                ((SKIPPED_STAGES++))
                return 0
            fi
            ;;
    esac

    log_stage_start "$stage_num" "$stage_name"

    if [[ ! -f "$stage_script" ]]; then
        log_stage_skip "$stage_name" "Script not found: $stage_script"
        ((SKIPPED_STAGES++))
        return 0
    fi

    local output
    if output=$(bash "$stage_script" 2>&1); then
        log_stage_pass "$stage_name"
        ((PASSED_STAGES++))
        STAGE_RESULTS+=("PASS:$stage_name")
        if [[ "$VERBOSE" == "true" ]]; then
            echo "$output"
        fi
    else
        log_stage_fail "$stage_name" "$output"
        ((FAILED_STAGES++))
        STAGE_RESULTS+=("FAIL:$stage_name:$(echo "$output" | head -5)")
    fi
}

echo "╔══════════════════════════════════════════════════════╗"
echo "║           Guardian Hardening Stage Runner            ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""
echo "Commit: ${CI_COMMIT_SHA:-${GITHUB_SHA:-unknown}}"
echo "Branch: ${CI_COMMIT_BRANCH:-${GITHUB_REF_NAME:-unknown}}"
echo "Pipeline: ${CI_PIPELINE_ID:-local}"
echo ""

# Stage 1: Docs Policy
run_stage "1" "docs_policy" \
    "${SCRIPTS_DIR}/stage_docs_policy.sh" \
    "always"

# Stage 2: Architecture Conformance
run_stage "2" "architecture_conformance" \
    "${SCRIPTS_DIR}/check_architecture_conformance.sh" \
    "always"

# Stage 3: Lint
run_stage "3" "lint" \
    "${SCRIPTS_DIR}/stage_lint.sh" \
    "always"

# Stage 4: Static Analysis
run_stage "4" "static_analysis" \
    "${SCRIPTS_DIR}/stage_static_analysis.sh" \
    "always"

# Stage 5: Unit Tests
run_stage "5" "unit" \
    "${SCRIPTS_DIR}/stage_unit.sh" \
    "always"

# Stage 6: Integration Tests
run_stage "6" "integration" \
    "${SCRIPTS_DIR}/stage_integration.sh" \
    "always"

# Stage 7: Security
run_stage "7" "security" \
    "${SCRIPTS_DIR}/stage_security.sh" \
    "always"

# Stage 8: Migration Verify (conditional)
run_stage "8" "migration_verify" \
    "${SCRIPTS_DIR}/stage_migration_verify.sh" \
    "migration_only"

# Stage 9: Package Build (conditional)
run_stage "9" "package_build" \
    "${SCRIPTS_DIR}/stage_package_build.sh" \
    "main_only"

# Stage 10: Release Readiness
run_stage "10" "release_readiness" \
    "${SCRIPTS_DIR}/stage_release_readiness.sh" \
    "always"

# Stage 11: Configuration Proofing
run_stage "11" "configuration_proofing" \
    "${SCRIPTS_DIR}/stage_configuration_proofing.sh" \
    "always"

# Stage 12: Audit Proofing
run_stage "12" "audit_proofing" \
    "${SCRIPTS_DIR}/stage_audit_proofing.sh" \
    "always"

# Stage 13: Cancellation Proofing
run_stage "13" "cancellation_proofing" \
    "${SCRIPTS_DIR}/stage_cancellation_proofing.sh" \
    "always"

# Stage 14: Failure Classification Proofing
run_stage "14" "failure-classification_proofing" \
    "${SCRIPTS_DIR}/stage_failure-classification_proofing.sh" \
    "always"

# Stage 15: Event-System Proofing
run_stage "15" "event-system_proofing" \
    "${SCRIPTS_DIR}/stage_event-system_proofing.sh" \
    "always"

# Stage 16: Enforcement Proofing
run_stage "16" "enforcement_proofing" \
    "${SCRIPTS_DIR}/stage_enforcement_proofing.sh" \
    "always"

# Stage 17: Budget-Tracking Proofing
run_stage "17" "budget-tracking_proofing" \
    "${SCRIPTS_DIR}/stage_budget-tracking_proofing.sh" \
    "always"

# Stage 18: State-Persistence Proofing
run_stage "18" "state-persistence_proofing" \
    "${SCRIPTS_DIR}/stage_state-persistence_proofing.sh" \
    "always"

# ── Summary ──

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║               Hardening Stage Summary                ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

for result in "${STAGE_RESULTS[@]}"; do
    IFS=':' read -ra parts <<< "$result"
    status="${parts[0]}"
    name="${parts[1]}"
    detail="${parts[2]:-}"

    case "$status" in
        PASS)
            echo -e "  ${GREEN}✓${NC} ${name}"
            ;;
        FAIL)
            echo -e "  ${RED}✗${NC} ${name}"
            if [[ -n "$detail" ]]; then
                echo -e "     ${RED}${detail}${NC}"
            fi
            ;;
        SKIP)
            echo -e "  ${YELLOW}⊘${NC} ${name}"
            ;;
    esac
done

echo ""
echo -e "  ${GREEN}Passed:   ${PASSED_STAGES}${NC}"
echo -e "  ${RED}Failed:   ${FAILED_STAGES}${NC}"
echo -e "  ${YELLOW}Skipped:  ${SKIPPED_STAGES}${NC}"
echo "  Total:    ${TOTAL_STAGES}"
echo ""

if [[ ${FAILED_STAGES} -gt 0 ]]; then
    echo -e "${RED}✗ Hardening FAILED. ${FAILED_STAGES} stage(s) did not pass.${NC}"
    echo "MR cannot be merged until all mandatory stages pass."
    exit 1
fi

echo -e "${GREEN}✓ All hardening stages passed. MR is ready for merge.${NC}"
exit 0
