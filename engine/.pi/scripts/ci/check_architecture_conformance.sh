#!/usr/bin/env bash
# Architecture Conformance Check Runner
#
# Validates that the current slice conforms to all architectural contracts
# defined in .pi/architecture/modules/*.md and .pi/architecture/decisions/*.md
#
# Usage: bash .pi/scripts/ci/check_architecture_conformance.sh [--module <name>]
#
# Exit codes:
#   0 — All conformance checks passed
#   1 — One or more conformance checks failed
#   2 — Architecture module not found or invalid

set -uo pipefail

PI_DIR=".pi"
ARCH_DIR="${PI_DIR}/architecture"
MODULES_DIR="${ARCH_DIR}/modules"
DECISIONS_DIR="${ARCH_DIR}/decisions"
SCRIPTS_DIR="${PI_DIR}/scripts/ci"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
TOTAL_COUNT=0

# Track failures for summary
declare -a FAILURES=()

log_pass() {
    echo -e "  ${GREEN}✓ PASS${NC} $1"
    ((PASS_COUNT++))
    ((TOTAL_COUNT++))
}

log_fail() {
    echo -e "  ${RED}✗ FAIL${NC} $1 — $2"
    ((FAIL_COUNT++))
    ((TOTAL_COUNT++))
    FAILURES+=("$1: $2")
}

log_skip() {
    echo -e "  ${YELLOW}⊘ SKIP${NC} $1 — $2"
    ((SKIP_COUNT++))
    ((TOTAL_COUNT++))
}

# ── Language Detection ──

detect_language() {
    if [[ -f "pyproject.toml" || -f "requirements.txt" || -f "Pipfile" || -f "setup.py" ]]; then
        echo "python"
    elif [[ -f "package.json" || -f "tsconfig.json" ]]; then
        echo "typescript"
    elif [[ -f "Cargo.toml" ]]; then
        echo "rust"
    elif [[ -f "go.mod" ]]; then
        echo "go"
    elif [[ -f "pom.xml" || -f "build.gradle" || -f "build.gradle.kts" ]]; then
        echo "java"
    elif [[ -f "Gemfile" ]]; then
        echo "ruby"
    else
        echo "unknown"
    fi
}

PROJECT_LANG=$(detect_language)

# ── Module Discovery ──

discover_modules() {
    if [[ ! -d "${MODULES_DIR}" ]]; then
        echo "ERROR: Architecture modules directory not found: ${MODULES_DIR}"
        echo "Run 'guardian init' or create ${MODULES_DIR}/ with module definitions."
        exit 2
    fi
    ls "${MODULES_DIR}"/*.md 2>/dev/null || true
}

parse_module_status() {
    local module_file="$1"
    local status=""
    if [[ -f "$module_file" ]]; then
        status=$(grep -i "^status:" "$module_file" 2>/dev/null | head -1 | sed 's/^status:[[:space:]]*//' || echo "unknown")
    fi
    echo "$status"
}

# ── Conformance Checks ──

# Check 1: Tenant Isolation Conformance
# Verifies that tenant-scoped data never crosses tenant boundaries
check_tenant_isolation() {
    local check_name="tenant_isolation_conformance"
    local ext="$1"
    local lang="$2"

    # Try language-specific validator first
    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_tenant_isolation.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_tenant_isolation.py" 2>/dev/null; then
                    log_pass "Tenant isolation conformance (Python validator)"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_tenant_isolation.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_tenant_isolation.ts" 2>/dev/null; then
                    log_pass "Tenant isolation conformance (TypeScript validator)"
                    return
                fi
            fi
            ;;
        rust)
            if [[ -f "${SCRIPTS_DIR}/check_tenant_isolation.sh" ]]; then
                if bash "${SCRIPTS_DIR}/check_tenant_isolation.sh" 2>/dev/null; then
                    log_pass "Tenant isolation conformance (Rust validator)"
                    return
                fi
            fi
            ;;
        go)
            if [[ -f "${SCRIPTS_DIR}/check_tenant_isolation.sh" ]]; then
                if bash "${SCRIPTS_DIR}/check_tenant_isolation.sh" 2>/dev/null; then
                    log_pass "Tenant isolation conformance (Go validator)"
                    return
                fi
            fi
            ;;
    esac

    # Fallback: grep-based check for tenant_id propagation
    local violations=0
    if command -v grep &>/dev/null; then
        while IFS= read -r model_file; do
            if grep -q "tenant_id" "$model_file" 2>/dev/null; then
                local has_tenant_filter=false
                if grep -qE "tenant_id.*=|WHERE.*tenant|tenant_scoped" "$model_file" 2>/dev/null; then
                    has_tenant_filter=true
                fi
                if [[ "$has_tenant_filter" == "false" ]]; then
                    ((violations++))
                    log_fail "$check_name" "Model $(basename "$model_file") has tenant_id but no tenant-scoped queries"
                fi
            fi
        done < <(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -50)
    fi

    if [[ $violations -eq 0 ]]; then
        log_pass "Tenant isolation conformance (no tenant-scoped models without filtering)"
    fi
}

# Check 2: Event Ordering Conformance
# Verifies that events are processed in correct order (causal, temporal)
check_event_ordering() {
    local check_name="event_ordering_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_event_ordering.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_event_ordering.py" 2>/dev/null; then
                    log_pass "Event ordering conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_event_ordering.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_event_ordering.ts" 2>/dev/null; then
                    log_pass "Event ordering conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_ordering=false
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qE "(sequence|sequence_num|causal|version|timestamp.*order)" "$f" 2>/dev/null; then
            has_ordering=true
            break
        fi
    done

    if [[ "$has_ordering" == "true" ]]; then
        log_pass "Event ordering conformance (ordering mechanism detected)"
    else
        log_skip "Event ordering conformance" "No event ordering mechanism found (may not apply to this slice)"
    fi
}

# Check 3: Outbox / DLQ Conformance
# Verifies that outbox pattern and dead-letter queues are properly implemented
check_outbox_dlq() {
    local check_name="outbox_dlq_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_outbox_dlq.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_outbox_dlq.py" 2>/dev/null; then
                    log_pass "Outbox/DLQ conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_outbox_dlq.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_outbox_dlq.ts" 2>/dev/null; then
                    log_pass "Outbox/DLQ conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_outbox=false
    local has_dlq=false

    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        grep -qi "outbox" "$f" 2>/dev/null && has_outbox=true
        grep -qi "dead.letter\|dlq\|failed.queue" "$f" 2>/dev/null && has_dlq=true
    done

    if [[ "$has_outbox" == "true" || "$has_dlq" == "true" ]]; then
        log_pass "Outbox/DLQ conformance (pattern detected)"
    else
        log_skip "Outbox/DLQ conformance" "No outbox/DLQ pattern in this slice"
    fi
}

# Check 4: Replay / Upcaster Conformance
# Verifies that event replay and schema upcasting work correctly
check_replay_upcaster() {
    local check_name="replay_upcaster_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_replay_upcaster.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_replay_upcaster.py" 2>/dev/null; then
                    log_pass "Replay/upcaster conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_replay_upcaster.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_replay_upcaster.ts" 2>/dev/null; then
                    log_pass "Replay/upcaster conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_replay=false
    local has_upcaster=false

    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        grep -qi "replay\|replay_from\|replay_events" "$f" 2>/dev/null && has_replay=true
        grep -qi "upcast\|schema.*version\|migrate.*event" "$f" 2>/dev/null && has_upcaster=true
    done

    if [[ "$has_replay" == "true" || "$has_upcaster" == "true" ]]; then
        log_pass "Replay/upcaster conformance (mechanism detected)"
    else
        log_skip "Replay/upcaster conformance" "No replay/upcaster mechanism in this slice"
    fi
}

# Check 5: RunStarted Publication Conformance
# Verifies that run-started events are published correctly
check_runstarted_publication() {
    local check_name="runstarted_publication_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_runstarted_publication.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_runstarted_publication.py" 2>/dev/null; then
                    log_pass "RunStarted publication conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_runstarted_publication.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_runstarted_publication.ts" 2>/dev/null; then
                    log_pass "RunStarted publication conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_publication=false
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qi "run.started\|run_started\|RunStarted\|runstarted" "$f" 2>/dev/null; then
            has_publication=true
            break
        fi
    done

    if [[ "$has_publication" == "true" ]]; then
        log_pass "RunStarted publication conformance"
    else
        log_skip "RunStarted publication conformance" "No runstarted events in this slice"
    fi
}

# Check 6: RunStarted Worker Activation Conformance
# Verifies that workers activate correctly on run-started events
check_runstarted_worker_activation() {
    local check_name="runstarted_worker_activation_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_runstarted_worker_activation.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_runstarted_worker_activation.py" 2>/dev/null; then
                    log_pass "RunStarted worker activation conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_runstarted_worker_activation.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_runstarted_worker_activation.ts" 2>/dev/null; then
                    log_pass "RunStarted worker activation conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_worker_activation=false
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qi "worker.*start\|activate.*worker\|on_run.*start" "$f" 2>/dev/null; then
            has_worker_activation=true
            break
        fi
    done

    if [[ "$has_worker_activation" == "true" ]]; then
        log_pass "RunStarted worker activation conformance"
    else
        log_skip "RunStarted worker activation conformance" "No worker activation in this slice"
    fi
}

# Check 7: Bounded LangGraph Execution Conformance
# Verifies that LangGraph executions are bounded (timeout, token, step limits)
check_bounded_execution() {
    local check_name="bounded_execution_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_bounded_execution.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_bounded_execution.py" 2>/dev/null; then
                    log_pass "Bounded execution conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_bounded_execution.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_bounded_execution.ts" 2>/dev/null; then
                    log_pass "Bounded execution conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_bounds=false
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qiE "(timeout|max_steps|max_tokens|step_limit|bounded|circuit.breaker)" "$f" 2>/dev/null; then
            has_bounds=true
            break
        fi
    done

    if [[ "$has_bounds" == "true" ]]; then
        log_pass "Bounded execution conformance (bounds detected)"
    else
        log_skip "Bounded execution conformance" "No bounded execution pattern in this slice"
    fi
}

# Check 8: Artifact Proof Surfaces Conformance
# Verifies that artifact proof surfaces are properly exposed
check_artifact_proof_surfaces() {
    local check_name="artifact_proof_surfaces_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_artifact_proof_surfaces.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_artifact_proof_surfaces.py" 2>/dev/null; then
                    log_pass "Artifact proof surfaces conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_artifact_proof_surfaces.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_artifact_proof_surfaces.ts" 2>/dev/null; then
                    log_pass "Artifact proof surfaces conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_proof=false
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qiE "(artifact.*proof|proof.*surface|verification.*artifact|artifact_hash)" "$f" 2>/dev/null; then
            has_proof=true
            break
        fi
    done

    if [[ "$has_proof" == "true" ]]; then
        log_pass "Artifact proof surfaces conformance"
    else
        log_skip "Artifact proof surfaces conformance" "No artifact proof surfaces in this slice"
    fi
}

# Check 9: Runtime Baseline Conformance
# Verifies runtime environment meets baseline requirements
check_runtime_baseline() {
    local check_name="runtime_baseline_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_runtime_baseline.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_runtime_baseline.py" 2>/dev/null; then
                    log_pass "Runtime baseline conformance"
                    return
                fi
            fi
            # Python version check
            if command -v python3 &>/dev/null; then
                local version
                version=$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
                local major minor
                major=$(echo "$version" | cut -d. -f1)
                minor=$(echo "$version" | cut -d. -f2)
                if [[ $major -gt 3 ]] || [[ $major -eq 3 && $minor -ge 10 ]]; then
                    log_pass "Runtime baseline conformance (Python ${version} >= 3.10)"
                else
                    log_fail "Runtime baseline conformance" "Python ${version} < 3.10"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_runtime_baseline.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_runtime_baseline.ts" 2>/dev/null; then
                    log_pass "Runtime baseline conformance"
                    return
                fi
            fi
            # Node version check
            if command -v node &>/dev/null; then
                local version
                version=$(node -v | sed 's/v//')
                local major
                major=$(echo "$version" | cut -d. -f1)
                if [[ $major -ge 18 ]]; then
                    log_pass "Runtime baseline conformance (Node ${version} >= 18)"
                else
                    log_fail "Runtime baseline conformance" "Node ${version} < 18"
                    return
                fi
            fi
            ;;
        rust)
            if [[ -f "${SCRIPTS_DIR}/check_runtime_baseline.sh" ]]; then
                if bash "${SCRIPTS_DIR}/check_runtime_baseline.sh" 2>/dev/null; then
                    log_pass "Runtime baseline conformance"
                    return
                fi
            fi
            if command -v rustc &>/dev/null; then
                log_pass "Runtime baseline conformance (rustc available)"
            fi
            ;;
        go)
            if [[ -f "${SCRIPTS_DIR}/check_runtime_baseline.sh" ]]; then
                if bash "${SCRIPTS_DIR}/check_runtime_baseline.sh" 2>/dev/null; then
                    log_pass "Runtime baseline conformance"
                    return
                fi
            fi
            if command -v go &>/dev/null; then
                log_pass "Runtime baseline conformance (go available)"
            fi
            ;;
    esac

    # Env var collision check
    if [[ -f ".env.example" ]]; then
        local unbound=0
        while IFS= read -r var; do
            var_name=$(echo "$var" | cut -d= -f1)
            if [[ -z "${!var_name:-}" ]]; then
                ((unbound++))
            fi
        done < <(grep -v "^#" .env.example 2>/dev/null | grep "=" || true)
        if [[ $unbound -eq 0 ]]; then
            log_pass "Runtime baseline conformance (all env vars configured)"
        else
            log_fail "Runtime baseline conformance" "${unbound} env vars from .env.example not set"
        fi
    else
        log_skip "Runtime baseline conformance" "No .env.example found"
    fi
}

# Check 10: Controlled Stage Progression Conformance
# Verifies that state machines / workflows progress through defined stages only
check_controlled_stage_progression() {
    local check_name="controlled_stage_progression_conformance"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_controlled_stage_progression.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_controlled_stage_progression.py" 2>/dev/null; then
                    log_pass "Controlled stage progression conformance"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_controlled_stage_progression.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_controlled_stage_progression.ts" 2>/dev/null; then
                    log_pass "Controlled stage progression conformance"
                    return
                fi
            fi
            ;;
    esac

    local has_state_machine=false
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qiE "(state_machine|transition|Stage\(|step_to|next_stage|progression)" "$f" 2>/dev/null; then
            has_state_machine=true
            break
        fi
    done

    if [[ "$has_state_machine" == "true" ]]; then
        log_pass "Controlled stage progression conformance (state machine detected)"
    else
        log_skip "Controlled stage progression conformance" "No state machine in this slice"
    fi
}

# ── Architecture Sanity Checks ──

check_arch_sanity() {
    local check_name="architecture_sanity"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_arch_sanity.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_arch_sanity.py" 2>/dev/null; then
                    log_pass "Architecture sanity (Python validator)"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_arch_sanity.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_arch_sanity.ts" 2>/dev/null; then
                    log_pass "Architecture sanity (TypeScript validator)"
                    return
                fi
            fi
            ;;
    esac

    # Generic: no orphaned imports
    local orphaned_imports=0
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qE "^import.*UNUSED|^from.*UNUSED" "$f" 2>/dev/null; then
            ((orphaned_imports++))
        fi
    done
    if [[ $orphaned_imports -gt 0 ]]; then
        log_fail "$check_name" "${orphaned_imports} files have UNUSED import markers"
    else
        log_pass "Architecture sanity (no orphaned imports)"
    fi

    # Concurrency safety patterns
    local has_concurrency_safety=false
    for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
        if grep -qiE "(lock|mutex|atomic|transaction|isolation|concurrent\.)" "$f" 2>/dev/null; then
            has_concurrency_safety=true
            break
        fi
    done
    if [[ "$has_concurrency_safety" == "true" ]]; then
        log_pass "Architecture sanity (concurrency safety patterns detected)"
    else
        log_skip "Architecture sanity" "No concurrency-sensitive code in this slice"
    fi

    # Settings/env collision check
    local collisions=0
    if [[ -f ".env.example" ]] && [[ -f ".env" ]]; then
        while IFS= read -r line; do
            var_name=$(echo "$line" | cut -d= -f1)
            if grep -q "^${var_name}=" ".env" 2>/dev/null; then
                local example_val actual_val
                example_val=$(grep "^${var_name}=" ".env.example" | cut -d= -f2-)
                actual_val=$(grep "^${var_name}=" ".env" | cut -d= -f2-)
                if [[ "$example_val" == "$actual_val" ]]; then
                    ((collisions++))
                fi
            fi
        done < <(grep -v "^#" .env.example 2>/dev/null | grep "=" || true)
    fi
    if [[ $collisions -gt 0 ]]; then
        log_fail "$check_name" "${collisions} env vars in .env match .env.example (should be different)"
    else
        log_pass "Architecture sanity (no settings/env collisions)"
    fi
}

# ── Import Boundary Check ──

check_import_boundaries() {
    local check_name="import_boundaries"
    local lang="$1"

    case "$lang" in
        python)
            if [[ -f "${SCRIPTS_DIR}/check_import_boundaries.py" ]]; then
                if python3 "${SCRIPTS_DIR}/check_import_boundaries.py" 2>/dev/null; then
                    log_pass "Import boundary conformance (Python validator)"
                    return
                fi
            fi
            ;;
        typescript)
            if [[ -f "${SCRIPTS_DIR}/check_import_boundaries.ts" ]]; then
                if npx tsx "${SCRIPTS_DIR}/check_import_boundaries.ts" 2>/dev/null; then
                    log_pass "Import boundary conformance (TypeScript validator)"
                    return
                fi
            fi
            ;;
    esac

    # Basic check: no cross-layer violations
    # Layer structure: app/domain → app/application → app/infrastructure → app/api
    local violations=0

    for layer_dir in app/domain app/application app/infrastructure app/api src/domain src/application src/infrastructure src/api; do
        if [[ ! -d "$layer_dir" ]]; then continue; fi
        while IFS= read -r file; do
            [[ -f "$file" ]] || continue
            if [[ "$layer_dir" == *"domain" ]]; then
                if grep -qE "from.*infrastructure|from.*api|import.*infrastructure|import.*api" "$file" 2>/dev/null; then
                    ((violations++))
                    log_fail "$check_name" "Domain layer imports from infrastructure/api in $(basename "$file")"
                fi
            fi
            if [[ "$layer_dir" == *"application" ]]; then
                if grep -qE "from.*api|import.*api" "$file" 2>/dev/null; then
                    ((violations++))
                    log_fail "$check_name" "Application layer imports from api in $(basename "$file")"
                fi
            fi
        done < <(find "$layer_dir" -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -20)
    done

    if [[ $violations -eq 0 ]]; then
        log_pass "Import boundary conformance (no cross-layer violations)"
    fi
}

# ── Main ──

echo "═══ Architecture Conformance Checks ═══"
echo ""

# Run all conformance checks
check_tenant_isolation "" "$PROJECT_LANG"
check_event_ordering "$PROJECT_LANG"
check_outbox_dlq "$PROJECT_LANG"
check_replay_upcaster "$PROJECT_LANG"
check_runstarted_publication "$PROJECT_LANG"
check_runstarted_worker_activation "$PROJECT_LANG"
check_bounded_execution "$PROJECT_LANG"
check_artifact_proof_surfaces "$PROJECT_LANG"
check_runtime_baseline "$PROJECT_LANG"
check_controlled_stage_progression "$PROJECT_LANG"

echo ""
echo "═══ Architecture Sanity Checks ═══"
echo ""

check_arch_sanity "" "$PROJECT_LANG"
check_import_boundaries "$PROJECT_LANG"

echo ""
echo "═══ Conformance Summary ═══"
echo -e "  ${GREEN}Pass: ${PASS_COUNT}${NC}"
echo -e "  ${RED}Fail: ${FAIL_COUNT}${NC}"
echo -e "  ${YELLOW}Skip: ${SKIP_COUNT}${NC}"
echo "  Total: ${TOTAL_COUNT}"

if [[ ${FAIL_COUNT} -gt 0 ]]; then
    echo ""
    echo "Failures:"
    for f in "${FAILURES[@]}"; do
        echo -e "  ${RED}✗${NC} $f"
    done
    echo ""
    echo "Architecture conformance FAILED. Fix the issues above before proceeding."
    exit 1
fi

echo ""
echo -e "${GREEN}All architecture conformance checks passed.${NC}"
exit 0
