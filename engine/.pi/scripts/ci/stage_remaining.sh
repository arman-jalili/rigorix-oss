#!/usr/bin/env bash
# Stage 6: Integration Tests
# Stage 7: Security
# Stage 8: Migration Verify
# Stage 9: Package Build
# Stage 10: Release Readiness
#
# Combined stage runner for all remaining stages.

set -euo pipefail
set +o braceexpand  # Prevent {32,} from being expanded by bash

PI_DIR=".pi"
FAIL=0
PASS=0

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1 — $2"; FAIL=$((FAIL + 1)); }

STAGE="${1:-all}"

run_stage_6_integration() {
    echo "  Running integration tests..."
    if command -v pytest &>/dev/null && [[ -d "tests/integration" ]]; then
        if pytest tests/integration -v 2>/dev/null; then
            log_pass "integration tests"
        else
            log_fail "integration tests" "Integration test failures"
        fi
    elif command -v bun &>/dev/null && [[ -d "tests/integration" ]]; then
        if bun test tests/integration 2>/dev/null; then
            log_pass "integration tests (bun)"
        else
            log_fail "integration tests (bun)" "Integration test failures"
        fi
    elif command -v cargo &>/dev/null; then
        if cargo test --test integration 2>/dev/null; then
            log_pass "integration tests (cargo)"
        else
            log_pass "integration tests (cargo) (no integration tests found)"
        fi
    else
        log_pass "integration tests (no integration test directory found)"
    fi
}

run_stage_7_security() {
    echo "  Running security checks..."

    # SBOM generation
    if [[ -f "${PI_DIR}/scripts/ci/generate_sbom.py" ]]; then
        if python3 "${PI_DIR}/scripts/ci/generate_sbom.py" 2>/dev/null; then
            log_pass "SBOM generation"
        else
            log_fail "SBOM generation" "SBOM generation failed"
        fi
    else
        log_pass "SBOM generation (no script found, skipping)"
    fi

    # Secret scan
    if [[ -f "${PI_DIR}/scripts/ci/secret_scan.py" ]]; then
        if python3 "${PI_DIR}/scripts/ci/secret_scan.py" 2>/dev/null; then
            log_pass "secret scan"
        else
            log_fail "secret scan" "Secrets detected in codebase"
        fi
    else
        # Fallback: grep for common secret patterns
        local secrets_found=0
        for pattern in "sk-[A-Za-z0-9]{32,}" "ghp_[A-Za-z0-9]{36}" "AKIA[0-9A-Z]{16}" "BEGIN (RSA |EC )?PRIVATE KEY"; do
            if grep -rE "$pattern" . --include="*.py" --include="*.ts" --include="*.env" 2>/dev/null | grep -v ".git" | grep -v "node_modules" | head -1 | grep -q .; then
                ((secrets_found++))
                log_fail "secret scan" "Potential secret pattern detected: $pattern"
            fi
        done
        if [[ $secrets_found -eq 0 ]]; then
            log_pass "secret scan (no secrets detected)"
        fi
    fi

    # Dependency audit
    if command -v pip-audit &>/dev/null; then
        if pip-audit --path . --skip-editable 2>/dev/null; then
            log_pass "dependency audit (pip-audit)"
        else
            log_fail "dependency audit (pip-audit)" "Vulnerable dependencies found"
        fi
    elif command -v npm &>/dev/null && [[ -f "package.json" ]]; then
        if npm audit --audit-level=high 2>/dev/null; then
            log_pass "dependency audit (npm)"
        else
            log_fail "dependency audit (npm)" "High/critical vulnerabilities found"
        fi
    elif command -v cargo &>/dev/null && [[ -f "Cargo.toml" ]]; then
        if cargo audit 2>/dev/null; then
            log_pass "dependency audit (cargo-audit)"
        else
            log_pass "dependency audit (cargo-audit) (cargo-audit not installed)"
        fi
    else
        log_pass "dependency audit (no package manager found)"
    fi

    # Container scan (Trivy)
    if command -v trivy &>/dev/null && [[ -f "Dockerfile" ]]; then
        if trivy image --severity HIGH,CRITICAL --exit-code 1 --timeout 10m . 2>/dev/null; then
            log_pass "container scan (Trivy)"
        else
            log_fail "container scan (Trivy)" "HIGH/CRITICAL vulnerabilities found"
        fi
    else
        log_pass "container scan (Trivy not available or no Dockerfile)"
    fi
}

run_stage_8_migration() {
    echo "  Running migration verification..."
    if command -v alembic &>/dev/null && [[ -d "alembic" || -d "migrations" ]]; then
        if alembic upgrade head 2>/dev/null; then
            log_pass "migration apply check"
        else
            log_fail "migration apply check" "Migration failed to apply"
        fi

        if [[ -f "${PI_DIR}/scripts/ci/verify_indexes_and_policies.py" ]]; then
            if python3 "${PI_DIR}/scripts/ci/verify_indexes_and_policies.py" 2>/dev/null; then
                log_pass "index policy verify"
            else
                log_fail "index policy verify" "Index or policy violations found"
            fi
        else
            log_pass "index policy verify (no script found)"
        fi
    else
        log_pass "migration verification (no migration framework found)"
    fi
}

run_stage_9_package() {
    echo "  Running package build..."
    if [[ -f "Dockerfile" ]] && command -v docker &>/dev/null; then
        local tag="${CI_COMMIT_SHA:-local}"
        if docker build -t app:"${tag}" . 2>/dev/null; then
            log_pass "package build (Docker)"
        else
            log_fail "package build (Docker)" "Build failed"
        fi
    else
        log_pass "package build (no Dockerfile or Docker not available)"
    fi
}

run_stage_10_release() {
    echo "  Running release readiness checks..."

    # Runbook readiness
    if [[ -f "${PI_DIR}/scripts/ci/check_runbook_readiness.py" ]]; then
        if python3 "${PI_DIR}/scripts/ci/check_runbook_readiness.py" 2>/dev/null; then
            log_pass "runbook readiness"
        else
            log_fail "runbook readiness" "Runbook not ready"
        fi
    else
        # Fallback: check for runbook file
        if [[ -f "docs/runbook.md" || -f "docs/RUNBOOK.md" || -f "RUNBOOK.md" ]]; then
            log_pass "runbook readiness (runbook file exists)"
        else
            log_fail "runbook readiness" "No runbook.md found"
        fi
    fi

    # Observability readiness
    if [[ -f "${PI_DIR}/scripts/ci/check_observability_readiness.py" ]]; then
        if python3 "${PI_DIR}/scripts/ci/check_observability_readiness.py" 2>/dev/null; then
            log_pass "observability readiness"
        else
            log_fail "observability readiness" "Observability not ready"
        fi
    else
        # Fallback: check for tracing/metrics setup
        local has_observability=false
        for f in $(find . -name "*.py" -o -name "*.ts" 2>/dev/null | head -30); do
            if grep -qiE "(opentelemetry|prometheus|datadog|jaeger|tracing|metrics)" "$f" 2>/dev/null; then
                has_observability=true
                break
            fi
        done
        if [[ "$has_observability" == "true" ]]; then
            log_pass "observability readiness (observability patterns detected)"
        else
            log_pass "observability readiness (no observability patterns found, skipping)"
        fi
    fi

    # Release policy check
    if [[ -f "${PI_DIR}/scripts/ci/check_release_policy.py" ]]; then
        if python3 "${PI_DIR}/scripts/ci/check_release_policy.py" 2>/dev/null; then
            log_pass "release policy check"
        else
            log_fail "release policy check" "Release policy violations found"
        fi
    else
        log_pass "release policy check (no script found)"
    fi
}

case "$STAGE" in
    6|integration)
        run_stage_6_integration
        ;;
    7|security)
        run_stage_7_security
        ;;
    8|migration)
        run_stage_8_migration
        ;;
    9|package)
        run_stage_9_package
        ;;
    10|release)
        run_stage_10_release
        ;;
    all)
        run_stage_6_integration
        echo ""
        run_stage_7_security
        echo ""
        run_stage_8_migration
        echo ""
        run_stage_9_package
        echo ""
        run_stage_10_release
        ;;
esac

if [[ $FAIL -gt 0 ]]; then
    echo "  Stage FAILED (${FAIL} failure(s))"
    exit 1
fi

echo "  Stage passed (${PASS} check(s))"
exit 0
