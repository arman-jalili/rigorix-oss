#!/usr/bin/env bash
# Guardian Local Developer Workflow — Preflight Engine
#
# Runs the applicable local preflight subset of CI checks before you commit.
# Detects changed files, runs only affected checks, generates reports.
#
# Usage:
#   ./run_preflight.sh                    # Run all checks
#   ./run_preflight.sh --staged           # Check staged files only
#   ./run_preflight.sh --stage=security   # Run only security stage
#   ./run_preflight.sh --json             # JSON output for CI/agent integration
#   ./run_preflight.sh --verbose          # Verbose output
#   ./run_preflight.sh --staged --stage=lint --json  # Combine options

set -euo pipefail

PI_DIR=".pi"
CI_DIR="${PI_DIR}/scripts/ci"
GIT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || echo ".")"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Options
STAGED=false
STAGE=""
JSON=false
VERBOSE=false
REPORT_FILE="preflight_report.json"

# Counters
TOTAL=0
PASSED=0
FAILED=0
SKIPPED=0
STAGES_RUN=()
RESULTS=()
START_TIME=$(date +%s)

# ── Argument Parsing ──

while [[ $# -gt 0 ]]; do
    case $1 in
        --staged) STAGED=true; shift ;;
        --stage=*) STAGE="${1#*=}"; shift ;;
        --json) JSON=true; shift ;;
        --verbose) VERBOSE=true; shift ;;
        *) shift ;;
    esac
done

# ── Helper Functions ──

log_info() {
    if [[ "$JSON" == "false" ]]; then
        echo -e "${BLUE}[INFO]${NC} $1"
    fi
}

log_pass() {
    if [[ "$JSON" == "false" ]]; then
        echo -e "  ${GREEN}[PASS]${NC} $1 (${2}s)"
    fi
    ((PASSED++))
    ((TOTAL++))
    RESULTS+=("{\"name\": \"$1\", \"status\": \"pass\", \"message\": \"\", \"duration\": $2}")
}

log_fail() {
    if [[ "$JSON" == "false" ]]; then
        echo -e "  ${RED}[FAIL]${NC} $1 — $2 (${3}s)"
    fi
    ((FAILED++))
    ((TOTAL++))
    RESULTS+=("{\"name\": \"$1\", \"status\": \"fail\", \"message\": \"$2\", \"duration\": $3}")
}

log_skip() {
    if [[ "$JSON" == "false" ]]; then
        echo -e "  ${YELLOW}[SKIP]${NC} $1 — $2"
    fi
    ((SKIPPED++))
    ((TOTAL++))
    RESULTS+=("{\"name\": \"$1\", \"status\": \"skip\", \"message\": \"$2\", \"duration\": 0}")
}

# ── Change Detection ──

detect_changed_files() {
    if [[ "$STAGED" == "true" ]]; then
        git diff --cached --name-only 2>/dev/null || true
    else
        git diff --name-only HEAD~1 HEAD 2>/dev/null || git ls-files 2>/dev/null || true
    fi
}

should_run_stage() {
    local stage="$1"
    if [[ -n "$STAGE" && "$STAGE" != "$stage" ]]; then
        return 1
    fi

    if [[ "$STAGED" == "true" ]]; then
        # Check if any staged files match the stage
        case "$stage" in
            docs_policy)
                git diff --cached --name-only 2>/dev/null | grep -qE "\.md$" && return 0
                ;;
            architecture_conformance|static_analysis)
                git diff --cached --name-only 2>/dev/null | grep -qE "\.(py|ts|rs|go)$" && return 0
                ;;
            lint)
                git diff --cached --name-only 2>/dev/null | grep -qE "\.(py|ts|rs|go)$" && return 0
                ;;
            unit|integration)
                git diff --cached --name-only 2>/dev/null | grep -qE "(test|spec)\.(py|ts|rs|go)$" && return 0
                ;;
            security)
                git diff --cached --name-only 2>/dev/null | grep -qE "\.(py|ts|rs|go|env|yml|yaml)$" && return 0
                ;;
            migration_verify)
                git diff --cached --name-only 2>/dev/null | grep -qE "(migration|alembic|schema)" && return 0
                ;;
            release_readiness)
                git diff --cached --name-only 2>/dev/null | grep -qE "(runbook|dr-plan|RUNBOOK|DR)" && return 0
                ;;
            *)
                return 0
                ;;
        esac
        return 1
    fi
    return 0
}

# ── Service Detection ──

check_postgres() {
    if command -v psql &>/dev/null && psql -h localhost -U postgres -c "SELECT 1" &>/dev/null 2>&1; then
        return 0
    fi
    return 1
}

check_redis() {
    if command -v redis-cli &>/dev/null && redis-cli ping 2>/dev/null | grep -q "PONG"; then
        return 0
    fi
    return 1
}

check_docker() {
    if command -v docker &>/dev/null && docker info &>/dev/null 2>&1; then
        return 0
    fi
    return 1
}

# ── Stage Runners ──

run_stage_docs_policy() {
    if ! should_run_stage "docs_policy"; then return 0; fi

    local stage_start=$(date +%s)
    log_info "Running stage: docs_policy"

    # MR traceability check
    local t=$(date +%s)
    if [[ -f "${CI_DIR}/check_mr_traceability.sh" ]]; then
        if bash "${CI_DIR}/check_mr_traceability.sh" >/dev/null 2>&1; then
            log_pass "check_mr_traceability.sh" "$(( $(date +%s) - t ))"
        else
            log_fail "check_mr_traceability.sh" "traceability check failed" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "check_mr_traceability.sh" "script not found"
    fi

    # Docs sync guard
    t=$(date +%s)
    if [[ -f "${CI_DIR}/check_docs_sync.sh" ]]; then
        if bash "${CI_DIR}/check_docs_sync.sh" >/dev/null 2>&1; then
            log_pass "check_docs_sync.sh" "$(( $(date +%s) - t ))"
        else
            log_fail "check_docs_sync.sh" "docs sync check failed" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "check_docs_sync.sh" "script not found"
    fi
}

run_stage_architecture_conformance() {
    if ! should_run_stage "architecture_conformance"; then return 0; fi

    log_info "Running stage: architecture_conformance"
    local t=$(date +%s)

    if [[ -f "${CI_DIR}/check_architecture_conformance.sh" ]]; then
        if bash "${CI_DIR}/check_architecture_conformance.sh" >/dev/null 2>&1; then
            log_pass "check_architecture_conformance.sh" "$(( $(date +%s) - t ))"
        else
            log_fail "check_architecture_conformance.sh" "conformance checks failed" "$(( $(date +%s) - t ))"
            # Show verbose output on failure
            if [[ "$VERBOSE" == "true" ]]; then
                bash "${CI_DIR}/check_architecture_conformance.sh" 2>&1 | tail -30
            fi
        fi
    else
        log_skip "check_architecture_conformance.sh" "script not found"
    fi
}

run_stage_lint() {
    if ! should_run_stage "lint"; then return 0; fi

    log_info "Running stage: lint"

    # Detect language
    local lang=""
    if [[ -f "pyproject.toml" || -f "requirements.txt" ]]; then lang="python"
    elif [[ -f "package.json" ]]; then lang="typescript"
    elif [[ -f "Cargo.toml" ]]; then lang="rust"
    elif [[ -f "go.mod" ]]; then lang="go"
    fi

    case "$lang" in
        python)
            local t=$(date +%s)
            if command -v ruff &>/dev/null; then
                if ruff check . >/dev/null 2>&1; then
                    log_pass "ruff check" "$(( $(date +%s) - t ))"
                else
                    log_fail "ruff check" "lint errors found" "$(( $(date +%s) - t ))"
                fi
                t=$(date +%s)
                if ruff format --check . >/dev/null 2>&1; then
                    log_pass "ruff format --check" "$(( $(date +%s) - t ))"
                else
                    log_fail "ruff format --check" "format errors found" "$(( $(date +%s) - t ))"
                fi
            else
                log_skip "ruff" "not installed"
            fi
            ;;
        typescript)
            local t=$(date +%s)
            if command -v biome &>/dev/null; then
                if biome check . >/dev/null 2>&1; then
                    log_pass "biome check" "$(( $(date +%s) - t ))"
                else
                    log_fail "biome check" "lint errors found" "$(( $(date +%s) - t ))"
                fi
                t=$(date +%s)
                if biome format --check . >/dev/null 2>&1; then
                    log_pass "biome format --check" "$(( $(date +%s) - t ))"
                else
                    log_fail "biome format --check" "format errors found" "$(( $(date +%s) - t ))"
                fi
            else
                log_skip "biome" "not installed"
            fi
            ;;
        rust)
            local t=$(date +%s)
            if command -v cargo &>/dev/null; then
                if cargo clippy -- -D warnings >/dev/null 2>&1; then
                    log_pass "cargo clippy" "$(( $(date +%s) - t ))"
                else
                    log_fail "cargo clippy" "clippy warnings found" "$(( $(date +%s) - t ))"
                fi
                t=$(date +%s)
                if cargo fmt --check >/dev/null 2>&1; then
                    log_pass "cargo fmt --check" "$(( $(date +%s) - t ))"
                else
                    log_fail "cargo fmt --check" "format errors found" "$(( $(date +%s) - t ))"
                fi
            else
                log_skip "cargo" "not installed"
            fi
            ;;
        go)
            local t=$(date +%s)
            if command -v golangci-lint &>/dev/null; then
                if golangci-lint run >/dev/null 2>&1; then
                    log_pass "golangci-lint" "$(( $(date +%s) - t ))"
                else
                    log_fail "golangci-lint" "lint errors found" "$(( $(date +%s) - t ))"
                fi
            else
                log_skip "golangci-lint" "not installed"
            fi
            t=$(date +%s)
            if command -v gofmt &>/dev/null; then
                if [[ -z "$(gofmt -d . 2>/dev/null)" ]]; then
                    log_pass "gofmt --check" "$(( $(date +%s) - t ))"
                else
                    log_fail "gofmt --check" "format errors found" "$(( $(date +%s) - t ))"
                fi
            else
                log_skip "gofmt" "not installed"
            fi
            ;;
        *)
            log_skip "lint" "no project configuration found"
            ;;
    esac
}

run_stage_static_analysis() {
    if ! should_run_stage "static_analysis"; then return 0; fi

    log_info "Running stage: static_analysis"

    # Type checking
    local t=$(date +%s)
    if [[ -f "pyproject.toml" ]] && command -v mypy &>/dev/null; then
        if mypy . --ignore-missing-imports >/dev/null 2>&1; then
            log_pass "mypy" "$(( $(date +%s) - t ))"
        else
            log_fail "mypy" "type errors found" "$(( $(date +%s) - t ))"
        fi
    elif [[ -f "tsconfig.json" ]] && command -v tsc &>/dev/null; then
        if tsc --noEmit >/dev/null 2>&1; then
            log_pass "tsc --noEmit" "$(( $(date +%s) - t ))"
        else
            log_fail "tsc --noEmit" "type errors found" "$(( $(date +%s) - t ))"
        fi
    elif [[ -f "Cargo.toml" ]] && command -v cargo &>/dev/null; then
        if cargo check >/dev/null 2>&1; then
            log_pass "cargo check" "$(( $(date +%s) - t ))"
        else
            log_fail "cargo check" "compilation errors" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "type_check" "no type checker available"
    fi

    # Architecture sanity
    t=$(date +%s)
    if [[ -f "${CI_DIR}/check_arch_sanity.sh" ]]; then
        if bash "${CI_DIR}/check_arch_sanity.sh" >/dev/null 2>&1; then
            log_pass "check_arch_sanity.sh" "$(( $(date +%s) - t ))"
        else
            log_fail "check_arch_sanity.sh" "sanity check failed" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "check_arch_sanity.sh" "script not found"
    fi

    # Import boundaries
    t=$(date +%s)
    if [[ -f "${CI_DIR}/check_import_boundaries.sh" ]]; then
        if bash "${CI_DIR}/check_import_boundaries.sh" >/dev/null 2>&1; then
            log_pass "check_import_boundaries.sh" "$(( $(date +%s) - t ))"
        else
            log_fail "check_import_boundaries.sh" "boundary violations found" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "check_import_boundaries.sh" "script not found"
    fi
}

run_stage_unit() {
    if ! should_run_stage "unit"; then return 0; fi

    log_info "Running stage: unit"

    local t=$(date +%s)
    if command -v pytest &>/dev/null && [[ -d "tests/unit" ]]; then
        if pytest tests/unit -q --tb=short >/dev/null 2>&1; then
            log_pass "pytest tests/unit" "$(( $(date +%s) - t ))"
        else
            log_fail "pytest tests/unit" "unit test failures" "$(( $(date +%s) - t ))"
        fi
    elif command -v bun &>/dev/null && [[ -d "tests" ]]; then
        if bun test -t "unit" >/dev/null 2>&1; then
            log_pass "bun test (unit)" "$(( $(date +%s) - t ))"
        else
            log_fail "bun test (unit)" "unit test failures" "$(( $(date +%s) - t ))"
        fi
    elif command -v cargo &>/dev/null; then
        if cargo test --lib >/dev/null 2>&1; then
            log_pass "cargo test --lib" "$(( $(date +%s) - t ))"
        else
            log_fail "cargo test --lib" "unit test failures" "$(( $(date +%s) - t ))"
        fi
    elif command -v go &>/dev/null; then
        if go test ./... -short >/dev/null 2>&1; then
            log_pass "go test" "$(( $(date +%s) - t ))"
        else
            log_fail "go test" "unit test failures" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "unit" "no test runner found"
    fi
}

run_stage_integration() {
    if ! should_run_stage "integration"; then return 0; fi

    log_info "Running stage: integration"

    if ! check_postgres || ! check_redis; then
        log_skip "integration" "PostgreSQL or Redis not available"
        if [[ "$VERBOSE" == "true" ]]; then
            log_info "Start services: docker run -d postgres:16 && docker run -d redis:7"
        fi
        return 0
    fi

    local t=$(date +%s)
    if command -v pytest &>/dev/null && [[ -d "tests/integration" ]]; then
        if pytest tests/integration -q --tb=short >/dev/null 2>&1; then
            log_pass "pytest tests/integration" "$(( $(date +%s) - t ))"
        else
            log_fail "pytest tests/integration" "integration test failures" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "integration" "no integration tests found"
    fi
}

run_stage_security() {
    if ! should_run_stage "security"; then return 0; fi

    log_info "Running stage: security"

    # Secret scan
    local t=$(date +%s)
    if [[ -f "${CI_DIR}/secret_scan.sh" ]]; then
        if bash "${CI_DIR}/secret_scan.sh" >/dev/null 2>&1; then
            log_pass "secret_scan.sh" "$(( $(date +%s) - t ))"
        else
            log_fail "secret_scan.sh" "secrets detected" "$(( $(date +%s) - t ))"
        fi
    else
        # Fallback: basic grep
        if grep -rE "(sk-[A-Za-z0-9]{32,}|ghp_[A-Za-z0-9]{36}|AKIA[0-9A-Z]{16})" . --include="*.py" --include="*.ts" --include="*.env" 2>/dev/null | grep -v ".git" | grep -v "node_modules" | head -1 | grep -q .; then
            log_fail "secret_scan" "potential secrets found" "$(( $(date +%s) - t ))"
        else
            log_pass "secret_scan" "$(( $(date +%s) - t ))"
        fi
    fi

    # Dependency audit
    t=$(date +%s)
    if command -v pip-audit &>/dev/null; then
        if pip-audit --skip-editable >/dev/null 2>&1; then
            log_pass "pip-audit" "$(( $(date +%s) - t ))"
        else
            log_fail "pip-audit" "vulnerable dependencies" "$(( $(date +%s) - t ))"
        fi
    elif command -v npm &>/dev/null && [[ -f "package.json" ]]; then
        if npm audit --audit-level=high >/dev/null 2>&1; then
            log_pass "npm audit" "$(( $(date +%s) - t ))"
        else
            log_fail "npm audit" "high/critical vulnerabilities" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "dependency_audit" "no package manager audit tool found"
    fi
}

run_stage_migration_verify() {
    if ! should_run_stage "migration_verify"; then return 0; fi

    log_info "Running stage: migration_verify"

    if ! check_postgres; then
        log_skip "migration_verify" "PostgreSQL not available"
        return 0
    fi

    local t=$(date +%s)
    if command -v alembic &>/dev/null; then
        if alembic upgrade head >/dev/null 2>&1; then
            log_pass "alembic upgrade head" "$(( $(date +%s) - t ))"
        else
            log_fail "alembic upgrade head" "migration failed" "$(( $(date +%s) - t ))"
        fi
    else
        log_skip "migration_verify" "alembic not installed"
    fi
}

run_stage_package_build() {
    if ! should_run_stage "package_build"; then return 0; fi

    log_info "Running stage: package_build"

    if ! check_docker; then
        log_skip "package_build" "Docker not available"
        return 0
    fi

    if [[ ! -f "Dockerfile" ]]; then
        log_skip "package_build" "no Dockerfile found"
        return 0
    fi

    local t=$(date +%s)
    if docker build -t guardian:local >/dev/null 2>&1; then
        log_pass "docker build" "$(( $(date +%s) - t ))"
    else
        log_fail "docker build" "build failed" "$(( $(date +%s) - t ))"
    fi
}

run_stage_release_readiness() {
    if ! should_run_stage "release_readiness"; then return 0; fi

    log_info "Running stage: release_readiness"

    local t=$(date +%s)
    if [[ -f "${PI_DIR}/scripts/validate-architecture-readiness.sh" ]]; then
        if bash "${PI_DIR}/scripts/validate-architecture-readiness.sh" >/dev/null 2>&1; then
            log_pass "validate-architecture-readiness.sh" "$(( $(date +%s) - t ))"
        else
            log_fail "validate-architecture-readiness.sh" "readiness checks failed" "$(( $(date +%s) - t ))"
            if [[ "$VERBOSE" == "true" ]]; then
                bash "${PI_DIR}/scripts/validate-architecture-readiness.sh" 2>&1 | tail -30
            fi
        fi
    else
        log_skip "validate-architecture-readiness.sh" "script not found"
    fi
}

# ── Main ──

CHANGED_FILES=$(detect_changed_files)

if [[ -n "$CHANGED_FILES" && "$JSON" == "false" ]]; then
    log_info "Changed files:"
    echo "$CHANGED_FILES" | while IFS= read -r f; do
        [[ -n "$f" ]] && echo "  - $f"
    done
fi

# Run all stages
run_stage_docs_policy
run_stage_architecture_conformance
run_stage_lint
run_stage_static_analysis
run_stage_unit
run_stage_integration
run_stage_security
run_stage_migration_verify
run_stage_package_build
run_stage_release_readiness

# Calculate duration
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Determine overall status
if [[ $FAILED -gt 0 ]]; then
    STATUS="fail"
else
    STATUS="pass"
fi

# Output
if [[ "$JSON" == "true" ]]; then
    # JSON output
    if [[ ${#STAGES_RUN[@]} -gt 0 ]]; then
        STAGES_JSON=$(printf '%s\n' "${STAGES_RUN[@]}" | jq -R . | jq -s .)
    else
        STAGES_JSON="[]"
    fi
    if [[ ${#RESULTS[@]} -gt 0 ]]; then
        RESULTS_JSON=$(printf '%s\n' "${RESULTS[@]}" | jq -s .)
    else
        RESULTS_JSON="[]"
    fi

    cat << EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "mode": "$([ "$STAGED" == "true" ] && echo "staged" || echo "all")",
  "stages_run": ${STAGES_JSON:-[]},
  "summary": {
    "total": ${TOTAL},
    "passed": ${PASSED},
    "failed": ${FAILED},
    "skipped": ${SKIPPED}
  },
  "duration_seconds": ${DURATION},
  "results": ${RESULTS_JSON:-[]},
  "status": "${STATUS}"
}
EOF
else
    # Human-readable output
    echo ""
    echo "================================"
    echo "PREFLIGHT SUMMARY"
    echo "================================"
    echo "Total checks:  ${TOTAL}"
    echo -e "Passed:        ${GREEN}${PASSED}${NC}"
    echo -e "Failed:        ${RED}${FAILED}${NC}"
    echo -e "Skipped:       ${YELLOW}${SKIPPED}${NC}"
    echo "Duration:      ${DURATION}s"
    echo "Report:        ${REPORT_FILE}"
    echo "================================"

    if [[ $FAILED -eq 0 ]]; then
        echo -e "${GREEN}✅ Preflight passed${NC}"
    else
        echo -e "${RED}❌ Preflight failed (${FAILED} check(s))${NC}"
        echo "Fix the issues above before committing."
    fi
fi

# Save report
if [[ "$JSON" == "true" ]]; then
    if [[ ${#STAGES_RUN[@]} -gt 0 ]]; then
        STAGES_JSON=$(printf '%s\n' "${STAGES_RUN[@]}" | jq -R . | jq -s .)
    else
        STAGES_JSON="[]"
    fi
    if [[ ${#RESULTS[@]} -gt 0 ]]; then
        RESULTS_JSON=$(printf '%s\n' "${RESULTS[@]}" | jq -s .)
    else
        RESULTS_JSON="[]"
    fi
    cat << EOF > "$REPORT_FILE"
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "mode": "$([ "$STAGED" == "true" ] && echo "staged" || echo "all")",
  "stages_run": ${STAGES_JSON:-[]},
  "summary": {
    "total": ${TOTAL},
    "passed": ${PASSED},
    "failed": ${FAILED},
    "skipped": ${SKIPPED}
  },
  "duration_seconds": ${DURATION},
  "results": ${RESULTS_JSON:-[]},
  "status": "${STATUS}"
}
EOF
fi

exit $([ $FAILED -gt 0 ] && echo 1 || echo 0)
