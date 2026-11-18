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

# The crate counter is the wrapper's, sourced rather than reimplemented (#1606). The runner makes cargo
# invocations of its own — `build` compiles the portable slice, `lint` runs clippy per crate — and until
# this file used the same counter, those builds reported nothing, so the stages that made them were
# permanently `unmeasured` and judged on the looser column. One implementation, two callers.
# shellcheck source=/dev/null
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/languages/rust/cargo-count.sh"
# `run_step` runs its command in a child bash, so the counter must be in that child's environment.
export -f cargo_compiled

QUICK=false; RELEASE=false; STAGE=""; CRATE_FILTER=""; LIST_ONLY=false; SAVE_OUTPUT=false; AFFECTED=false
SELF_TEST=false; PROBE_EXIT=""; LOCK_FILE=""; JOBS=""
START_TIME=$(date +%s)
CRATES=("engine" "mcp" "cli" "actions" "server")
REPORT_DIR="${REPORT_DIR:-.pi/output}"
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
        report_interrupted_run
        return 0
    fi
    local holder tree
    holder="$(cat "$LOCK_FILE/pid" 2>/dev/null || echo 'unknown')"
    tree="$(cat "$LOCK_FILE/tree" 2>/dev/null || echo 'unknown')"
    # A lock whose pid is gone is not a lock (#1567). The previous run was interrupted — Ctrl-C, a
    # closed terminal, a killed process — and refusing the next run leaves the operator to remove the
    # directory by hand from a message that already told them to.
    if [[ "$holder" =~ ^[0-9]+$ ]] && ! kill -0 "$holder" 2>/dev/null; then
        echo -e "${YELLOW}local-ci: reclaiming a lock left by pid $holder, which is gone${NC}" >&2
        rm -f "$LOCK_FILE/pid" "$LOCK_FILE/tree" 2>/dev/null
        rmdir "$LOCK_FILE" 2>/dev/null
        if mkdir "$LOCK_FILE" 2>/dev/null; then
            printf '%s\n' "$$" > "$LOCK_FILE/pid"
            printf '%s\n' "$(pwd)" > "$LOCK_FILE/tree"
            LOCK_HELD=true
            report_interrupted_run
            return 0
        fi
    fi
    echo -e "${RED}local-ci: another run holds the lock${NC}" >&2
    echo "  lock:  $LOCK_FILE" >&2
    echo "  held by pid $holder in $tree" >&2
    echo "  Two runs in one tree build into the same target/ and target/llvm-cov-target/; a test" >&2
    echo "  binary replaced mid-run fails with no test name. Wait for it, or remove the lock if" >&2
    echo "  that pid is gone." >&2
    return 1
}

# Say what an interrupted run left behind (#1567).
#
# A run directory with no `.end` marker for a stage is the fingerprint: the stage started and the run
# never finished it. `target/` is then in a state where the *next* cargo invocation fails with
# "extern location for X does not exist" — a message about a dependency, never about the interruption —
# and recovering costs a full rebuild. Naming it here is the difference between a one-line diagnosis and
# an hour of chasing a crate name.
report_interrupted_run() {
    local stale=""
    for d in "$REPORT_DIR"/.run-*; do
        [[ -d "$d" ]] || continue
        [[ -f "$d/.complete" ]] && continue
        stale="$d"
    done
    [[ -n "$stale" ]] || return 0
    echo -e "${YELLOW}local-ci: $stale is a run that did not finish${NC}" >&2
    echo "  If a build now fails with 'extern location for <crate> does not exist', that is this:" >&2
    echo "  target/ keeps fingerprints for artifacts an interrupted run never wrote, and the fix is" >&2
    echo "  'cargo clean' (a full rebuild) — not 'cargo clean -p <crate>', which reveals the next gap." >&2
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
# Does this stage's record contain a failure? A stage's status is its steps' status, and every stage
# function ended with `stage_end`, whose own last command was `date` — so it always succeeded and a run
# printed "✅ stage lint — 17 passed" beside a FAIL row (measured). The overall verdict sums the tsv and
# was right; the line a reader watches was not.
stage_has_failures() {  # $1 results dir, $2 stage key
    local failed
    failed="$(grep -c '^FAIL' "$1/$2.tsv" 2>/dev/null || true)"
    [[ "${failed:-0}" -gt 0 ]]
}

# ── the budget's measures (#1562) ────────────────────────────────────────────
# Defined here, above the self-test, because the cases pin them and a helper the fixtures cannot call
# is a helper whose fixtures are a copy of it.
#
# There is no run-level crate threshold, and that is the second half of #1612. `COLD_CRATES=100` split
# the run into "warm" (fewer than a hundred crates) and "cold", so a run that built 31 crates was judged
# against a 12m50s ceiling sampled from runs that built nothing, and took 23m10s. The compile is real
# and it is charged to the run: the threshold is **1**, exactly as it is for a stage (#1604), because
# the question every row in the table asks is "did this subject pay a compile?" and one crate is a yes.
# `total` keeps its own row — its subject is the whole run — but it no longer decides that question by a
# different rule from the seven rows beside it.

# A count is a line cargo's own machine stream produced, printed once per cargo invocation by
# `languages/rust/validate-ci.sh`:
#
#     cargo build: 3 crate(s) built
#
# The first version of this counter grepped `Compiling` out of the stage transcript — and cargo writes
# that to stderr, which every cargo call in that wrapper discarded with `2>/dev/null`. A stage that
# compiled for eleven minutes counted zero crates, the run was judged warm, and the warm ceilings were
# applied to a cold run: the failure the two columns exist to prevent, re-entering through the
# instrument that was meant to prevent it (#1599).
#
# Three states, not two. "Compiled nothing" and "no measurement" are different facts, and only one of
# them is a fact: a run where no step reported a count is **unmeasured**, and an unmeasured run is
# judged on the cold column. Failing a run because the instrument was blind is the same defect as the
# one this fixes, pointing the other way.
COUNTED_RE='^ *cargo [a-z]+: [0-9]+ crate\(s\) built$'

compiled_by_stage() {  # $1 results dir, $2 stage key → crates the stage's cargo invocations built
    local total
    total="$({ grep -hE "$COUNTED_RE" "$1/$2.steps.log" 2>/dev/null || true; } \
        | awk '{s += $3} END {print s + 0}')"
    printf '%s' "${total:-0}"
}

stage_counts_reported() {  # $1 results dir, $2 stage key → yes if that stage reported a count at all
    if grep -qE "$COUNTED_RE" "$1/$2.steps.log" 2>/dev/null; then printf 'yes'; else printf 'no'; fi
}

# "Did this stage invoke cargo?" used to be a guess about text — `grep` for `cargo <word>` in the
# transcript — and it could only err loose: the guards stage was called `unmeasured` because one guard
# *printed the words* "cargo wrapper(s)", and `security` because a PASS label quoted `cargo audit`
# (both measured in the run at 671862bc). A stage is now recorded by the tool that ran it: every step
# that invokes cargo writes `cargo-step: <label>` into its own transcript, so the fact is the runner's
# record rather than a reading of prose (#1606). The sniffing is deleted rather than improved, because
# the question it guessed at is now answered where the invocation is made.
CARGO_STEP_RE='^ *cargo-step: '

stage_recorded_cargo() {  # $1 results dir, $2 stage key → yes if a cargo step recorded itself
    if grep -qE "$CARGO_STEP_RE" "$1/$2.steps.log" 2>/dev/null; then printf 'yes'; else printf 'no'; fi
}

run_recorded_cargo() {  # $1 results dir, $2.. stage keys → yes if any stage recorded a cargo step
    local key
    for key in "${@:2}"; do
        if [[ "$(stage_recorded_cargo "$1" "$key")" == "yes" ]]; then printf 'yes'; return; fi
    done
    printf 'no'
}

record_cargo_step() {  # $1 label → the transcript's own record that this step invoked cargo
    printf '  cargo-step: %s\n' "$1" >> "$RESULTS_DIR/$CUR_STAGE.steps.log"
}

counts_reported() {  # $1 results dir, $2.. stage keys → yes if any stage reported a count at all
    local key
    for key in "${@:2}"; do
        if [[ "$(stage_counts_reported "$1" "$key")" == "yes" ]]; then printf 'yes'; return; fi
    done
    printf 'no'
}

# The subject of a stage's ceiling is **the stage**, so the condition it is selected by is that
# stage's own compiled count — not the run's. One regime for the run applied one column to every
# stage, and stages are exactly what differ: in the run at 0976e62f the run was warm (19 crates, cold
# at 100+) while `lint` built all 19 of them and took 8m25s, judged against a 1m48s ceiling sampled
# from runs whose `lint` built nothing. The run was warm; the experiment that ran was not (#1604).
#
# The threshold is 1: the question is "did this subject pay a compile?" and one crate is a yes. `total`
# keeps its own row — its subject is the whole run — but it answers that question by the same rule
# (#1604, unified in #1612).
stage_condition() {  # $1 crates, $2 reported a count (yes|no), $3 recorded a cargo step (yes|no) → nothing|built|unmeasured
    if [[ "${2:-no}" == "yes" ]]; then
        if [[ "${1:-0}" -ge 1 ]]; then printf 'built'; else printf 'nothing'; fi
        return
    fi
    # No count line: either the stage recorded no cargo step (it built nothing, and nothing was
    # there to measure) or it recorded one whose invocation printed no count (a blind instrument).
    # The third state is nearly unreachable now that every invocation reports — it survives because a
    # cargo process can die before printing, and a state deleted because it "can't happen" is how the
    # next blind instrument gets filed as warm.
    if [[ "${3:-yes}" == "no" ]]; then printf 'nothing'; else printf 'unmeasured'; fi
}

stage_column() {  # $1 condition → warm (built nothing) | cold (built something, or could not be measured)
    # The uncounted case takes the looser column: failing a subject for a blind instrument is the
    # original defect mirrored (#1599).
    case "${1:-nothing}" in
        nothing) printf 'warm' ;;
        *) printf 'cold' ;;
    esac
}

stage_budget_cell() {  # $1 results dir, $2 stage key → the column the stage is judged on, and the count that chose it
    local built reported recorded condition
    built="$(compiled_by_stage "$1" "$2")"
    reported="$(stage_counts_reported "$1" "$2")"
    recorded="$(stage_recorded_cargo "$1" "$2")"
    condition="$(stage_condition "$built" "$reported" "$recorded")"
    case "$condition" in
        unmeasured) printf 'cold(?)' ;;
        nothing) printf 'warm(0)' ;;
        *) printf '%s(%s)' "$(stage_column "$condition")" "$built" ;;
    esac
}

budget_verdict() {  # $1 seconds, $2 ceiling → pass|over
    if [[ "${1:-0}" -gt "${2:-0}" ]]; then printf 'over'; else printf 'pass'; fi
}

# A movement inside the recorded noise band is the same code measured twice, not a regression
# (#1612). `.github/ci-budgets.tsv` records the per-stage run-to-run spread as 0.4-1.3 minutes, the
# CI-side gate fails only beyond 1.5 minutes of overage for exactly that reason (#1523), and this
# gate had no floor at all: a `guards` stage that moved 13 s inside its own band failed a green run.
# `band` is a column of `.pi/local-ci-budget.tsv`, not a constant, so a reader can see which tolerance
# each row was judged with — and `validate-budget-derivation.sh` recomputes it.
# Which rows contributed to the run-level verdict (#1618). The total's band is the sum of the stage
# bands, so the honest answer to "why did the run fail?" is the list of rows that moved — a run-level
# row that prints only its own total is a number with no decomposition, and the decomposition is the
# thing that failed.
budget_row_names() {  # $@ entries of the form "name: ..." → comma-separated names
    local out="" entry
    for entry in "$@"; do out="${out:+$out, }${entry%%:*}"; done
    printf '%s' "$out"
}

# The sentence that follows a run-level verdict, naming the rows that produced it. A stage that went
# over its own ceiling is a contributor whatever the run did; if no stage did, the overrun is the sum
# of movements each of which its own row called noise — which is exactly what a run-level band is for.
budget_contributors() {  # reads OVER_BUDGET and BUDGET_WARNED, which hold the rows already judged
    local over="${#OVER_BUDGET[@]}" warned="${#BUDGET_WARNED[@]}"
    if [[ "$over" -gt 0 ]]; then
        printf 'stage(s) over their own ceiling: %s' "$(budget_row_names "${OVER_BUDGET[@]}")"
    elif [[ "$warned" -gt 0 ]]; then
        printf 'every stage was inside its own ceiling; %s stage(s) inside their bands accumulated it: %s' \
            "$warned" "$(budget_row_names "${BUDGET_WARNED[@]}")"
    else
        printf 'no stage moved past its own ceiling'
    fi
}

budget_band() {  # $1 seconds, $2 ceiling, $3 band → pass|warn|over
    if [[ "$(budget_verdict "$1" "$2")" == "pass" ]]; then printf 'pass'; return; fi
    if [[ $((${1:-0} - ${2:-0})) -le "${3:-0}" ]]; then printf 'warn'; else printf 'over'; fi
}

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

    # A lock whose holder is gone is not a lock (#1567): the previous run was interrupted, and
    # making the operator delete the directory by hand — from a message that told them to — is the
    # refusal working against the person it is for.
    mkdir -p "$tmp/dead"
    echo "999999" > "$tmp/dead/pid"
    echo "$tmp" > "$tmp/dead/tree"
    out="$(bash "$0" --lock-file "$tmp/dead" --probe-exit 0 2>&1)" && code=0 || code=$?
    check "a lock whose pid is gone is reclaimed" "$code" "0"
    contains "the reclaim names the dead pid" "$out" "reclaiming a lock left by pid 999999"

    # An interrupted run is named on the next start, because what it leaves behind fails as a
    # missing crate rather than as an interruption.
    mkdir -p "$tmp/results/.run-999999" "$tmp/results/.run-2"
    : > "$tmp/results/.run-999999/lint.start"
    : > "$tmp/results/.run-2/.complete"
    out="$(REPORT_DIR="$tmp/results" bash "$0" --lock-file "$tmp/free2" --probe-exit 0 2>&1)" && code=0 || code=$?
    contains "an unfinished run is named on the next start" "$out" ".run-999999 is a run that did not finish"
    check "a finished run is not named" "$(grep -c '.run-2 is a run' <<<"$out")" "0"

    # A free lock is taken, used, and released.
    out="$(bash "$0" --lock-file "$tmp/free" --probe-exit 0 2>&1)" && code=0 || code=$?
    check "a free lock is taken and the probe exits 0" "$code" "0"
    if [[ -e "$tmp/free" ]]; then
        check "the lock is released on exit" "still there" "gone"
    else
        check "the lock is released on exit" "gone" "gone"
    fi

    # A stage whose step failed is a failed stage — its status is its steps', not `stage_end`'s own
    # last command (measured on a run that printed "✅ stage lint — 17 passed" beside a FAIL row).
    mkdir -p "$tmp/stage"
    printf 'PASS\tsomething fine\t1\t\n' > "$tmp/stage/selftest-stage.tsv"
    stage_has_failures "$tmp/stage" "selftest-stage" && code=0 || code=$?
    check "a stage whose steps all passed has no failure" "$code" "1"
    printf 'FAIL\tsomething broke\t1\t\n' >> "$tmp/stage/selftest-stage.tsv"
    stage_has_failures "$tmp/stage" "selftest-stage" && code=0 || code=$?
    check "a stage with one failed step has a failure" "$code" "0"
    # ── the budget's regime and verdict (#1562) ──
    # The two runs that made the old single column wrong: 11m40s warm (the owner's) and 36m28s cold
    # (a rebuild). Both must pass; a run that compiled nothing and took 36 minutes must not.
    check "a 11m40s warm run is inside the warm ceiling" "$(budget_verdict 700 770)" "pass"
    check "a 36m28s cold run is inside the cold ceiling" "$(budget_verdict 2188 2409)" "pass"
    check "a run that compiled nothing and took 36m is over the warm ceiling" "$(budget_verdict 2188 770)" "over"
    # The run-level row follows the same rule as every stage row: a run that built anything is judged on
    # the built-something column, where before a 100-crate threshold made a 31-crate run "warm" (#1612).
    check "a 31-crate run is judged on the built-something column" \
        "$(stage_column "$(stage_condition 31 yes yes)")" "cold"
    check "a run that built nothing is judged on the nothing-built column" \
        "$(stage_column "$(stage_condition 0 yes yes)")" "warm"
    check "a 23-minute run that built 31 crates passes its cold ceiling" "$(budget_band 1390 2409 394)" "pass"
    # 1390 is past 770 + 394, so #1612's acceptance still holds: a 23-minute run that built nothing is
    # over the warm ceiling by 226 s more than the decomposition's own noise, which is a regression.
    check "the same 23 minutes with nothing built fails the warm ceiling" "$(budget_band 1390 770 394)" "over"
    # ── the run-level row is consistent with its decomposition (#1618) ──
    # `total` carried band 0 while the eight stage rows it bounds carried 394 s between them, so two
    # reachable states failed the run while every stage passed. Both are now one state: inside the band.
    check "every stage exactly at its ceiling (779s) does not fail the run" "$(budget_band 779 770 394)" "warn"
    check "a run inside the accumulated stage noise (1164s) warns" "$(budget_band 1164 770 394)" "warn"
    check "one second past the accumulated noise (1165s) fails" "$(budget_band 1165 770 394)" "over"
    check "a run at its own ceiling passes" "$(budget_band 770 770 394)" "pass"
    # #1599's premise: "compiled nothing" and "measured nothing" are different facts, and the second
    # one is not a fact at all. A run that reported no count is unmeasured and takes the cold column —
    # the looser one, because failing a run for a blind instrument is the original defect mirrored.
    check "a run where nothing reported a count is unmeasured, not warm" \
        "$(stage_condition 0 no yes)" "unmeasured"
    check "an unmeasured subject is judged on the cold column" \
        "$(stage_column "$(stage_condition 0 no yes)")" "cold"
    check "a nothing-built subject is judged on the warm column" \
        "$(stage_column "$(stage_condition 0 yes yes)")" "warm"
    check "a built-something subject is judged on the cold column" \
        "$(stage_column "$(stage_condition 9 yes yes)")" "cold"
    # ── a stage's ceiling is selected by that stage's own count (#1604) ──
    # The number that chose the column is the stage's, not the run's: `lint` building 19 crates in a
    # warm run is a stage that paid a compile, and the ceiling that describes it is the other one.
    check "a stage that built nothing is judged on the nothing-built column" \
        "$(stage_column "$(stage_condition 0 yes yes)")" "warm"
    check "a stage that built 3 crates is judged on the built-something column" \
        "$(stage_column "$(stage_condition 3 yes yes)")" "cold"
    check "an unmeasured stage is judged on the looser column" \
        "$(stage_column "$(stage_condition 0 no yes)")" "cold"
    # A stage that ran no cargo built nothing and had nothing to measure: `docs` and `integration` log
    # zero cargo invocations, so `unmeasured` would be a claim about an instrument that never ran.
    check "a stage that ran no cargo is nothing-built, not unmeasured" \
        "$(stage_condition 0 no no)" "nothing"
    # The case that used to be impossible to express: one run, two different columns, both true. The
    # run is warm because it built 19 crates; `lint` is not, because those seconds are lint's.
    check "a run that built nothing can contain a stage judged on the cold column" \
        "$(stage_condition 0 yes yes)/$(stage_column "$(stage_condition 19 yes yes)")" "nothing/cold"
    check "a stage that builds nothing for eleven minutes still fails" "$(budget_verdict 660 108)" "over"
    # ── the noise band (#1612) ──
    # The two movements that failed a green run at 23a78b2d, with the ceilings they were judged
    # against: `test` +29 s, `guards` +13 s. Both are inside the 78 s the project measured, so both
    # warn instead of failing.
    check "a 29 s movement inside the band warns" "$(budget_band 156 127 78)" "warn"
    check "a 13 s movement inside the band warns" "$(budget_band 96 83 78)" "warn"
    check "a stage inside its ceiling passes" "$(budget_band 120 127 78)" "pass"
    check "a movement beyond the band fails" "$(budget_band 900 127 78)" "over"
    check "a movement exactly at the band warns" "$(budget_band 205 127 78)" "warn"
    check "one second past the band fails" "$(budget_band 206 127 78)" "over"
    # A short row cannot borrow the full band: a 1-second stage that takes two minutes is not noise.
    check "a 1-second ceiling with a 1-second band fails at 2 minutes" "$(budget_band 120 1 1)" "over"
    # The cold run that measured the sample: `guards` compiled 38 crates inside its wave and took
    # 3m21s, which the old 112s single ceiling failed and the derived cold ceiling passes. This is the
    # regression the two columns exist for, pinned with the numbers it actually produced.
    check "a cold guards run of 3m21s is inside the cold ceiling" "$(budget_verdict 201 221)" "pass"
    check "the same 3m21s against the warm ceiling is a failure" "$(budget_verdict 201 84)" "over"
    check "a 27m12s cold run is inside the cold total" "$(budget_verdict 1632 2409)" "pass"
    # The counter reads what cargo reported, not a line format it happens to print: a transcript with
    # no `Compiling` line in it at all still counts the crates its cargo invocations built (#1599).
    mkdir -p "$tmp/compiled"
    printf '  cargo build: 3 crate(s) built\n  cargo test: 4 crate(s) built\n' > "$tmp/compiled/build.steps.log"
    printf '  cargo build: 0 crate(s) built\n' > "$tmp/compiled/test.steps.log"
    check "the counter sums the crates a stage's invocations reported" "$(compiled_by_stage "$tmp/compiled" build)" "7"
    check "a stage whose transcript has no Compiling line still counts its compile" \
        "$(grep -c 'Compiling' "$tmp/compiled/build.steps.log" || true)" "0"
    check "a stage that built nothing counts zero" "$(compiled_by_stage "$tmp/compiled" test)" "0"
    check "a stage that reported counts is measured" "$(counts_reported "$tmp/compiled" build test)" "yes"
    mkdir -p "$tmp/quiet"
    # A recorded invocation whose output never carried a count: the cargo process died before printing.
    printf '  cargo-step: cargo build -p rigorix-engine\nPASS\tbuild rigorix-engine (11s)\t11\t\n' \
        > "$tmp/quiet/build.steps.log"
    printf 'error[E0425]: cannot find value `nope` in this scope\n' >> "$tmp/quiet/build.steps.log"
    check "a stage that reported no count is not measured" "$(counts_reported "$tmp/quiet" build)" "no"
    # The per-stage form of the same two facts, and the cell the summary table prints: the count is the
    # stage's own, and a stage with no count says so instead of claiming zero (#1604).
    check "a stage's own count selects its cell" "$(stage_budget_cell "$tmp/compiled" build)" "cold(7)"
    check "a stage that built nothing has the nothing-built cell" "$(stage_budget_cell "$tmp/compiled" test)" "warm(0)"
    check "a stage that reported no count says so in its cell" "$(stage_budget_cell "$tmp/quiet" build)" "cold(?)"
    mkdir -p "$tmp/nocargo"
    printf 'PASS\tvalidate-docs.sh (2s)\t2\t\n' > "$tmp/nocargo/docs.steps.log"
    check "a stage that ran no cargo is measured as nothing built" "$(stage_budget_cell "$tmp/nocargo" docs)" "warm(0)"
    printf 'PASS\tcargo build -p rigorix-server (16s)\t16\t\n' > "$tmp/quiet/server.steps.log"
    # #1606: the record is the runner's, not a reading of the transcript's prose. A stage that merely
    # *mentions* cargo — a guard printing "cargo wrapper(s)", a PASS label quoting `cargo audit`, both of
    # which really happened — is not a stage that invoked it.
    check "a transcript that mentions cargo but records no step did not invoke it" \
        "$(stage_recorded_cargo "$tmp/quiet" server)" "no"
    check "a mentioned cargo with no count is nothing-built, not unmeasured" \
        "$(stage_budget_cell "$tmp/quiet" server)" "warm(0)"
    printf '  cargo-step: coverage via coverage.sh\n' >> "$tmp/quiet/server.steps.log"
    check "a recorded step with no count line is unmeasured" \
        "$(stage_budget_cell "$tmp/quiet" server)" "cold(?)"
    printf '  cargo build: 5 crate(s) built\n' >> "$tmp/quiet/server.steps.log"
    check "a recorded step that reports is measured" \
        "$(stage_budget_cell "$tmp/quiet" server)" "cold(5)"


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
    # The stage's status is its steps' status (measured, not assumed): every stage function ended with
    # this call, so its exit code was always 0 and a run printed `✅ stage lint — 17 passed` beside a
    # `FAIL validate-ci.sh (engine)` row. The overall verdict was right — it sums the tsv — but the line
    # a reader watches was not, and a stage that reports green while a step failed is the reporting
    # defect the run-level fixes were about.
    ! stage_has_failures "$RESULTS_DIR" "$CUR_STAGE"
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

# A cargo step the budget can see (#1606). It records the invocation, then runs it through the same
# counter the per-crate wrapper uses, so the count lands in this stage's transcript and the stage's
# ceiling is selected by what it actually built. `run_step` backgrounds `bash -c`, which is why
# `cargo_compiled` is exported above.
run_cargo() { # $1 label, then the cargo subcommand and its arguments — no leading `cargo`
    local label="$1"; shift
    record_cargo_step "cargo $*"
    run_step "$label" bash -c 'cargo_compiled "$@"' _ "$@"
}

# A cargo subcommand that compiles nothing by definition — `cargo audit` reads the dependency graph.
# Its count is 0 because there is no build, not because a measurement failed, so it prints the line and
# the stage stays measured rather than being filed as blind. Named here rather than pattern-matched
# away, and named again in `validate-cargo-diagnostics.sh`'s exclusion list.
run_cargo_nobuild() { # $1 label, then the cargo subcommand and its arguments — no leading `cargo`
    local label="$1"; shift
    record_cargo_step "cargo $*"
    printf '  cargo %s: 0 crate(s) built\n' "$1" >> "$RESULTS_DIR/$CUR_STAGE.steps.log"
    run_step "$label" cargo "$@"
}

run_script() {
    local label="$1" crate="$2" script_abs="$3" extra="${4:-}"
    # script_abs is e.g. "engine/.pi/scripts/ci/stage_lint.sh"
    # We strip the crate prefix so it runs from the crate dir
    local script_rel="${script_abs#$crate/}"
    local fn
    fn="$(mktemp -d)/run"
    {
        echo '#!/usr/bin/env bash'
        printf 'cd %q || exit 1\n' "$crate"
        printf 'exec bash %q %s\n' "$script_rel" "$extra"
    } > "$fn"
    chmod +x "$fn"
    run_step "$label" "$fn"
}

run_root_script() {
    local label="$1" script="$2" extra="${3:-}"
    run_script "$label" "." "$script" "$extra"
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
        # `cargo fmt --check` builds nothing and has no count; it is excluded by name here and in
        # `validate-cargo-diagnostics.sh` (#1606).
        run_cmd "cargo fmt --check -p rigorix-$crate" cargo fmt --check -p "rigorix-$crate"
        run_cargo "cargo clippy -p rigorix-$crate -- -D warnings" clippy -p "rigorix-$crate" -- -D warnings
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
        run_cargo_nobuild "cargo audit" audit --deny warnings
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
            run_cargo "cargo check -p rigorix-$crate (quick)" check -p "rigorix-$crate"
        elif [[ "$RELEASE" == "true" ]]; then
            run_cargo "cargo build -p rigorix-$crate --release" build -p "rigorix-$crate" --release
        else
            run_cargo "cargo build -p rigorix-$crate" build -p "rigorix-$crate"
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
        # #1626: this step ran the pipeline's `cargo check … --target wasm32-unknown-unknown` and
        # was still a different subject, because the pipeline sets `RUSTFLAGS: -D warnings` for the
        # whole job and this copy did not. A `pub(crate)` producer whose only caller is native-only
        # is dead code in the slice — a *warning* — so the local run said green and both CI jobs
        # (`portable`, `wasm gates`) said red. The invocation now lives in one script that carries
        # the warning policy and the inventory check with it, so the flag cannot be forgotten here.
        # The portable tests are the script's subject too (#1641), so they are switched on here
        # rather than restated: one body, and the flags travel with the command.
        run_root_script "validate-portable-slice.sh (wasm32 + inventory + portable tests)" \
            ".pi/scripts/validate-portable-slice.sh" "--with-tests"
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
        # `nextest` is outside cargo's own stream — it is a subcommand with its own reporter, and
        # handing it `--message-format` would count nextest's plan rather than this run's builds — so
        # it is excluded by name in `validate-cargo-diagnostics.sh`. This stage is measured anyway, by
        # the `validate-ci.sh` steps below; the `cargo test` fallback is the counted path (#1606).
        if command -v cargo-nextest >/dev/null 2>&1; then
            run_cmd "cargo nextest run -p rigorix-$crate" cargo nextest run -p "rigorix-$crate"
        else
            echo "  ⚠️  cargo-nextest not installed — falling back to cargo test (https://get.nexte.st)"
            run_cargo "cargo test -p rigorix-$crate" test -p "rigorix-$crate"
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
    # `cargo llvm-cov` is invoked inside `coverage.sh`, not here, and it has no `--message-format`: the
    # compile this stage pays for is real and cannot be counted from this file. Recording the step
    # keeps the stage honestly `unmeasured` (the looser column) instead of claiming it built nothing —
    # a marker is the runner saying "there is cargo work here I cannot see", which is different from
    # silence and different from a count (#1606).
    record_cargo_step "coverage via coverage.sh (cargo llvm-cov inside that script)"
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
    # The transcript is written in both modes: it is where the runner reads how many crates this stage
    # compiled, and "how many crates did this run compile" is the fact the regime is derived from
    # (#1562). A serial run that kept its transcript only on a tty could not answer it.
    if [[ "$JOBS" -eq 1 ]]; then
        stage_"$key" 2>&1 | tee "$log" || rc=$?
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
printf '  %-12s %6s %6s %6s %10s %7s %9s %12s\n' stage steps pass fail duration share compiled budget
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
    # `compiled` is the fact the budget's regime is derived from, printed per stage so a reader can
    # see which stage paid for the compile instead of inferring it from a duration (#1562).
    # `budget` is the column that stage's ceiling is selected from and the count that selected it:
    # `warm(0)` built nothing, `cold(19)` built nineteen, `cold(?)` could not be measured (#1604).
    printf '  %-12s %6s %6s %6s %10s %6s%% %8s %12s\n' "$key" "$((p + f + s))" "$p" "$f" "$(fmt_dur "$secs")" "$share" "$(compiled_by_stage "$RESULTS_DIR" "$key")" "$(stage_budget_cell "$RESULTS_DIR" "$key")"
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

# ── the budget (#1551, re-derived for #1562/#1568, conditioned per stage in #1604) ─────────────
# A ceiling is derived from measured runs and recorded with its sample size and the machine, the way
# `.pi/coverage-layers.tsv` derives its floors. Two things make this a gate rather than a comment:
#
#   1. The condition is measured, not assumed — and measured **per stage**. A stage that compiled the
#      workspace and a stage that compiled nothing are different experiments: the 36-minute rebuild the
#      old single column called a regression was that row comparing a warm number to a cold run, and the
#      8m25s `lint` of 2026-11-16 was `lint` paying for 19 crates inside a run the run-level regime called
#      warm. One regime for the run decided both, and the subject of every stage ceiling is a stage
#      (#1604). So each stage selects its column from its own count, `total` keeps the run's regime — it
#      is the one row whose subject really is the whole run — and both are printed, because a reader
#      should never have to infer which experiment a number belongs to.
#   2. A ceiling carries its sample: `runs`, `slowest`, and `compiled`, the count the sample's stages
#      reported. "4m > 2m" is an argument, "4m > 2m (cold sample: 6 runs, slowest 1m15s, sample compiled
#      unmeasured)" is a measurement with a provenance, and a ceiling raised to fit one run cannot be
#      written without the run count disagreeing with it.
BUDGET_FILE=".pi/local-ci-budget.tsv"
OVER_BUDGET=()
BUDGET_WARNED=()

COMPILED_TOTAL=0
for _k in "${STAGE_ORDER[@]}"; do
    [[ -f "$RESULTS_DIR/$_k.tsv" ]] || continue
    COMPILED_TOTAL=$((COMPILED_TOTAL + $(compiled_by_stage "$RESULTS_DIR" "$_k")))
done
COUNTS_REPORTED="no"
RUN_RECORDED="no"
if [[ "${#STAGE_ORDER[@]}" -gt 0 ]]; then
    COUNTS_REPORTED="$(counts_reported "$RESULTS_DIR" "${STAGE_ORDER[@]}")"
    RUN_RECORDED="$(run_recorded_cargo "$RESULTS_DIR" "${STAGE_ORDER[@]}")"
fi
# `total` is conditioned on the run's own count, by the rule every stage row follows (#1612).
TOTAL_CONDITION="$(stage_condition "$COMPILED_TOTAL" "$COUNTS_REPORTED" "$RUN_RECORDED")"
TOTAL_COLUMN="$(stage_column "$TOTAL_CONDITION")"
TOTAL_COMPILED_NOTE="$COMPILED_TOTAL crate(s) built, so the $TOTAL_COLUMN column"

if [[ -f "$BUDGET_FILE" ]]; then
    while IFS=$'\t' read -r key warm_ceiling warm_runs warm_slowest warm_compiled cold_ceiling cold_runs cold_slowest cold_compiled band _on _machine; do
        [[ "$key" == "stage" ]] && continue
        [[ "$key" =~ ^# ]] && continue
        [[ -z "${key:-}" || -z "${warm_ceiling:-}" ]] && continue
        # Which column this row is judged on, and the count that chose it. The subject of a stage's
        # ceiling is the stage, so the count is the stage's; `total` is the one row whose subject is
        # the run, so it keeps the run-level regime computed above (#1604).
        #
        # `total` is skipped for `--stage`, which measures one stage and reports that stage —
        # comparing a 43-second guards run against a run ceiling would be a budget answering a
        # different question than the one asked (#1551).
        if [[ "$key" == "total" ]]; then
            if [[ -n "$STAGE" ]]; then continue; fi
            secs="$DURATION"
            row_column="$TOTAL_COLUMN"; condition="$TOTAL_CONDITION"; built="$COMPILED_TOTAL"
        else
            [[ -f "$RESULTS_DIR/$key.end" ]] || continue
            start="$(cat "$RESULTS_DIR/$key.start" 2>/dev/null || echo 0)"
            end="$(cat "$RESULTS_DIR/$key.end" 2>/dev/null || echo 0)"
            secs=$((end - start))
            built="$(compiled_by_stage "$RESULTS_DIR" "$key")"
            condition="$(stage_condition "$built" "$(stage_counts_reported "$RESULTS_DIR" "$key")" "$(stage_recorded_cargo "$RESULTS_DIR" "$key")")"
            row_column="$(stage_column "$condition")"
        fi
        if [[ "$row_column" == "cold" ]]; then
            ceiling="$cold_ceiling"; runs="$cold_runs"; slowest="$cold_slowest"; sample_compiled="$cold_compiled"
        else
            ceiling="$warm_ceiling"; runs="$warm_runs"; slowest="$warm_slowest"; sample_compiled="$warm_compiled"
        fi
        [[ -z "${ceiling:-}" ]] && continue
        # Inside the band a movement is the same code measured twice: warn, print the band it was
        # compared against, and do not fail the run (#1612).
        case "$(budget_band "$secs" "$ceiling" "${band:-0}")" in
            over)
                if [[ "$key" == "total" ]]; then
                    OVER_BUDGET+=("$key: $(fmt_dur "$secs") > $row_column ceiling $(fmt_dur "$ceiling") (+$(fmt_dur $((secs - ceiling))), beyond the $(fmt_dur "${band:-0}") band — the sum of the stage bands) — $(budget_contributors) — $key built ${built} crate(s) ($condition), so the $row_column column applies; the $row_column sample is ${runs:-?} run(s), slowest $(fmt_dur "${slowest:-0}"), sample compiled ${sample_compiled:-unmeasured}")
                else
                    OVER_BUDGET+=("$key: $(fmt_dur "$secs") > $row_column ceiling $(fmt_dur "$ceiling") (+$(fmt_dur $((secs - ceiling))), beyond the $(fmt_dur "${band:-0}") band) — $key built ${built} crate(s) ($condition), so the $row_column column applies; the $row_column sample is ${runs:-?} run(s), slowest $(fmt_dur "${slowest:-0}"), sample compiled ${sample_compiled:-unmeasured}")
                fi
                ;;
            warn)
                if [[ "$key" == "total" ]]; then
                    BUDGET_WARNED+=("$key: $(fmt_dur "$secs") > $row_column ceiling $(fmt_dur "$ceiling") (+$(fmt_dur $((secs - ceiling)))) — inside the $(fmt_dur "${band:-0}") band, which is the sum of the stage bands: the run is inside the noise its own rows accumulated, so this is the same code measured twice (#1618)")
                else
                    BUDGET_WARNED+=("$key: $(fmt_dur "$secs") > $row_column ceiling $(fmt_dur "$ceiling") (+$(fmt_dur $((secs - ceiling)))) — inside the $(fmt_dur "${band:-0}") measured band for this row, so this is the same code measured twice, not a regression")
                fi
                ;;
        esac
    done < "$BUDGET_FILE"
fi

echo -e "  ${BLUE}ℹ${NC} this run built $COMPILED_TOTAL crate(s) — the total row is judged on the $TOTAL_COLUMN column ($TOTAL_CONDITION), the same rule every stage above follows"
echo -e "  ${BLUE}ℹ${NC} a stage's ceiling is selected by that stage's own compiled count — 0 crates: the warm (nothing-built) column, 1+ or unmeasured: the cold (built-something) column — printed per stage above as \`budget\`"

if [[ "${#BUDGET_WARNED[@]}" -gt 0 ]]; then
    echo -e "${YELLOW}row(s) outside their ceiling but inside the measured noise band ($BUDGET_FILE):${NC}"
    for w in "${BUDGET_WARNED[@]}"; do
        echo -e "  ${YELLOW}⚠${NC}  $w"
    done
    echo "  The band is the per-stage run-to-run spread this project measured (0.4-1.3 min, recorded in"
    echo "  .github/ci-budgets.tsv): failing inside it reports the measurement, not the code (#1612)."
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
        echo -e "      ↳ built $(compiled_by_stage "$RESULTS_DIR" "$stage_key") crate(s) in this stage"
    done
    echo "  The total row ($TOTAL_COMPILED_NOTE) governs the run; every stage above was judged on its own"
    echo "  compiled count, and every row uses the same rule: 0 crates built is the nothing-built column, one"
    echo "  or more is the built-something column, and an unmeasured subject takes the looser one. A slow run"
    echo "  cannot earn the cold ceiling by being slow (#1612)."
    echo "  Raise a ceiling with a measurement of its own column, the date, the machine and the sample size."
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
    # A finished run, whatever its verdict: the marker is what distinguishes "this run failed" from
    # "this run never returned" on the next start (#1567).
    : > "$RESULTS_DIR/.complete" 2>/dev/null || true
    echo -e "${RED}❌ CI SIMULATION FAILED — ${FAILED} check(s) failed${NC}"
    echo "  Debug a single stage: bash .pi/scripts/local-ci.sh --stage=<stage>"
    echo "  Debug a single crate: bash .pi/scripts/local-ci.sh --crate=<crate>"
    exit 1
else
    : > "$RESULTS_DIR/.complete" 2>/dev/null || true
    echo -e "${GREEN}✅ CI SIMULATION PASSED — all ${PASSED} checks passed${NC}"
    echo "  Warm-serial equivalent: --jobs 1. Per-stage transcript: $RESULTS_DIR/<stage>.log"
    exit 0
fi