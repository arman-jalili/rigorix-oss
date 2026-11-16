#!/usr/bin/env bash
# ============================================================================
# validate-cargo-diagnostics.sh — a cargo invocation must keep its reason,
#                                 and must report what it built (#1599)
# ============================================================================
# Two rules, both learned from one eleven-minute stage that reported "0 crates compiled":
#
#   1. **No cargo invocation discards stderr.** `cargo clippy … 2>/dev/null` throws away the
#      diagnostics that make a failed step readable: the step prints an exit code and no reason. The
#      reason is the whole value of a failing check.
#   2. **Every invocation that builds reports what it built**, through `cargo_compiled`, which prints
#      `cargo <subcommand>: N crate(s) built` from cargo's own `--message-format` stream.
#      `.pi/scripts/local-ci.sh` derives its warm/cold regime from exactly that line, so a wrapper that
#      stops printing it turns every run unmeasured.
#
# The subject is every `languages/rust/validate-ci.sh` copy in the tree (root plus each crate), because
# the budget's counter reads all of them — **and `local-ci.sh`**, which makes cargo invocations of its
# own (#1606). A guard whose subject stops at the wrapper passes while the runner's direct calls stay
# blind, which is exactly what happened until #1606: `build` compiled the portable slice for 1m26s and
# reported nothing, so the stage was permanently `unmeasured` and judged on the looser column.
#
# A capability probe (`cargo clippy --version`) is excluded by name: it builds nothing, so it has no
# reason to preserve — and it is named here rather than silently pattern-matched away. Same for the
# subcommands in `NON_BUILDING_SUBCOMMANDS`, each with the reason it cannot report a count.
#
# Usage:
#   .pi/scripts/validate-cargo-diagnostics.sh
#   .pi/scripts/validate-cargo-diagnostics.sh --self-test
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT" || exit 1

# An invocation, not a mention: the line must *start* with cargo (optionally after `if`/`!`), so a
# comment that quotes one, or a string that names one, is not a false positive. `cargo_compiled` never
# matches, because it is the fix rather than the defect.
COMPILING_RE='^[[:space:]]*(if[[:space:]]+!?[[:space:]]*|![[:space:]]*)?cargo[[:space:]]+(build|test|nextest|clippy|check|rustc|run|bench)([[:space:]]|$)'
# The runner's shape: a cargo invocation is the *command* handed to `run_cmd`/`run_step`, immediately
# after the label. Anchoring on that shape is what keeps a fixture string, a `printf` argument and an
# `echo` message from reading as invocations — three false positives the first version produced on this
# very file (measured: a self-test fixture holding `cargo build -p rigorix-engine`, and the
# `cargo-nextest not installed` warning).
RUNNER_CARGO_RE='^[[:space:]]*run_(cmd|step)[[:space:]]+"[^"]*"[[:space:]]+cargo[[:space:]]+(build|test|nextest|clippy|check|rustc|run|bench)([[:space:]]|$)'
DISCARD_RE='2>[[:space:]]*/dev/null'
# Subcommands that cannot report a count, each with the reason it cannot. They are listed rather than
# pattern-matched away, so "not checked" is a stated fact: an entry here is an admission that the stage
# relying on it is judged on the looser column, and it is the right place to look when a stage sits at
# `unmeasured` forever.
NON_BUILDING_SUBCOMMANDS=' fmt | audit | llvm-cov | nextest '

# ── the checker: one file in, findings out (so --self-test can drive it) ─────
scan_file() { # $1 path → prints one finding per line, exits 1 if there are any
    local path="$1" lineno=0 line findings=0
    while IFS= read -r line; do
        lineno=$((lineno + 1))
        case "$line" in
            *'--version'*) continue ;;   # a capability probe builds nothing
        esac
        if ! printf '%s' "$line" | grep -qE "$COMPILING_RE"; then
            continue
        fi
        if printf '%s' "$line" | grep -qE "$DISCARD_RE"; then
            printf '%s:%s: a cargo invocation discards stderr — a failing step would print an exit code and no reason\n' "$path" "$lineno"
            findings=$((findings + 1))
        fi
        if ! printf '%s' "$line" | grep -q 'cargo_compiled'; then
            printf '%s:%s: a cargo invocation bypasses cargo_compiled — its build is not counted, so the run cannot state its regime\n' "$path" "$lineno"
            findings=$((findings + 1))
        fi
    done < "$path"
    [[ "$findings" -eq 0 ]]
}

# ── the runner: cargo invoked without a count is a stage nobody can condition ─
# `local-ci.sh` may invoke cargo directly, but only through `run_cargo` (counted) or
# `run_cargo_nobuild` (a subcommand that compiles nothing, which prints its own 0). A bare invocation
# passed to `run_cmd`/`run_step` is the defect #1606 filed: it runs, it compiles, and it reports
# nothing. Non-building subcommands and `--version` probes are excluded by name.
scan_runner() { # $1 path → prints one finding per line, exits 1 if there are any
    local path="$1" lineno=0 line findings=0 sub
    while IFS= read -r line; do
        lineno=$((lineno + 1))
        case "$line" in
            \#*) continue ;;            # a comment that quotes one is not an invocation
            *'--version'*) continue ;;   # a capability probe builds nothing
        esac
        if ! printf '%s' "$line" | grep -qE "$RUNNER_CARGO_RE"; then
            continue
        fi
        # Already routed: `run_cargo` / `run_cargo_nobuild` call the shared counter themselves.
        case "$line" in
            *'run_cargo '*|*'run_cargo_nobuild '*) continue ;;
        esac
        # A named non-building subcommand: allowed, and it is the caller's job to keep it in the list.
        sub="$(printf '%s' "$line" | sed -nE 's/.*cargo[[:space:]]+(build|test|nextest|clippy|check|rustc|run|bench).*/\1/p' | head -1)"
        if grep -qE "$NON_BUILDING_SUBCOMMANDS" <<<" $sub "; then
            continue
        fi
        printf '%s:%s: a cargo invocation in the runner bypasses run_cargo — its build is not counted, so its stage stays unmeasured forever\n' "$path" "$lineno"
        findings=$((findings + 1))
    done < "$path"
    [[ "$findings" -eq 0 ]]
}

# ── self-test: plant the very defect this guard exists for ──────────────────
# Run before the scan and exit: a guard whose fixtures only run after the tree passes cannot tell a
# broken guard from a broken tree.
if [[ "${1:-}" == "--self-test" ]]; then
    tmp="$(mktemp -d)"
    trap 'find "${tmp:-/nonexistent}" -mindepth 0 -delete 2>/dev/null || true' EXIT
    passes=0
    fails=0
    expect() { # expect <0|1> <label> <file>
        local want="$1" label="$2" file="$3" got=0
        scan_file "$file" >/dev/null 2>&1 || got=$?
        if [[ "$got" -eq "$want" ]]; then
            echo "  ✅ $label"
            passes=$((passes + 1))
        else
            echo "  ❌ $label (exit $got, wanted $want)"
            fails=$((fails + 1))
        fi
    }
    expect_runner() { # the same, against the runner scan
        local want="$1" label="$2" file="$3" got=0
        scan_runner "$file" >/dev/null 2>&1 || got=$?
        if [[ "$got" -eq "$want" ]]; then
            echo "  ✅ $label"
            passes=$((passes + 1))
        else
            echo "  ❌ $label (exit $got, wanted $want)"
            fails=$((fails + 1))
        fi
    }
    echo "── validate-cargo-diagnostics --self-test ──"
    # The planted defect is the shape the tree actually had: the compile, then `2>/dev/null`.
    printf 'cargo build $SCOPE_FLAG --quiet 2>/dev/null\n' > "$tmp/hidden.sh"
    expect 1 "a compile that discards stderr is caught" "$tmp/hidden.sh"
    printf 'cargo clippy -p thing --all-targets -- -D warnings\n' > "$tmp/bypassed.sh"
    expect 1 "a compile that bypasses cargo_compiled is caught" "$tmp/bypassed.sh"
    printf 'if cargo clippy --version >/dev/null 2>&1; then :; fi\n' > "$tmp/probe.sh"
    expect 0 "a --version capability probe is not a compile" "$tmp/probe.sh"
    printf 'if cargo_compiled clippy $SCOPE_FLAG --all-targets -- -D warnings; then :; fi\n' > "$tmp/counted.sh"
    expect 0 "a counted invocation passes" "$tmp/counted.sh"
    # The runner's shape (#1606): the defect is a cargo invocation handed to `run_cmd`, the fix is
    # `run_cargo`, and a subcommand that compiles nothing is allowed by name rather than by accident.
    printf 'run_cmd "build" cargo build -p rigorix-cli\n' > "$tmp/runner-blind.sh"
    expect_runner 1 "a runner invocation that bypasses run_cargo is caught" "$tmp/runner-blind.sh"
    printf 'run_cargo "cargo build -p rigorix-cli" build -p rigorix-cli\n' > "$tmp/runner-counted.sh"
    expect_runner 0 "a runner invocation through run_cargo passes" "$tmp/runner-counted.sh"
    printf 'run_cargo_nobuild "cargo audit" cargo audit --deny warnings\n' > "$tmp/runner-audit.sh"
    expect_runner 0 "a named non-building subcommand passes" "$tmp/runner-audit.sh"
    printf '# run_cmd "x" cargo build -p y   (a comment quoting one)\n' > "$tmp/runner-comment.sh"
    expect_runner 0 "a comment quoting an invocation is not one" "$tmp/runner-comment.sh"
    if [[ "$fails" -gt 0 ]]; then
        echo "❌ validate-cargo-diagnostics self-test: $fails of $((passes + fails)) case(s) failed"
        exit 1
    fi
    echo "✅ validate-cargo-diagnostics self-test: $passes/$passes"
    exit 0
fi

# ── the subject: the copies the budget actually reads ───────────────────────
# One per mirrored tree, the same six `validate-script-drift.sh` keeps in lockstep, because those are
# the files `local-ci.sh` invokes. `engine/.agents/scripts/languages/rust/validate-ci.sh` also exists and
# is deliberately not a subject: nothing dispatches to it (`.agents/scripts/...` is named only by two old
# design documents), so a finding there would not change a run. It is named here rather than silently
# skipped, and printed below when present, so "not checked" is a stated fact rather than an absence.
TREES=(. engine mcp cli actions wasm)
SUBJECTS=()
for tree in "${TREES[@]}"; do
    candidate="$tree/.pi/scripts/languages/rust/validate-ci.sh"
    candidate="${candidate#./}"
    [[ -f "$candidate" ]] && SUBJECTS+=("$candidate")
done

LEGACY="engine/.agents/scripts/languages/rust/validate-ci.sh"
if [[ -f "$LEGACY" ]]; then
    echo "ℹ  $LEGACY is not a subject: nothing invokes it (the tree's own \`.agents/scripts\` is named only by two old documents), so it cannot change a run's cost. Convert it or delete it in a commit that can test the difference."
fi

if [[ "${#SUBJECTS[@]}" -eq 0 ]]; then
    # A check with no subject is not a pass: 0 files means the glob moved, not that the tree is clean.
    echo "❌ validate-cargo-diagnostics: no languages/rust/validate-ci.sh found — the subject moved, so this guard checked nothing"
    exit 1
fi

failed=0
RUNNERS=()
for tree in "${TREES[@]}"; do
    candidate="$tree/.pi/scripts/local-ci.sh"
    candidate="${candidate#./}"
    [[ -f "$candidate" ]] && RUNNERS+=("$candidate")
done
if [[ "${#RUNNERS[@]}" -eq 0 ]]; then
    echo "❌ validate-cargo-diagnostics: no .pi/scripts/local-ci.sh found — the runner is a subject and it is missing"
    exit 1
fi
for runner in "${RUNNERS[@]}"; do
    if ! out="$(scan_runner "$runner")"; then
        failed=$((failed + 1))
        printf '%s\n' "$out"
    fi
done

for subject in "${SUBJECTS[@]}"; do
    if ! out="$(scan_file "$subject")"; then
        failed=$((failed + 1))
        printf '%s\n' "$out"
    fi
    # The wrapper must still reach the line the budget counts. Since #1606 the line is printed by the
    # shared counter, so the wrapper is checked for sourcing it and the counter for printing it — a
    # wrapper that stops sourcing the counter is blind in exactly the way this guard exists for, one
    # indirection further out.
    if ! grep -q 'cargo-count.sh' "$subject"; then
        echo "❌ $subject does not source languages/rust/cargo-count.sh — its builds report nothing"
        failed=$((failed + 1))
    fi
    counter="$(dirname "$subject")/cargo-count.sh"
    if [[ ! -f "$counter" ]]; then
        echo "❌ $counter is missing — the counter the wrapper and the runner share is not there"
        failed=$((failed + 1))
    elif ! grep -q 'crate(s) built' "$counter"; then
        echo "❌ $counter prints no 'crate(s) built' line — local-ci.sh counts that line and nothing else"
        failed=$((failed + 1))
    fi
done

if [[ "$failed" -gt 0 ]]; then
    echo "❌ validate-cargo-diagnostics: $failed file(s) with a cargo invocation that hides its reason or its work (${#SUBJECTS[@]} checked)"
    exit 1
fi
echo "✅ validate-cargo-diagnostics: ${#SUBJECTS[@]} cargo wrapper(s) and ${#RUNNERS[@]} runner copy(ies) keep their diagnostics and report what they built"