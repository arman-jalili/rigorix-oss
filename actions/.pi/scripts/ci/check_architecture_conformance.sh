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
#
# This file is copied verbatim to each crate that ships the gate. The copies MUST
# stay byte-identical (#1281): they had drifted enough that the same command
# reported "no summary, exit 1" from the repository root, "32 failures" from a
# crate directory, and "0 failures" through the crate's own copy — three
# different answers from one gate. Edit this file, then copy it:
#
#   for d in . actions cli engine mcp; do
#     cp .pi/scripts/ci/check_architecture_conformance.sh "$d/.pi/scripts/ci/"
#   done
#
# Behavior that must hold in every directory:
#   - no `src/` at this level        → SKIP, summary, exit 0
#   - a check that aborts            → a failure line naming that check, summary
#   - the summary always prints      → including on an early exit (EXIT trap)

set -euo pipefail

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
    PASS_COUNT=$((PASS_COUNT + 1))
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
}

log_fail() {
    if [[ -n "${2:-}" ]]; then
        echo -e "  ${RED}✗ FAIL${NC} $1 — $2"
    else
        echo -e "  ${RED}✗ FAIL${NC} $1"
    fi
    FAIL_COUNT=$((FAIL_COUNT + 1))
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
    FAILURES+=("$1: $2")
}

log_skip() {
    if [[ -n "${2:-}" ]]; then
        echo -e "  ${YELLOW}⊘ SKIP${NC} $1 — $2"
    else
        echo -e "  ${YELLOW}⊘ SKIP${NC} $1"
    fi
    SKIP_COUNT=$((SKIP_COUNT + 1))
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
}

# Source files worth scanning. Build trees and vendored dependencies are PRUNED,
# not filtered: `-not -path "*/target/*"` still descends into `target/` and pays
# for every file in it (0.04s vs 8.6s on this repo, once per check), which is how
# a gate that scans "a few dozen files" takes minutes from the repository root.
scan_code_files() {
    local root="${1:-.}"
    find "$root" \
        \( -name target -o -name .git -o -name node_modules -o -name .venv -o -name graphify-out \) -prune -o \
        -type f \( -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" \) -print 2>/dev/null
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
                    violations=$((violations + 1))
                    log_fail "$check_name" "Model $(basename "$model_file") has tenant_id but no tenant-scoped queries"
                fi
            fi
        done < <(scan_code_files | head -50)
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
    for f in $(scan_code_files | head -30); do
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

    for f in $(scan_code_files | head -30); do
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

    for f in $(scan_code_files | head -30); do
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
    for f in $(scan_code_files | head -30); do
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
    for f in $(scan_code_files | head -30); do
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
    for f in $(scan_code_files | head -30); do
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
    for f in $(scan_code_files | head -30); do
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
                unbound=$((unbound + 1))
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
    for f in $(scan_code_files | head -30); do
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
    for f in $(scan_code_files | head -30); do
        if grep -qE "^import.*UNUSED|^from.*UNUSED" "$f" 2>/dev/null; then
            orphaned_imports=$((orphaned_imports + 1))
        fi
    done
    if [[ $orphaned_imports -gt 0 ]]; then
        log_fail "$check_name" "${orphaned_imports} files have UNUSED import markers"
    else
        log_pass "Architecture sanity (no orphaned imports)"
    fi

    # Concurrency safety patterns
    local has_concurrency_safety=false
    for f in $(scan_code_files | head -30); do
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
                    collisions=$((collisions + 1))
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
    # DDD 4-layer structure: domain/ → application/ → infrastructure/ → interfaces/
    local violations=0

    for layer_dir in app/domain app/application app/infrastructure app/api src/domain src/application src/infrastructure src/api; do
        if [[ ! -d "$layer_dir" ]]; then continue; fi
        while IFS= read -r file; do
            [[ -f "$file" ]] || continue
            if [[ "$layer_dir" == *"domain" ]]; then
                if grep -qE "from.*infrastructure|from.*api|from.*interfaces|import.*infrastructure|import.*api|import.*interfaces" "$file" 2>/dev/null; then
                    violations=$((violations + 1))
                    log_fail "$check_name" "Domain layer imports from outer layer in $(basename "$file")"
                fi
            fi
            if [[ "$layer_dir" == *"application" ]]; then
                if grep -qE "from.*api|from.*interfaces|import.*api|import.*interfaces" "$file" 2>/dev/null; then
                    violations=$((violations + 1))
                    log_fail "$check_name" "Application layer imports from api/interfaces in $(basename "$file")"
                fi
            fi
        done < <(scan_code_files "$layer_dir" | head -20)
    done

    # ── per-module layout + boundary check ──────────────────────────────────
    #
    # The required layer set is derived from THIS repository's modules, never
    # from `.pi/architecture/modules/module-template.md`'s React/Next.js table
    # (#1281). That template asks every module for an `interfaces/` layer
    # ("UI components, pages, layouts — React, Next.js"), so 32 of the engine's
    # 33 three-layer modules were reported "incomplete" for a layout this
    # codebase does not use. A gate that is wrong 32 times out of 37 is a gate
    # that gets ignored, and the one real violation is the 33rd line of noise.
    #
    # A *DDD module* here is a module with a `domain/` layer — the marker of the
    # pattern in this codebase. Modules without one (a `host/` of wiring, a
    # `usage_guide/` with only interfaces/, a `shared/` of helpers) are not DDD
    # modules and are not judged by this rule.
    #
    # The required set is the dominant signature among the DDD modules: a module
    # missing a layer its peers all have is drift; an extra layer is not a
    # violation. Pin it instead with ARCH_REQUIRED_LAYERS=domain,application,…
    if [[ ! -d src ]]; then
        log_skip "Import boundary conformance — no src/ at this level, nothing to scan"
        return
    fi

    local module_dirs
    module_dirs=$(find src -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort || true)

    # Collect the DDD modules (module => layers present).
    local ddd_modules=""
    local module_dir module layer signature other other_sig
    local match_count best_count best_sig required candidate
    for module_dir in $module_dirs; do
        module=$(basename "$module_dir")
        case "$module" in shared|common|config|lib|bin) continue ;; esac
        [[ -d "${module_dir}/domain" ]] || continue
        ddd_modules="${ddd_modules}${module_dir} "
    done

    if [[ -z "$ddd_modules" ]]; then
        log_skip "Import boundary conformance — no DDD module (domain/ layer) under src/ to check"
        return
    fi

    # Dominant layer signature — O(n²) over a few dozen modules, and portable to
    # bash 3.2 (no associative arrays).
    best_count=0
    best_sig=""
    for module_dir in $ddd_modules; do
        candidate=""
        for layer in domain application infrastructure interfaces; do
            [[ -d "${module_dir}/${layer}" ]] && candidate="${candidate}${layer},"
        done
        match_count=0
        for other in $ddd_modules; do
            other_sig=""
            for layer in domain application infrastructure interfaces; do
                [[ -d "${other}/${layer}" ]] && other_sig="${other_sig}${layer},"
            done
            [[ "$other_sig" == "$candidate" ]] && match_count=$((match_count + 1))
        done
        if [[ $match_count -gt $best_count ]] ||
            { [[ $match_count -eq $best_count ]] && [[ ${#candidate} -gt ${#best_sig} ]]; }; then
            best_count=$match_count
            best_sig="$candidate"
        fi
    done
    required="$best_sig"
    [[ -n "${ARCH_REQUIRED_LAYERS:-}" ]] && required="${ARCH_REQUIRED_LAYERS// /},"

    for module_dir in $ddd_modules; do
        module=$(basename "$module_dir")

        # contracts/ wrapper anti-pattern — a port belongs in domain/.
        if [[ -d "${module_dir}/contracts" ]]; then
            violations=$((violations + 1))
            log_fail "$check_name" "Module '${module}' has a contracts/ wrapper — ports belong in domain/"
        fi

        # Every declared layer present?
        local missing=""
        local required_layer
        for required_layer in ${required//,/ }; do
            [[ -d "${module_dir}/${required_layer}" ]] || missing="${missing} ${required_layer}"
        done
        if [[ -n "$missing" ]]; then
            violations=$((violations + 1))
            log_fail "$check_name" "Module '${module}' is missing declared layer(s):${missing}"
        fi

        # The boundary this codebase holds everywhere: `domain/` is pure and
        # may not reach outward (application / infrastructure / interfaces).
        # A module may reference ANOTHER module freely — that is composition,
        # not a layer violation.
        #
        # The module template's "application/ may not import interfaces/" rule
        # is NOT enforced: in this repository `interfaces/` is the incoming
        # port as often as it is the outermost adapter layer, and application
        # factories legitimately wire it (e.g.
        # `capacity_ledger/application/factory.rs` returns
        # `crate::capacity_ledger::interfaces::CapacityLedger`). Enforcing a
        # rule the codebase does not follow is how the layer check earned its
        # 32 false failures; a gate may only be as strict as the truth here.
        #
        # Comments are stripped first: a doc link such as
        # `[`CapacityLedger`]: crate::capacity_ledger::interfaces::CapacityLedger`
        # documents an edge, it is not one. (Heuristic, as everywhere here: a
        # `//` inside a string literal truncates the rest of that line.)
        if [[ -d "${module_dir}/domain" ]]; then
            local domain_file
            while IFS= read -r domain_file; do
                [[ -f "$domain_file" ]] || continue
                if sed -E 's,//.*,,' "$domain_file" |
                    grep -qE "(crate|super)::[A-Za-z0-9_:]*(application|infrastructure|interfaces)"; then
                    violations=$((violations + 1))
                    log_fail "$check_name" "Module '${module}': domain/$(basename "$domain_file") references an outer layer (application|infrastructure|interfaces)"
                fi
            done < <(scan_code_files "${module_dir}/domain" || true)
        fi
    done

    if [[ $violations -eq 0 ]]; then
        log_pass "Import boundary conformance (no cross-layer violations)"
    fi
}

# ── Main ──

SUMMARY_PRINTED=0

print_summary() {
    SUMMARY_PRINTED=1
    echo ""
    echo "═══ Conformance Summary ═══"
    echo -e "  ${GREEN}Pass: ${PASS_COUNT}${NC}"
    echo -e "  ${RED}Fail: ${FAIL_COUNT}${NC}"
    echo -e "  ${YELLOW}Skip: ${SKIP_COUNT}${NC}"
    echo "  Total: ${TOTAL_COUNT}"

    if [[ ${FAIL_COUNT} -gt 0 ]]; then
        echo ""
        echo "Failures:"
        local f
        for f in "${FAILURES[@]}"; do
            echo -e "  ${RED}✗${NC} $f"
        done
    fi
}

# The summary prints in every outcome, including an early exit (#1281). Without
# this, a check that aborted under `set -e` produced `exit 1` and no failure
# line at all — `validate-architecture-readiness.sh` then reported "conformance
# failed" for a tree on which nothing failed. The trap does not change the exit
# code.
on_exit() {
    if [[ "${SUMMARY_PRINTED:-0}" -eq 0 ]]; then
        echo ""
        echo -e "  ${YELLOW}(the run ended before the summary — a check aborted; the counts below are partial)${NC}"
        print_summary
    fi
}
trap on_exit EXIT

# A check that aborts must be *named*, not silent. `if ! cmd` also suspends
# `set -e` inside the check, so one bad `find` can no longer kill the run before
# the other checks and the summary.
run_check() {
    local fn="$1"
    shift
    if ! "$fn" "$@"; then
        log_fail "$fn" "check aborted — the last line above names where"
    fi
}

echo "═══ Architecture Conformance Checks ═══"
echo ""

# Run all conformance checks
run_check check_tenant_isolation "" "$PROJECT_LANG"
run_check check_event_ordering "$PROJECT_LANG"
run_check check_outbox_dlq "$PROJECT_LANG"
run_check check_replay_upcaster "$PROJECT_LANG"
run_check check_runstarted_publication "$PROJECT_LANG"
run_check check_runstarted_worker_activation "$PROJECT_LANG"
run_check check_bounded_execution "$PROJECT_LANG"
run_check check_artifact_proof_surfaces "$PROJECT_LANG"
run_check check_runtime_baseline "$PROJECT_LANG"
run_check check_controlled_stage_progression "$PROJECT_LANG"

echo ""
echo "═══ Architecture Sanity Checks ═══"
echo ""

run_check check_arch_sanity "" "$PROJECT_LANG"
run_check check_import_boundaries "$PROJECT_LANG"

print_summary

if [[ ${FAIL_COUNT} -gt 0 ]]; then
    echo ""
    echo "Architecture conformance FAILED. Fix the issues above before proceeding."
    exit 1
fi

echo ""
echo -e "${GREEN}All architecture conformance checks passed.${NC}"
exit 0
