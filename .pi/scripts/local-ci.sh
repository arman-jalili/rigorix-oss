#!/usr/bin/env bash
# ═════════════════════════════════════════════════════════════════════
# local-ci.sh — Local CI Simulation
#
# Runs the exact same checks as the GitHub Actions CI pipeline,
# including all per-crate stage scripts discovered under .pi/scripts/ci/.
# Use this BEFORE pushing to verify CI will pass.
#
# Usage:
#   bash .pi/scripts/local-ci.sh              # Full CI simulation
#   bash .pi/scripts/local-ci.sh --quick      # Skip release build + slow checks
#   bash .pi/scripts/local-ci.sh --stage=lint # Run only one stage
#   bash .pi/scripts/local-ci.sh --crate=engine # Run for one crate only
#   bash .pi/scripts/local-ci.sh --list       # List all discovered scripts
#
# Exit code: 0 if all mandatory stages pass, 1 if any fail.
# ═════════════════════════════════════════════════════════════════════

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BLUE='\033[0;34m'
NC='\033[0m'

QUICK=false
STAGE=""
CRATE=""
LIST_ONLY=false
FAILED=0
PASSED=0
SKIPPED=0
START_TIME=$(date +%s)
declare -a STAGE_RESULTS=()

# Parse args
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick) QUICK=true; shift ;;
        --stage=*) STAGE="${1#*=}"; shift ;;
        --crate=*) CRATE="${1#*=}"; shift ;;
        --list) LIST_ONLY=true; shift ;;
        *) shift ;;
    esac
done

CRATES=("engine" "cli" "actions")
CRATE_LABELS=("engine" "cli" "actions")
if [[ -n "$CRATE" ]]; then
    CRATES=("$CRATE")
fi

pass()   { local msg="$1"; echo -e "  ${GREEN}✅ PASS${NC} ${msg}"; PASSED=$((PASSED + 1)); }
fail()   { local msg="$1"; echo -e "  ${RED}❌ FAIL${NC} ${msg}"; FAILED=$((FAILED + 1)); }
skip()   { local msg="$1"; echo -e "  ${YELLOW}⏭  SKIP${NC} ${msg}"; SKIPPED=$((SKIPPED + 1)); }
info()   { local msg="$1"; echo -e "  ${BLUE}ℹ️  ${NC} ${msg}"; }
header() { local name="$1"; echo -e "\n${CYAN}══════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}  ${name}${NC}"; echo -e "${CYAN}══════════════════════════════════════════════════════════${NC}"; }
subheader() { echo -e "\n${BLUE}--- ${1} ---${NC}"; }

run_check() {
    local check_name="$1"; shift
    local stage_name="$1"; shift
    if [[ -n "$STAGE" && "$STAGE" != "$stage_name" ]]; then
        return 0
    fi
    echo ""
    echo "  🔍 ${check_name}"
    local out err exit_code
    out="$("$@" 2>&1)" && exit_code=0 || exit_code=$?
    if [[ $exit_code -eq 0 ]]; then
        pass "${check_name}"
    else
        fail "${check_name}"
        # Show last 15 lines of output on failure
        echo "$out" | tail -15 | sed 's/^/      /'
    fi
    return $exit_code
}

run_sub_stage() {
    local name="$1"; shift
    local stage="$1"; shift
    local dir="$1"; shift
    local script="$1"; shift
    if [[ -n "$STAGE" && "$STAGE" != "$stage" ]]; then
        return 0
    fi
    if [[ ! -f "$script" ]]; then
        info "Script not found: $script"
        return 0
    fi
    echo ""
    echo "  🔍 ${name}"
    local out exit_code
    out="$(cd "$dir" && bash "$script" "$@" 2>&1)" && exit_code=0 || exit_code=$?
    if [[ $exit_code -eq 0 ]]; then
        pass "${name}"
    else
        fail "${name} (exit: ${exit_code})"
        echo "$out" | tail -20 | sed 's/^/      /'
    fi
    return $exit_code
}

should_run() {
    [[ -z "$STAGE" || "$STAGE" == "$1" ]]
}

# ── List Mode ───────────────────────────────────────────────────
if [[ "$LIST_ONLY" == "true" ]]; then
    echo -e "${CYAN}All discoverable CI scripts:\n${NC}"
    echo "── Root ──"
    find .pi/scripts -name "*.sh" -not -path "*/languages/*" | sort | sed 's/^/  /'
    for crate in engine cli actions; do
        echo ""
        echo "── ${crate} ──"
        if [[ -d "$crate/.pi/scripts/ci" ]]; then
            find "$crate/.pi/scripts/ci" -name "*.sh" | sort | sed 's/^/  /'
        fi
        if [[ -d "$crate/.pi/scripts" ]]; then
            find "$crate/.pi/scripts" -maxdepth 1 -name "*.sh" | sort | sed 's/^/  /'
        fi
    done
    echo ""
    info "Total crates: ${#CRATES[@]}"
    info "Root scripts: $(find .pi/scripts -name '*.sh' -not -path '*/languages/*' | wc -l | tr -d ' ')"
    for crate in engine cli actions; do
        count=$(find "$crate/.pi/scripts/ci" -name "*.sh" 2>/dev/null | wc -l | tr -d ' ')
        info "${crate} CI scripts: ${count:-0}"
    done
    exit 0
fi

# ── Header ──────────────────────────────────────────────────────
echo -e "${CYAN}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║           Rigorix Local CI Simulation                  ║${NC}"
echo -e "${CYAN}║           $(date)              ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "  Mode:    $([ "$QUICK" = true ] && echo 'quick' || echo 'full')"
echo "  Stage:   ${STAGE:-all}"
echo "  Crates:  ${CRATES[*]}"
echo "  Start:   $(date)"
echo ""

# ═════════════════════════════════════════════════════════════════
# Stage 1: Lint
# ═════════════════════════════════════════════════════════════════
if should_run "lint"; then
    header "Stage 1: Lint"

    for crate in "${CRATES[@]}"; do
        subheader "Lint: ${crate}"

        run_check "cargo fmt --check -p rigorix-${crate}" "lint" \
            bash -c "cargo fmt --check -p rigorix-${crate}"

        run_check "cargo clippy -p rigorix-${crate} -- -D warnings" "lint" \
            bash -c "cargo clippy -p rigorix-${crate} -- -D warnings"

        run_sub_stage "stage_lint.sh (${crate})" "lint" "$crate" ".pi/scripts/ci/stage_lint.sh"
        run_sub_stage "validate-ci.sh (${crate})" "lint" "$crate" ".pi/scripts/validate-ci.sh"
    done
fi

# ═════════════════════════════════════════════════════════════════
# Stage 2: Build
# ═════════════════════════════════════════════════════════════════
if should_run "build"; then
    header "Stage 2: Build"

    for crate in "${CRATES[@]}"; do
        subheader "Build: ${crate}"

        if [[ "$QUICK" == "true" ]]; then
            run_check "cargo check -p rigorix-${crate} (quick mode)" "build" \
                bash -c "cargo check -p rigorix-${crate}"
        else
            run_check "cargo build -p rigorix-${crate} --release" "build" \
                bash -c "cargo build -p rigorix-${crate} --release"
        fi

        run_sub_stage "stage_static_analysis.sh (${crate})" "build" "$crate" ".pi/scripts/ci/stage_static_analysis.sh"
        run_sub_stage "stage_package_build.sh (${crate})" "build" "$crate" ".pi/scripts/ci/stage_package_build.sh"
    done
fi

# ═════════════════════════════════════════════════════════════════
# Stage 3: Test
# ═════════════════════════════════════════════════════════════════
if should_run "test"; then
    header "Stage 3: Test"

    for crate in "${CRATES[@]}"; do
        subheader "Test: ${crate}"

        run_check "cargo test -p rigorix-${crate}" "test" \
            bash -c "cargo test -p rigorix-${crate} 2>&1 | tail -5"

        run_sub_stage "stage_unit.sh (${crate})" "test" "$crate" ".pi/scripts/ci/stage_unit.sh"
    done

    if [[ "$QUICK" != "true" ]]; then
        subheader "Integration Tests"
        run_check "cargo test --workspace -- --ignored" "test" \
            bash -c "cargo test --workspace -- --ignored 2>&1 | tail -5"
    else
        skip "integration tests (--quick mode)"
    fi
fi

# ═════════════════════════════════════════════════════════════════
# Stage 4: Security
# ═════════════════════════════════════════════════════════════════
if should_run "security"; then
    header "Stage 4: Security"

    subheader "Dependency Audit"
    if command -v cargo-audit &>/dev/null || cargo audit --version &>/dev/null 2>&1; then
        run_check "cargo audit" "security" \
            bash -c "cargo audit --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0002 2>&1 | tail -10"
    else
        fail "cargo-audit not installed — run 'cargo install cargo-audit'"
    fi

    subheader "Secret Scan"
    secrets_found=false
    for crate in "${CRATES[@]}"; do
        results=$(grep -rE "(sk-[A-Za-z0-9]{32,}|ghp_[A-Za-z0-9]{36}|AKIA[0-9A-Z]{16})" \
            --include="*.rs" --include="*.toml" --include="*.yml" --include="*.yaml" \
            --include="*.env" --include="*.sh" "$crate/" 2>/dev/null \
            | grep -v ".git" | grep -v ".rigorix" | grep -v "node_modules" \
            | grep -v "ghp_secret12345" \
            || true)
        if [[ -n "$results" ]]; then
            echo "    ❌ Potential secrets in ${crate}:"
            echo "$results" | sed 's/^/      /'
            secrets_found=true
        fi
    done
    if [[ "$secrets_found" == "false" ]]; then
        pass "secret scan"
    else
        fail "secret scan — false positives may need exclusion rules"
    fi

    subheader "Per-Crate Security"
    for crate in "${CRATES[@]}"; do
        run_sub_stage "stage_security.sh (${crate})" "security" "$crate" ".pi/scripts/ci/stage_security.sh"
    done
fi

# ═════════════════════════════════════════════════════════════════
# Stage 5: Documentation
# ═════════════════════════════════════════════════════════════════
if should_run "docs"; then
    header "Stage 5: Documentation"

    run_sub_stage "validate-canonical.sh" "docs" "." ".pi/scripts/validate-canonical.sh"
    run_sub_stage "validate-architecture.sh" "docs" "." ".pi/scripts/validate-architecture.sh"
    run_sub_stage "validate-architecture-readiness.sh" "docs" "." ".pi/scripts/validate-architecture-readiness.sh"
fi

# ═════════════════════════════════════════════════════════════════
# Stage 6: Integration
# ═════════════════════════════════════════════════════════════════
if should_run "integration"; then
    header "Stage 6: Integration"

    for crate in "${CRATES[@]}"; do
        run_sub_stage "stage_integration.sh (${crate})" "integration" "$crate" ".pi/scripts/ci/stage_integration.sh"
    done

    run_sub_stage "validate-integration.sh" "integration" "." ".pi/scripts/validate-integration.sh"
    run_sub_stage "validate-operations.sh" "integration" "." ".pi/scripts/validate-operations.sh"
fi

# ═════════════════════════════════════════════════════════════════
# Summary
# ═════════════════════════════════════════════════════════════════
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
MINS=$((DURATION / 60))
SECS=$((DURATION % 60))
TOTAL_CHECKS=$((PASSED + FAILED + SKIPPED))

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                     Final Results                        ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  ${GREEN}✅ Passed:${NC}  ${PASSED}"
echo -e "  ${RED}❌ Failed:${NC}  ${FAILED}"
echo -e "  ${YELLOW}⏭  Skipped:${NC} ${SKIPPED}"
echo -e "  Total:   ${TOTAL_CHECKS}"
echo -e "  Time:    ${MINS}m ${SECS}s"
echo ""

if [[ $FAILED -gt 0 ]]; then
    echo -e "${RED}╔══════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ❌ CI SIMULATION FAILED — ${FAILED} check(s) failed                ║${NC}"
    echo -e "${RED}╚══════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "  To run a single stage:   bash .pi/scripts/local-ci.sh --stage=lint"
    echo "  To run a single crate:   bash .pi/scripts/local-ci.sh --crate=engine"
    echo "  Quick mode (skip heavy): bash .pi/scripts/local-ci.sh --quick"
    echo "  List all scripts:        bash .pi/scripts/local-ci.sh --list"
    echo ""
    echo "  If stage_*.sh scripts fail, run them directly for full output:"
    echo "    (cd engine && bash .pi/scripts/ci/stage_unit.sh)"
    echo "    (cd engine && bash .pi/scripts/ci/stage_security.sh)"
    exit 1
else
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ✅ CI SIMULATION PASSED — all ${PASSED} checks passed            ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "  Ready to push."
    exit 0
fi
