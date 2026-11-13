#!/usr/bin/env bash
# local-ci.sh — Local CI Simulation
# Discovers and runs ALL CI scripts across all crates.
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BLUE='\033[0;34m'; NC='\033[0m'

QUICK=false; RELEASE=false; STAGE=""; CRATE_FILTER=""; LIST_ONLY=false; SAVE_OUTPUT=false; AFFECTED=false
FAILED=0; PASSED=0; SKIPPED=0; START_TIME=$(date +%s)
FAILURES=()
CRATES=("engine" "mcp" "cli" "actions" "server")
REPORT_DIR=".pi/output"
VALID_STAGES=" lint build test security coverage docs integration "

usage() {
    echo "usage: local-ci.sh [--quick] [--release] [--stage=<stage>|--stage <stage>]" >&2
    echo "                   [--crate=<crate>] [--list] [--save]" >&2
    echo "                   [--affected[=<base>]]  # narrow to what the diff can reach (#1441)" >&2
    echo "stages:${VALID_STAGES}" >&2
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick) QUICK=true; shift ;;
        --release) RELEASE=true; shift ;;
        --stage=*) STAGE="${1#*=}"; shift ;;
        --stage)
            [[ $# -ge 2 ]] || { echo "local-ci: --stage requires a value" >&2; usage; exit 2; }
            STAGE="$2"; shift 2 ;;
        --crate=*) CRATE_FILTER="${1#*=}"; shift ;;
        --crate)
            [[ $# -ge 2 ]] || { echo "local-ci: --crate requires a value" >&2; usage; exit 2; }
            CRATE_FILTER="$2"; shift 2 ;;
        --list) LIST_ONLY=true; shift ;;
        --affected) AFFECTED=true; AFFECTED_BASE="origin/main"; shift ;;
        --affected=*) AFFECTED=true; AFFECTED_BASE="${1#*=}"; shift ;;
        --save) SAVE_OUTPUT=true; shift ;;
        *)
            echo "local-ci: unknown argument '$1'" >&2
            usage
            exit 2 ;;
    esac
done

if [[ -n "$STAGE" && "$VALID_STAGES" != *" $STAGE "* ]]; then
    echo "local-ci: unknown stage '$STAGE'" >&2
    usage
    exit 2
fi

[[ -n "$CRATE_FILTER" ]] && CRATES=("$CRATE_FILTER")

# ── narrowing by the diff (#1441) ────────────────────────────────────────────
# The table is `.github/ci-affected.tsv` — the same one CI reads, so the developer computes the
# pipeline's subject before pushing instead of discovering it afterwards. A crate the map does
# not select is named as skipped, never silently dropped: a run that quietly checked less than
# it says is the failure mode the table exists to prevent.
if [[ "$AFFECTED" == "true" ]]; then
    AFFECTED_SUBJECTS="$(bash .github/scripts/ci_affected.sh --base "$AFFECTED_BASE" --list 2>/dev/null || echo ALL)"
    if [[ "$AFFECTED_SUBJECTS" == "ALL" ]]; then
        echo "  affected: ALL (nothing is narrower than this diff)"
    else
        KEPT=()
        for crate in "${CRATES[@]}"; do
            if printf '%s\n' "$AFFECTED_SUBJECTS" | grep -qx "$crate"; then
                KEPT+=("$crate")
            else
                echo "  affected: skipping $crate (the diff cannot reach it)"
            fi
        done
        CRATES=("${KEPT[@]}")
        echo "  affected: ${AFFECTED_SUBJECTS//$'\n'/, }"
    fi
    if [[ "$LIST_ONLY" == "true" ]]; then
        printf '%s\n' "$AFFECTED_SUBJECTS"
        exit 0
    fi
fi
mkdir -p "$REPORT_DIR"
REPORT_FILE="$REPORT_DIR/ci-report-$(date +%Y%m%d-%H%M%S).txt"

# If --save, redirect all output to both stdout and file
if [[ "$SAVE_OUTPUT" == "true" ]]; then
    exec &> >(tee "$REPORT_FILE")
    echo "Saving report to: $REPORT_FILE"
fi

pass()   { echo -e "  ${GREEN}✅ PASS${NC} $1"; PASSED=$((PASSED + 1)); }
fail()   {
    local label="$1"
    local details="${2:-}"
    echo -e "  ${RED}❌ FAIL${NC} $label"
    FAILED=$((FAILED + 1))
    FAILURES+=("$label${details:+ — $details}")
}
skip()   { echo -e "  ${YELLOW}⏭  SKIP${NC} $1"; SKIPPED=$((SKIPPED + 1)); }
info()   { echo -e "  ${BLUE}ℹ️  ${NC} $1"; }
header() { echo -e "\n${CYAN}══════════════════════════════════════════════════════════\n  $1\n${CYAN}══════════════════════════════════════════════════════════${NC}"; }
subheader() { echo -e "\n${BLUE}--- $1 ---${NC}"; }

run_cmd() {
    local label="$1"; shift
    echo "  🔍 $label"
    local out exit_code
    out="$("$@" 2>&1)" && exit_code=0 || exit_code=$?
    if [[ $exit_code -eq 0 ]]; then pass "$label"
    else
        local details
        details="$(echo "$out" | tail -5 | tr '\n' '; ' | head -c 200)"
        fail "$label (exit: ${exit_code})" "$details"
        echo "$out" | tail -15 | sed 's/^/      /'
    fi
}

run_script() {
    local label="$1" crate="$2" script_abs="$3"
    # script_abs is e.g. "engine/.pi/scripts/ci/stage_lint.sh"
    # We strip the crate prefix so it runs from the crate dir
    local script_rel="${script_abs#$crate/}"
    echo "  🔍 $label"
    local out exit_code
    out="$(cd "$crate" && bash "$script_rel" 2>&1)" && exit_code=0 || exit_code=$?
    if [[ $exit_code -eq 0 ]]; then pass "$label"
    else
        local details
        details="$(echo "$out" | tail -5 | tr '\n' '; ' | head -c 200)"
        fail "$label (exit: ${exit_code})" "$details"
        echo "$out" | tail -15 | sed 's/^/      /'
    fi
}

run_root_script() {
    local label="$1" script="$2"
    run_script "$label" "." "$script"
}

should_run() { [[ -z "$STAGE" || "$STAGE" == "$1" ]]; }

# ── List Mode ───────────────────────────────────────────────────
if [[ "$LIST_ONLY" == "true" ]]; then
    echo -e "${CYAN}All discoverable CI scripts:\n${NC}"
    echo "── Root scripts ──"
    root_count=0
    for f in .pi/scripts/*.sh; do
        [[ -f "$f" ]] && echo "  $(basename "$f")" && root_count=$((root_count + 1))
    done
    echo "  → $root_count scripts"
    echo ""

    for crate in engine cli actions; do
        echo "── $crate CI scripts ──"
        count=0
        # First list ci/ scripts
        for f in "$crate/.pi/scripts/ci/"*.sh; do
            if [[ -f "$f" ]]; then
                echo "  $(basename "$f")"
                count=$((count + 1))
            fi
        done
        # Then list validate-* scripts (at $crate/.pi/scripts/ level)
        for f in "$crate/.pi/scripts/"validate-*.sh; do
            if [[ -f "$f" ]]; then
                echo "  $(basename "$f")"
                count=$((count + 1))
            fi
        done
        echo "  → $count scripts"
        echo ""
    done

    info "Total crates: ${#CRATES[@]}"
    info "Root scripts: $root_count"
    for crate in engine cli actions; do
        # Count both ci/ scripts and validate-* scripts for consistency
        ccount=$(find "$crate/.pi/scripts/ci" -maxdepth 1 -name "*.sh" 2>/dev/null | wc -l | tr -d ' ')
        vcount=$(find "$crate/.pi/scripts" -maxdepth 1 -name "validate-*.sh" 2>/dev/null | wc -l | tr -d ' ')
        total=$((ccount + vcount))
        info "$crate CI scripts: $total (${ccount} ci/ + ${vcount} validate)"
    done
    exit 0
fi

echo -e "${CYAN}╔══════════════════════════════════════════════════════════╗"
echo -e "║           Rigorix Local CI Simulation                  ║"
echo -e "║           $(date)              ║"
echo -e "╚══════════════════════════════════════════════════════════╝${NC}"
echo "  Mode:    $([ "$QUICK" = true ] && echo 'quick' || echo 'full')"
echo "  Profile: $([ "$RELEASE" = true ] && echo 'release' || echo 'debug')"
echo "  Stage:   ${STAGE:-all}"
echo "  Crates:  ${CRATES[*]}"; echo ""

# ═════════════════════════════════════════════════════════════════
# Stage 1: Lint
# ═════════════════════════════════════════════════════════════════
if should_run "lint"; then
    header "Stage 1: Lint"
    for crate in "${CRATES[@]}"; do
        subheader "Lint: $crate"
        run_cmd "cargo fmt --check -p rigorix-$crate" cargo fmt --check -p "rigorix-$crate"
        run_cmd "cargo clippy -p rigorix-$crate -- -D warnings" cargo clippy -p "rigorix-$crate" -- -D warnings
        for script in "$crate/.pi/scripts/ci/stage_lint.sh" "$crate/.pi/scripts/validate-ci.sh"; do
            [[ -f "$script" ]] && run_script "$(basename "$script") ($crate)" "$crate" "$script"
        done
    done
fi

# ═════════════════════════════════════════════════════════════════
# Stage 2: Build
# ═════════════════════════════════════════════════════════════════
if should_run "build"; then
    header "Stage 2: Build"
    for crate in "${CRATES[@]}"; do
        subheader "Build: $crate"
        if [[ "$QUICK" == "true" ]]; then
            run_cmd "cargo check -p rigorix-$crate (quick)" cargo check -p "rigorix-$crate"
        elif [[ "$RELEASE" == "true" ]]; then
            run_cmd "cargo build -p rigorix-$crate --release" cargo build -p "rigorix-$crate" --release
        else
            run_cmd "cargo build -p rigorix-$crate" cargo build -p "rigorix-$crate"
        fi
        for script in "$crate/.pi/scripts/ci/stage_static_analysis.sh" "$crate/.pi/scripts/ci/stage_package_build.sh"; do
            [[ -f "$script" ]] && run_script "$(basename "$script") ($crate)" "$crate" "$script"
        done
    done
fi

# ═════════════════════════════════════════════════════════════════
# Stage 3: Test (cargo nextest + doctests + all proofing scripts)
# ═════════════════════════════════════════════════════════════════
if should_run "test"; then
    header "Stage 3: Test"
    for crate in "${CRATES[@]}"; do
        subheader "Test: $crate"
        # #1373: nextest is the runner (per-test process isolation, per-test timing).
        # Falling back to `cargo test` rather than failing keeps a contributor without
        # the tool unblocked — but the fallback is announced, because the two runners do
        # not report the same way and a silent substitution is how a result gets quoted
        # against the wrong subject.
        if command -v cargo-nextest >/dev/null 2>&1; then
            run_cmd "cargo nextest run -p rigorix-$crate" cargo nextest run -p "rigorix-$crate"
        else
            echo "  ⚠️  cargo-nextest not installed — falling back to cargo test (https://get.nexte.st)"
            run_cmd "cargo test -p rigorix-$crate" cargo test -p "rigorix-$crate"
        fi
        run_cmd "doctests -p rigorix-$crate" bash .pi/scripts/run-doctests.sh -p "rigorix-$crate"
        # All stage_*.sh scripts that aren't already in lint/build
        for script in "$crate/.pi/scripts/ci/stage_unit.sh" "$crate/.pi/scripts/ci/stage_integration.sh"; do
            [[ -f "$script" ]] && run_script "$(basename "$script") ($crate)" "$crate" "$script"
        done
        # All proofing scripts (*_proofing.sh)
        for script in "$crate/.pi/scripts/ci/"*_proofing.sh; do
            if [[ -f "$script" ]]; then
                run_script "$(basename "$script") ($crate)" "$crate" "$script"
            fi
        done
    done
fi

# ═════════════════════════════════════════════════════════════════
# Stage 4: Security
# ═════════════════════════════════════════════════════════════════
if should_run "security"; then
    header "Stage 4: Security"
    if command -v cargo-audit &>/dev/null || cargo audit --version &>/dev/null 2>&1; then
        run_cmd "cargo audit" cargo audit --deny warnings
    else
        fail "cargo audit — not installed (run 'cargo install cargo-audit')"
    fi
    # Secret scan
    secrets_found=false
    for crate in "${CRATES[@]}"; do
        while IFS= read -r line; do
            echo "    ❌ Secret in ${crate}: $line"
            secrets_found=true
        done < <(grep -rE "(sk-[A-Za-z0-9]{32,}|ghp_[A-Za-z0-9]{36}|AKIA[0-9A-Z]{16})" \
            --include="*.rs" --include="*.toml" --include="*.yml" --include="*.yaml" \
            --include="*.env" --include="*.sh" "$crate/" 2>/dev/null \
            | grep -v ".git" | grep -v ".rigorix" | grep -v "ghp_secret12345" | grep -v "AKIAIOSFODNN7EXAMPLE" || true)
    done
    if [[ "$secrets_found" == "false" ]]; then pass "secret scan"; else fail "secret scan"; fi
    # The security validator's own seed: a non-advisory audit failure must print, not abort.
    # `run_here=no` in `.pi/guards.tsv` keeps it out of `validate-guards.sh` (it has no fixtures
    # for the runner), so the local runner is where its self-test is executed (#1531).
    if [[ -f .pi/scripts/languages/rust/validate-security.sh ]]; then
        run_cmd "validate-security.sh --self-test" \
            bash .pi/scripts/languages/rust/validate-security.sh --self-test
    fi
    # Per-crate security scripts
    for crate in "${CRATES[@]}"; do
        for script in "$crate/.pi/scripts/ci/stage_security.sh" "$crate/.pi/scripts/validate-security.sh"; do
            [[ -f "$script" ]] && run_script "$(basename "$script") ($crate)" "$crate" "$script"
        done
    done
fi

# ═════════════════════════════════════════════════════════════════
# ═════════════════════════════════════════════════════════════════
# Stage 4b: Coverage (real line-coverage measurement via cargo-llvm-cov)
# ═════════════════════════════════════════════════════════════════
if should_run "coverage"; then
    header "Stage 4b: Coverage"
    run_cmd "coverage (workspace, gated >=${COVERAGE_THRESHOLD:-60}%)" bash .pi/scripts/coverage.sh --gate
fi

# Stage 5: Documentation
# ═════════════════════════════════════════════════════════════════
if should_run "docs"; then
    header "Stage 5: Documentation"
    # GAP-A-27: the root-level validators assume a flat src/ layout and fail
    # on this monorepo (no src/ at root). They are fully covered per-crate
    # below, so only the per-crate variants run.
    for crate in "${CRATES[@]}"; do
        for script in "$crate/.pi/scripts/validate-canonical.sh" \
                      "$crate/.pi/scripts/validate-architecture.sh" \
                      "$crate/.pi/scripts/validate-architecture-readiness.sh"; do
            [[ -f "$script" ]] && run_script "$(basename "$script") ($crate)" "$crate" "$script"
        done
    done
fi

# ═════════════════════════════════════════════════════════════════
# Stage 6: Integration
# ═════════════════════════════════════════════════════════════════
if should_run "integration"; then
    header "Stage 6: Integration"
    for script in .pi/scripts/validate-integration.sh .pi/scripts/validate-operations.sh; do
        [[ -f "$script" ]] && run_root_script "$(basename "$script")" "$script"
    done
fi

# ═════════════════════════════════════════════════════════════════
# Summary
# ═════════════════════════════════════════════════════════════════
END_TIME=$(date +%s); DURATION=$((END_TIME - START_TIME))
MINS=$((DURATION / 60)); SECS=$((DURATION % 60))
echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════╗"
echo -e "║                     Final Results                        ║"
echo -e "╚══════════════════════════════════════════════════════════╝${NC}"
echo "  ✅ Passed steps:  $PASSED"
echo "  ❌ Failed steps:  $FAILED"
echo "  ⏭  Skipped steps: $SKIPPED"
echo "  Total steps:  $((PASSED + FAILED + SKIPPED))"
echo "  Time:    ${MINS}m ${SECS}s"
echo ""
if [[ $FAILED -gt 0 ]]; then
    echo ""
    echo -e "${RED}═══════════════ Failed Steps ═══════════════${NC}"
    for f in "${FAILURES[@]}"; do
        label="${f%% — *}"
        detail="${f#* — }"
        if [[ "$detail" == "$label" ]]; then
            echo -e "  ${RED}❌${NC} $label"
        else
            echo -e "  ${RED}❌${NC} $label"
            echo "      ${detail}"
        fi
    done
    echo ""
    echo -e "${RED}❌ CI SIMULATION FAILED — ${FAILED} check(s) failed${NC}"
    echo "  Debug a single stage: bash .pi/scripts/local-ci.sh --stage=<stage>"
    echo "  Debug a single crate: bash .pi/scripts/local-ci.sh --crate=<crate>"
    exit 1
else
    echo -e "${GREEN}✅ CI SIMULATION PASSED — all ${PASSED} checks passed${NC}"
    exit 0
fi
