#!/usr/bin/env bash
# local-ci.sh — Local CI Simulation
# Discovers and runs ALL CI scripts across all crates.
#
# Cost is part of the contract (#1551). Every step prints its own wall-clock
# duration, each stage is timed and reported in a final table with its share of
# the run, independent stages run concurrently with `--jobs`, and
# `.pi/local-ci-budget.tsv` holds a ceiling per stage that was derived from a
# measured run — the run fails when a stage exceeds it.
#
# One run per tree (#1552). A run takes a lock and a second concurrent run is
# refused with the holding pid, because two runs share `target/` and
# `target/llvm-cov-target/`: a binary replaced under a running test process
# produces `process didn't exit successfully` with no test name, which is a
# riddle rather than a verdict.
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BLUE='\033[0;34m'; NC='\033[0m'

QUICK=false; RELEASE=false; STAGE=""; CRATE_FILTER=""; LIST_ONLY=false; SAVE_OUTPUT=false; AFFECTED=false
SELF_TEST=false; PROBE_EXIT=""; LOCK_FILE=""; JOBS=""
START_TIME=$(date +%s)
CRATES=("engine" "mcp" "cli" "actions" "server")
REPORT_DIR=".pi/output"
VALID_STAGES=" lint build test security guards coverage docs integration "
# The table's order, and the serial order: the legacy one, so `--jobs 1` reproduces what a
# contributor ran before this change (#1551's acceptance asks for the order and the duration).
STAGE_ORDER=("lint" "build" "test" "security" "guards" "coverage" "docs" "integration")

usage() {
    echo "usage: local-ci.sh [--quick] [--release] [--stage=<stage>|--stage <stage>]" >&2
    echo "                   [--crate=<crate>] [--list] [--save]" >&2
    echo "                   [--affected[=<base>]]  # narrow to what the diff can reach (#1441)" >&2
    echo "                   [--jobs <n>]           # stages that share no state run concurrently" >&2
    echo "  guards = the pipeline's audit gate: .pi/scripts/validate-guards.sh + its self-test (#1546)" >&2
    echo "stages:${VALID_STAGES}" >&2
    echo "" >&2
    echo "  Concurrency: lint, docs, security and guards are independent and may run together;" >&2
    echo "  build precedes test precedes coverage. --jobs 1 is the serial order." >&2
    echo "  A run holds a lock ($LOCK_FILE) for its lifetime: a second run in the same tree is" >&2
    echo "  refused, because both would build into the same target/ and target/llvm-cov-target/." >&2
    echo "  Budget: .pi/local-ci-budget.tsv holds a measured ceiling per stage; the run fails" >&2
    echo "  when a stage exceeds it." >&2
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
        --jobs=*) JOBS="${1#*=}"; shift ;;
        --jobs)
            [[ $# -ge 2 ]] || { echo "local-ci: --jobs requires a value" >&2; usage; exit 2; }
            JOBS="$2"; shift 2 ;;
        --lock-file=*) LOCK_FILE="${1#*=}"; shift ;;
        --lock-file)
            [[ $# -ge 2 ]] || { echo "local-ci: --lock-file requires a value" >&2; usage; exit 2; }
            LOCK_FILE="$2"; shift 2 ;;
        --self-test) SELF_TEST=true; shift ;;
        --probe-exit=*) PROBE_EXIT="${1#*=}"; shift ;;
        --probe-exit)
            [[ $# -ge 2 ]] || { echo "local-ci: --probe-exit requires a value" >&2; usage; exit 2; }
            PROBE_EXIT="$2"; shift 2 ;;
        --help|-h) usage; exit 0 ;;
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

# ── the lock (#1552) ─────────────────────────────────────────────────────────
# Two runs in one tree write the same `target/` and `target/llvm-cov-target/`, and a test binary
# replaced while it is executing exits non-zero with no test name — a verdict nobody can act on.
# `mkdir` is the atomic primitive that exists on both macOS and Linux (there is no portable
# `flock` binary), and the pid inside is what makes the refusal name something.
[[ -n "$LOCK_FILE" ]] || LOCK_FILE=".pi/.local-ci.lock"
LOCK_HELD=false

acquire_lock() {
    if mkdir "$LOCK_FILE" 2>/dev/null; then
        printf '%s\n' "$$" > "$LOCK_FILE/pid"
        printf '%s\n' "$(pwd)" > "$LOCK_FILE/tree"
        LOCK_HELD=true
        return 0
    fi
    local holder tree
    holder="$(cat "$LOCK_FILE/pid" 2>/dev/null || echo 'unknown')"
    tree="$(cat "$LOCK_FILE/tree" 2>/dev/null || echo 'unknown')"
    echo -e "${RED}local-ci: another run holds the lock${NC}" >&2
    echo "  lock:  $LOCK_FILE" >&2
    echo "  held by pid $holder in $tree" >&2
    echo "  Two runs in one tree build into the same target/ and target/llvm-cov-target/; a test" >&2
    echo "  binary replaced mid-run fails with no test name. Wait for it, or remove the lock if" >&2
    echo "  that pid is gone." >&2
    return 1
}

release_lock() {
    if [[ "$LOCK_HELD" == "true" ]] && [[ "$(cat "$LOCK_FILE/pid" 2>/dev/null || echo '')" == "$$" ]]; then
        find "$LOCK_FILE" -mindepth 1 -delete 2>/dev/null || true
        rmdir "$LOCK_FILE" 2>/dev/null || true
    fi
}

# ── exit statuses that are not verdicts (#1552) ──────────────────────────────
# bash reports a signal death as 128+n, which is how a SIGKILL arrives as "exit 137" and looks like
# a failing test. Say what it was.
describe_exit() {
    local rc="$1"
    if [[ "$rc" -gt 128 ]]; then
        local sig=$((rc - 128))
        local name="signal $sig"
        case "$sig" in
            2) name="SIGINT" ;; 3) name="SIGQUIT" ;; 6) name="SIGABRT" ;;
            9) name="SIGKILL" ;; 11) name="SIGSEGV" ;; 13) name="SIGPIPE" ;;
            15) name="SIGTERM" ;; 24) name="SIGXCPU" ;; 25) name="SIGXFSZ" ;;
        esac
        printf 'killed by %s (exit %s)' "$name" "$rc"
    else
        printf 'exit %s' "$rc"
    fi
}

# A scheduled stage that leaves no result file did not run, and a stage that does not run cannot
# fail. This is a function rather than four inline lines because the run needs it and the self-test
# needs to prove it bites (#1551): the first version of these waves dropped `build` entirely — 111
# checks instead of 126 — and reported a green run, because the summary only described the stages
# that had written something.
missing_stages() {  # $1 results dir, then the keys that were scheduled; prints the missing ones
    local dir="$1"; shift
    local key missing=()
    for key in "$@"; do
        [[ -f "$dir/$key.start" ]] || missing+=("$key")
    done
    if [[ "${#missing[@]}" -gt 0 ]]; then
        printf '%s\n' "${missing[@]}"
        return 1
    fi
    return 0
}

# ── self-test: the fixtures #1552 is proved with ─────────────────────────────
# The lock's test does not depend on this repository being idle: the fixture builds the state a
# running run leaves behind (`mkdir` + pid) and asserts the refusal names it. That is the property
# — the state, not the race.
if [[ "$SELF_TEST" == "true" ]]; then
    tmp="$(mktemp -d)"
    trap 'find "${tmp:-/nonexistent}" -mindepth 0 -delete 2>/dev/null || true' EXIT
    failures=0; checks=0
    check() {  # $1 label, $2 got, $3 wanted
        checks=$((checks + 1))
        if [[ "$2" == "$3" ]]; then
            echo -e "  ${GREEN}✅${NC} $1"
        else
            echo -e "  ${RED}❌${NC} $1 (got '$2', wanted '$3')"
            failures=$((failures + 1))
        fi
    }
    contains() {  # $1 label, $2 haystack, $3 needle
        checks=$((checks + 1))
        if printf '%s' "$2" | grep -qF "$3"; then
            echo -e "  ${GREEN}✅${NC} $1"
        else
            echo -e "  ${RED}❌${NC} $1 — '$3' not in: $(printf '%s' "$2" | head -3 | tr '\n' ' ')"
            failures=$((failures + 1))
        fi
    }

    echo "── local-ci self-test: the lock and the exit statuses ──"

    check "a signal death is named, not reported as a failing test" "$(describe_exit 137)" "killed by SIGKILL (exit 137)"
    check "SIGTERM is named" "$(describe_exit 143)" "killed by SIGTERM (exit 143)"
    check "an ordinary failure stays an exit status" "$(describe_exit 1)" "exit 1"

    # A held lock, exactly as a running run leaves it.
    mkdir -p "$tmp/held"
    echo "$$" > "$tmp/held/pid"
    echo "$tmp" > "$tmp/held/tree"
    out="$(bash "$0" --lock-file "$tmp/held" --probe-exit 0 2>&1)" && code=0 || code=$?
    check "a second run in a held tree is refused" "$code" "1"
    contains "the refusal names the holding pid" "$out" "held by pid $$"
    check "the refused run leaves the first run's lock in place" "$(cat "$tmp/held/pid")" "$$"

    # A free lock is taken, used, and released.
    out="$(bash "$0" --lock-file "$tmp/free" --probe-exit 0 2>&1)" && code=0 || code=$?
    check "a free lock is taken and the probe exits 0" "$code" "0"
    if [[ -e "$tmp/free" ]]; then
        check "the lock is released on exit" "still there" "gone"
    else
        check "the lock is released on exit" "gone" "gone"
    fi

    # A scheduled stage that wrote nothing is not a pass. The fixture is the exact state the bug
    # left behind: `lint` ran, `build` never did.
    mkdir -p "$tmp/results"
    date +%s > "$tmp/results/lint.start"
    out="$(missing_stages "$tmp/results" lint build 2>&1)" && code=0 || code=$?
    check "a scheduled stage with no result fails the run" "$code" "1"
    check "it names the stage that did not run" "$out" "build"
    out="$(missing_stages "$tmp/results" lint 2>&1)" && code=0 || code=$?
    check "a fully scheduled run passes the same check" "$code" "0"

    # The probe's own exit status is passed through, with its name.
    out="$(bash "$0" --lock-file "$tmp/free" --probe-exit 9 2>&1)" && code=0 || code=$?
    check "the probe's status is its own" "$code" "9"

    if [[ "$failures" -gt 0 ]]; then
        echo -e "${RED}local-ci self-test: $failures of $checks case(s) failed${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ local-ci self-test: $checks/$checks${NC}"
    exit 0
fi

acquire_lock || exit 1
trap 'release_lock' EXIT

if [[ -n "$PROBE_EXIT" ]]; then
    # A fixture mode for --self-test: take the lock, exit with a chosen status, touch nothing.
    exit "$PROBE_EXIT"
fi

# ── jobs default: the smaller of 4 and the core count ────────────────────────
if [[ -z "$JOBS" ]]; then
    cores="$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
    JOBS=4
    [[ "$cores" -lt "$JOBS" ]] && JOBS="$cores"
fi
[[ "$JOBS" -ge 1 ]] || { echo "local-ci: --jobs must be >= 1" >&2; exit 2; }
[[ -n "$STAGE" ]] && JOBS=1   # one stage is one stage; concurrency would be an accident

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

# ── stage bookkeeping ────────────────────────────────────────────────────────
# Counters live in files, per stage, because stages may run in subshells: a global incremented in a
# subshell is a number that disappears, which is how a run reports "0 failed" while printing a
# failure. Every `pass`/`fail` writes a row; the summary reads the rows.
RESULTS_DIR="$REPORT_DIR/.run-$$"
mkdir -p "$RESULTS_DIR"
CUR_STAGE="run"
STAGE_LOG="/dev/null"

fmt_dur() {
    local s="$1"
    if [[ "$s" -ge 60 ]]; then printf '%dm %02ds' $((s / 60)) $((s % 60)); else printf '%ds' "$s"; fi
}

stage_begin() {
    CUR_STAGE="$1"
    STAGE_LOG="$RESULTS_DIR/$1.log"
    : > "$STAGE_LOG"
    date +%s > "$RESULTS_DIR/$1.start"
    header "$2"
}

stage_end() {
    date +%s > "$RESULTS_DIR/$CUR_STAGE.end"
}

record() {  # $1 status, $2 label, $3 seconds, $4 detail
    printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4:-}" >> "$RESULTS_DIR/$CUR_STAGE.tsv"
}
pass() { echo -e "  ${GREEN}✅ PASS${NC} $1"; record PASS "$1" "${STEP_SECONDS:-0}" ""; }
fail() {
    echo -e "  ${RED}❌ FAIL${NC} $1"
    [[ -n "${2:-}" ]] && echo "      $2"
    record FAIL "$1" "${STEP_SECONDS:-0}" "${2:-}"
}
skip() { echo -e "  ${YELLOW}⏭  SKIP${NC} $1"; record SKIP "$1" 0 ""; }
info() { echo -e "  ${BLUE}ℹ️  ${NC} $1"; }
header() { echo -e "\n${CYAN}══════════════════════════════════════════════════════════\n  $1\n${CYAN}══════════════════════════════════════════════════════════${NC}"; }
subheader() { echo -e "\n${BLUE}--- $1 ---${NC}"; }

# A failed step's one-line summary has to name what failed. `tail -5` of a
# guard's output ends in the last guard that passed — the summary then reads
# "❌ guards: gating set … ✅ ADR references: every one of 14 …", which names the
# wrong thing. Prefer the lines that carry a ❌; fall back to the tail only when
# the output has none (#1546).
strip_ansi() { printf '%s' "${1//$'\033'\[[0-9;]*m/}"; }

first_failures() {
    local bad
    bad="$(strip_ansi "$1" | grep -F '❌' | tail -2 | tr '\n' '; ' | head -c 200)"
    if [[ -z "$bad" ]]; then
        bad="$(strip_ansi "$1" | tail -3 | tr '\n' '; ' | head -c 200)"
    fi
    printf '%s' "$bad"
}

# `run_step` streams: the child's output goes to a file, a heartbeat prints the last line every
# fifteen seconds, and the exit status is described rather than collapsed (#1551, #1552). A long
# compile and a hang must not look the same on screen.
run_step() {
    local label="$1"; shift
    local log="$RESULTS_DIR/$CUR_STAGE.steps.log"
    local t0 t_elapsed pid rc last shown=0
    echo "  🔍 $label"
    t0=$SECONDS
    "$@" >> "$log" 2>&1 &
    pid=$!
    # Poll cheaply; the heartbeat is a separate interval. A 5-second poll made every step take
    # five seconds — a floor nobody could see in the table, which is the same defect as a cost
    # nobody could attribute.
    while kill -0 "$pid" 2>/dev/null; do
        sleep 0.2
        if kill -0 "$pid" 2>/dev/null; then
            t_elapsed=$((SECONDS - t0))
            if [[ $((t_elapsed - shown)) -ge 15 ]]; then
                shown=$t_elapsed
                last="$(tail -n 1 "$log" 2>/dev/null | tr -d '\r' | head -c 120)"
                printf '     … %s (%s) %s\n' "$label" "$(fmt_dur "$t_elapsed")" "${last}"
            fi
        fi
    done
    rc=0
    wait "$pid" || rc=$?
    STEP_SECONDS=$((SECONDS - t0))
    if [[ "$rc" -eq 0 ]]; then
        pass "$label ($(fmt_dur "$STEP_SECONDS"))"
    else
        local out details
        out="$(tail -n 40 "$log" 2>/dev/null || true)"
        details="$(first_failures "$out")"
        fail "$label — $(describe_exit "$rc") ($(fmt_dur "$STEP_SECONDS"))" "$details"
        printf '%s\n' "$out" | tail -15 | sed 's/^/      /'
    fi
}

run_cmd() {
    local label="$1"; shift
    run_step "$label" "$@"
}

run_script() {
    local label="$1" crate="$2" script_abs="$3"
    # script_abs is e.g. "engine/.pi/scripts/ci/stage_lint.sh"
    # We strip the crate prefix so it runs from the crate dir
    local script_rel="${script_abs#$crate/}"
    local fn
    fn="$(mktemp -d)/run"
    {
        echo '#!/usr/bin/env bash'
        printf 'cd %q || exit 1\n' "$crate"
        printf 'exec bash %q\n' "$script_rel"
    } > "$fn"
    chmod +x "$fn"
    run_step "$label" "$fn"
}

run_root_script() {
    local label="$1" script="$2"
    run_script "$label" "." "$script"
}

should_run() { [[ -z "$STAGE" || "$STAGE" == "$1" ]]; }

# A per-stage ceiling measures a stage *inside* a run, not alone (#1551). On a tree that has not been
# built, the first stage to touch each crate pays for the whole compile and reads as a regression:
# measured after a mutation invalidated `target/`, `--stage guards` cost 2m13s against a 112s ceiling,
# against 43s warm. Recorded here, before any stage runs, so a budget failure can say so instead of
# implying the stage got slower.
COLD_START=false
if [[ ! -d target/debug/deps ]] || [[ -z "$(ls -A target/debug/deps 2>/dev/null || true)" ]]; then
    COLD_START=true
fi

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
echo "  Crates:  ${CRATES[*]}"
echo "  Jobs:    $JOBS$([[ "$JOBS" -gt 1 ]] && echo ' (independent stages run concurrently)' || echo ' (serial)')"
echo ""

# ═════════════════════════════════════════════════════════════════
# Stage functions. Each writes its own results rows, so a stage run in a
# subshell cannot lose a counter.
# ═════════════════════════════════════════════════════════════════

stage_lint() {
    stage_begin lint "Stage 1: Lint"
    for crate in "${CRATES[@]}"; do
        subheader "Lint: $crate"
        run_cmd "cargo fmt --check -p rigorix-$crate" cargo fmt --check -p "rigorix-$crate"
        run_cmd "cargo clippy -p rigorix-$crate -- -D warnings" cargo clippy -p "rigorix-$crate" -- -D warnings
        for script in "$crate/.pi/scripts/ci/stage_lint.sh" "$crate/.pi/scripts/validate-ci.sh"; do
            [[ -f "$script" ]] && run_script "$(basename "$script") ($crate)" "$crate" "$script"
        done
    done
    stage_end
}

stage_docs() {
    stage_begin docs "Stage 5: Documentation"
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
    stage_end
}

stage_security() {
    stage_begin security "Stage 4: Security"
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
    stage_end
}

stage_guards() {
    # The `🔒 Audit` job runs `.pi/scripts/validate-guards.sh` and its self-test
    # (`.github/workflows/ci.yml`). This stage runs the same commands on the
    # same subject, because a local run that never executes the gating set
    # reports `0 failed` about a smaller subject than the pipeline's — which
    # happened twice on this repository while the report said "all gating guards
    # passed" (#1546). Not behind a flag: an opt-in gate is a default of "ignore".
    stage_begin guards "Stage 4c: Guards"
    run_cmd "guards: gating set (ci audit job)" bash .pi/scripts/validate-guards.sh
    run_cmd "guards: self-test (each gating guard bites)" bash .pi/scripts/validate-guards.sh --self-test
    run_cmd "guards: coverage (this runner executes what gates)" bash .pi/scripts/validate-guards.sh --coverage
    run_cmd "guards: runner self-test (the lock and the exit statuses)" bash "$0" --self-test
    stage_end
}

stage_build() {
    stage_begin build "Stage 2: Build"
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

    # ── portable slice ──────────────────────────────────────────────────────
    # #1536 merged green here and red in the pipeline: `engine/tests/unit/mod.rs` gained a module
    # without the `#[cfg(not(feature = "portable"))]` gate its siblings carry, and nothing in this
    # script ever compiled `--features portable`. The run said 120/0 and was true about a
    # *different subject* than the job that failed. The pipeline's two commands are repeated here
    # verbatim, and the target check announces itself when it cannot run.
    if [[ " ${CRATES[*]} " == *" engine "* ]]; then
        subheader "Portable slice"
        run_step "cargo test -p rigorix-engine --features portable --tests" \
            cargo test --locked -p rigorix-engine --features portable --tests
        if rustup target list --installed 2>/dev/null | grep -qx "wasm32-unknown-unknown"; then
            run_step "cargo check -p rigorix-engine --features portable (wasm32)" \
                cargo check --locked -p rigorix-engine --features portable --target wasm32-unknown-unknown
        else
            echo "  ⚠️  wasm32-unknown-unknown not installed — the portable *type* check cannot run here."
            echo "      install: rustup target add wasm32-unknown-unknown   (the slice's tests above still ran)"
        fi
    fi
    stage_end
}

stage_test() {
    stage_begin test "Stage 3: Test"
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
    echo "  ℹ️  the suite is executed a second time by the coverage stage — that second execution"
    echo "      is the measurement (one instrumented run of the same tests), not a re-run."
    stage_end
}

stage_coverage() {
    # `cargo llvm-cov` builds the whole workspace into `target/llvm-cov-target`, a directory that
    # shares nothing with `target/debug`: the cost of this stage is a second full build graph plus
    # one instrumented execution. Its own line in the table is what makes that visible (#1551).
    stage_begin coverage "Stage 4b: Coverage"
    info "this stage builds the workspace into target/llvm-cov-target (a second build graph)"
    run_cmd "coverage (workspace, gated >=${COVERAGE_THRESHOLD:-60}%)" bash .pi/scripts/coverage.sh --gate
    stage_end
}

stage_integration() {
    stage_begin integration "Stage 6: Integration"
    for script in .pi/scripts/validate-integration.sh .pi/scripts/validate-operations.sh; do
        [[ -f "$script" ]] && run_root_script "$(basename "$script")" "$script"
    done
    stage_end
}

# ═════════════════════════════════════════════════════════════════
# Scheduling (#1551)
# ═════════════════════════════════════════════════════════════════
# Waves: the stages that share no readable or writable state first (lint, docs, security, guards),
# then the chain that does (build → test → coverage), then integration. `--stage <name>` runs one.
run_stage() {  # $1 key
    local key="$1" log="$RESULTS_DIR/$1.log" t0=$SECONDS rc=0
    if [[ "$JOBS" -eq 1 ]]; then
        stage_"$key" || rc=$?
    else
        stage_"$key" > "$log" 2>&1 || rc=$?
    fi
    # The stage writes its own start/end stamps; this only fills them in when it could not (a
    # stage that died before `stage_begin` is a stage that took no time, not a negative one).
    local start_s end_s secs
    start_s="$(cat "$RESULTS_DIR/$key.start" 2>/dev/null || echo "$t0")"
    end_s="$(date +%s)"
    printf '%s\n' "$end_s" > "$RESULTS_DIR/$key.end"
    secs=$((end_s - start_s))
    local rows_fail; rows_fail="$(grep -c '^FAIL' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"
    local rows_pass; rows_pass="$(grep -c '^PASS' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"
    if [[ "$rc" -eq 0 ]]; then
        echo -e "  ${GREEN}✅ stage${NC} $key — ${rows_pass:-0} passed ($(fmt_dur "$secs"))"
    else
        echo -e "  ${RED}❌ stage${NC} $key — ${rows_fail:-0} failed ($(fmt_dur "$secs")); transcript: $log"
    fi
}

run_wave() {  # $@ keys
    local -a keys=("$@")
    if [[ "$JOBS" -eq 1 || "${#keys[@]}" -eq 1 ]]; then
        local k
        for k in "${keys[@]}"; do run_stage "$k"; done
        return 0
    fi
    local -a pids=()
    local k i
    for k in "${keys[@]}"; do
        ( run_stage "$k" ) &
        pids+=($!)
    done
    # A ticker, so a long stage is visible while it runs rather than inferred afterwards.
    local alive
    while true; do
        alive=0
        for pid in "${pids[@]}"; do kill -0 "$pid" 2>/dev/null && alive=$((alive + 1)); done
        [[ "$alive" -eq 0 ]] && break
        sleep 10
        for k in "${keys[@]}"; do
            if [[ -f "$RESULTS_DIR/$k.log" ]] && [[ ! -f "$RESULTS_DIR/$k.end" ]]; then
                printf '  … %s: %s\n' "$k" "$(tail -n 1 "$RESULTS_DIR/$k.log" 2>/dev/null | tr -d '\r' | head -c 100)"
            fi
        done
    done
    local rc=0
    for pid in "${pids[@]}"; do wait "$pid" || rc=$?; done
    return 0
}

# What this run promised to do. The summary checks it afterwards: a stage that was scheduled and
# produced no result is not a pass, it is a gate that stopped running (#1551).
SCHEDULED=()
if [[ -n "$STAGE" ]]; then
    SCHEDULED=("$STAGE")
    run_stage "$STAGE"
elif [[ "$JOBS" -eq 1 ]]; then
    # Serial, in the order this runner used before the stages were separable.
    SCHEDULED=("${STAGE_ORDER[@]}")
    for key in "${STAGE_ORDER[@]}"; do run_stage "$key"; done
else
    # Waves. lint/docs/security/guards share no readable or writable state. `build` is deliberately
    # NOT in that wave: it was measured there first, and on a cold tree four concurrent cargo
    # invocations contend on the same `target/` lock and the run took 43m53s instead of speeding
    # up. Cargo already serialises on that directory; adding callers to the queue does not add
    # throughput. What waves buy is the non-cargo stages overlapping, which is what they do.
    SCHEDULED=(lint docs security guards build test coverage integration)
    run_wave lint docs security guards
    # `build` gets its own wave: the portable slice and the wasm32 check live in it, and it was
    # silently absent from this path once already — 111 checks instead of 126, and nothing failed,
    # because the summary only ever described the stages that wrote a result file.
    run_wave build
    run_wave test
    run_wave coverage
    run_wave integration
fi

# ═════════════════════════════════════════════════════════════════
# Summary: a table with every stage's cost and its share (#1551)
# ═════════════════════════════════════════════════════════════════
END_TIME=$(date +%s); DURATION=$((END_TIME - START_TIME))
PASSED=0; FAILED=0; SKIPPED=0
FAILURES=()
for key in "${STAGE_ORDER[@]}"; do
    [[ -f "$RESULTS_DIR/$key.tsv" ]] || continue
    p="$(grep -c '^PASS' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"
    f="$(grep -c '^FAIL' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"
    s="$(grep -c '^SKIP' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"
    PASSED=$((PASSED + ${p:-0})); FAILED=$((FAILED + ${f:-0})); SKIPPED=$((SKIPPED + ${s:-0}))
    while IFS=$'\t' read -r status label _secs detail; do
        [[ "$status" == "FAIL" ]] && FAILURES+=("$label${detail:+ — $detail}")
    done < <(grep '^FAIL' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)
done

# Every scheduled stage must have been started. Absence is the failure mode this check exists for:
# a run that drops a stage reports fewer checks and a green verdict, which is worse than a red one.
SILENT_MISSES=()
while IFS= read -r key; do
    [[ -z "$key" ]] && continue
    SILENT_MISSES+=("$key")
    FAILURES+=("$key — scheduled and produced no result: a stage that does not run cannot fail")
    FAILED=$((FAILED + 1))
done < <(missing_stages "$RESULTS_DIR" "${SCHEDULED[@]+"${SCHEDULED[@]}"}" 2>/dev/null || true)

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════╗"
echo -e "║                     Final Results                        ║"
echo -e "╚══════════════════════════════════════════════════════════╝${NC}"
printf '  %-12s %6s %6s %6s %10s %7s\n' stage steps pass fail duration share
# The denominator is summed in a first pass. Computing it as the table is printed made the first
# stage 100% of itself — the shares read 100, 8, 53, 12 … and summed to 244.
TABLE_TOTAL=0
for key in "${STAGE_ORDER[@]}"; do
    [[ -f "$RESULTS_DIR/$key.end" ]] || continue
    start="$(cat "$RESULTS_DIR/$key.start" 2>/dev/null || echo 0)"
    end="$(cat "$RESULTS_DIR/$key.end" 2>/dev/null || echo 0)"
    TABLE_TOTAL=$((TABLE_TOTAL + end - start))
done
for key in "${STAGE_ORDER[@]}"; do
    [[ -f "$RESULTS_DIR/$key.end" ]] || continue
    start="$(cat "$RESULTS_DIR/$key.start" 2>/dev/null || echo 0)"
    end="$(cat "$RESULTS_DIR/$key.end" 2>/dev/null || echo 0)"
    secs=$((end - start))
    p="$(grep -c '^PASS' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"; p=${p:-0}
    f="$(grep -c '^FAIL' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"; f=${f:-0}
    s="$(grep -c '^SKIP' "$RESULTS_DIR/$key.tsv" 2>/dev/null || true)"; s=${s:-0}
    # Share is of the sum of stage durations, not of the wall clock: when four stages run in one
    # wave their durations overlap, and shares of the wall clock summed to 114%. The number a
    # contributor lives in is the wall clock, so that is printed on its own row beneath — the two
    # are different facts and the table says which is which.
    share=0
    [[ "$TABLE_TOTAL" -gt 0 ]] && share=$((secs * 100 / TABLE_TOTAL))
    printf '  %-12s %6s %6s %6s %10s %6s%%\n' "$key" "$((p + f + s))" "$p" "$f" "$(fmt_dur "$secs")" "$share"
done
echo "  ────────────────────────────────────────────────────────────"
# Two totals, because a run has two costs. `stage time` is the work done and is what the shares are
# of; `wall clock` is what the contributor waited. They are equal in serial mode and differ by the
# overlap when stages run in waves — printing one and calling it both is how 11m31 and 20m both read
# as true. A single `--stage` is not a run, so it prints one row and no share.
printf '  %-12s %6s %6s %6s %10s' "stage time" "$((PASSED + FAILED + SKIPPED))" "$PASSED" "$FAILED" "$(fmt_dur "$TABLE_TOTAL")"
if [[ -z "$STAGE" ]]; then printf ' %6s%%' 100; fi
echo ""
if [[ -z "$STAGE" ]]; then
    printf '  %-12s %6s %6s %6s %10s\n' "wall clock" "" "" "" "$(fmt_dur "$DURATION")"
    if [[ "$TABLE_TOTAL" -gt "$DURATION" ]]; then
        echo "  (the $(fmt_dur $((TABLE_TOTAL - DURATION))) between them is concurrency: stages in one wave overlap)"
    fi
fi
echo ""

# ── the budget (#1551) ───────────────────────────────────────────────────────
# A ceiling is derived from a measured run and recorded with the date and the machine, the way
# `.pi/coverage-layers.tsv` derives its floors. A budget that is nudged to fit the last run is a
# comment; this one fails the run.
BUDGET_FILE=".pi/local-ci-budget.tsv"
OVER_BUDGET=()
if [[ -f "$BUDGET_FILE" ]]; then
    while IFS=$'\t' read -r key _measured ceiling _on _machine; do
        [[ "$key" =~ ^# ]] && continue
        [[ -z "${key:-}" || -z "${ceiling:-}" ]] && continue
        # `total` is the one row that bounds the run rather than a stage: eight stages each inside
        # their ceiling can still be a run nobody would accept, and that is the number a contributor
        # actually experiences. It is skipped for `--stage`, which measures one stage and reports
        # that stage — comparing a 43-second guards run against a run ceiling would be a budget
        # answering a different question than the one asked (#1551).
        if [[ "$key" == "total" ]]; then
            if [[ -n "$STAGE" ]]; then continue; fi
            secs="$DURATION"
        else
            [[ -f "$RESULTS_DIR/$key.end" ]] || continue
            start="$(cat "$RESULTS_DIR/$key.start" 2>/dev/null || echo 0)"
            end="$(cat "$RESULTS_DIR/$key.end" 2>/dev/null || echo 0)"
            secs=$((end - start))
        fi
        if [[ "$secs" -gt "$ceiling" ]]; then
            OVER_BUDGET+=("$key: $(fmt_dur "$secs") > ceiling $(fmt_dur "$ceiling")")
        fi
    done < "$BUDGET_FILE"
fi
if [[ "${#OVER_BUDGET[@]}" -gt 0 ]]; then
    echo -e "${RED}stage(s) over the measured budget ($BUDGET_FILE):${NC}"
    for o in "${OVER_BUDGET[@]}"; do
        echo -e "  ${RED}❌${NC} $o"
        # Where the seconds went, from the stage's own step records: "guards took 2m13s" is a
        # number, "guards took 2m13s and 1m40s of it was `cargo check`" is a finding (#1551).
        stage_key="${o%%:*}"
        if [[ -f "$RESULTS_DIR/$stage_key.tsv" ]]; then
            while IFS=$'\t' read -r secs label; do
                [[ -z "${secs:-}" ]] && continue
                # The label already ends in its own duration (`run_cmd` prints it, `pass` records
                # that string), so printing the label is printing the measurement once.
                echo -e "      ↳ ${label}"
            done < <(awk -F'\t' '$1=="PASS"||$1=="FAIL"{print $3 "\t" $2}' "$RESULTS_DIR/$stage_key.tsv" 2>/dev/null | sort -rn | head -3)
        fi
    done
    echo "  Raise a ceiling with a measurement (and the date and machine), never by nudging it."
    if [[ "$COLD_START" == true ]]; then
        echo "  ℹ this tree had no build cache when the run started: compilation is charged to whichever stage"
        echo "    touches each crate first, so a stage can pass its warm ceiling on a tree that is merely unbuilt."
    fi
    FAILED=$((FAILED + ${#OVER_BUDGET[@]}))
fi

echo ""
if [[ $FAILED -gt 0 ]]; then
    # An empty array under `set -u` is an unbound variable in bash 3.2, and the budget can fail a
    # run with no failing *step* at all — which is exactly when this list is empty.
    if [[ "${#FAILURES[@]}" -gt 0 ]]; then
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
    fi
    echo -e "${RED}❌ CI SIMULATION FAILED — ${FAILED} check(s) failed${NC}"
    echo "  Debug a single stage: bash .pi/scripts/local-ci.sh --stage=<stage>"
    echo "  Debug a single crate: bash .pi/scripts/local-ci.sh --crate=<crate>"
    exit 1
else
    echo -e "${GREEN}✅ CI SIMULATION PASSED — all ${PASSED} checks passed${NC}"
    echo "  Warm-serial equivalent: --jobs 1. Per-stage transcript: $RESULTS_DIR/<stage>.log"
    exit 0
fi