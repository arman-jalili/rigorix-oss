#!/usr/bin/env bash
# ============================================================================
# cargo-count.sh — how many crates a cargo invocation built (#1599, #1606)
# ============================================================================
# Sourced, never executed: `languages/rust/validate-ci.sh` (the per-crate wrapper) and
# `.pi/scripts/local-ci.sh` (the runner, which makes cargo invocations of its own) both count through
# this one function. Two copies of this rule would be two answers to "how many crates did this run
# build?", which is the question the whole warm/cold budget is conditioned on — and the last time the
# counter lived in one place and was grepped from another, a stage that compiled for eleven minutes
# was counted as none (#1599).
#
# The stream is cargo's own, not a line format it happens to print:
#
#     --message-format=json-render-diagnostics
#
# keeps both halves. rustc's diagnostics are rendered for a human on stderr; the machine stream
# carries one `compiler-artifact` message per **unit in the plan**, fresh or not, each tagged
# `"fresh":false` only when it was actually built. Counting the messages answers "how big is this
# crate" — a warm run reports 213 of them with nothing to do (measured) — and counting `fresh:false`
# units answers "how many build units" (52 for one touched library, measured, because a test target is
# a unit of its own). The budget asks **how many crates were compiled**, so the `package_id` of each
# non-fresh artifact is deduplicated: one touched crate is 1, a full rebuild is one per package.
# Everything cargo printed that is not the stream is echoed back into the transcript (for `cargo test`
# that is the harness's own output), and the count is printed as one line:
#
#     cargo build: 2 crate(s) built
#
# `local-ci.sh` reads exactly that line (`COUNTED_RE`), so the number a run reports is cargo's own.
# ---------------------------------------------------------------------------
# A compile is counted from cargo's own stream (#1599)
# ---------------------------------------------------------------------------
# Every cargo call below used to end in `2>/dev/null`. Cargo writes `Compiling …` to stderr, so the
# budget's counter — which greps that line out of this script's transcript — saw **zero** crates for
# the work done here: a stage that compiled for eleven minutes was counted as none, the run was judged
# warm, and the warm ceilings were applied to a cold run. The same redirect threw away the diagnostics
# of a failing step, which is why a failed lint printed an exit code and no reason.
#
# `--message-format=json-render-diagnostics` keeps both halves. rustc's diagnostics are rendered for a
# human on stderr; the machine stream carries one `compiler-artifact` message per **unit in the plan**,
# fresh or not, each tagged `"fresh":false` only when it was actually built. Counting the messages
# would answer "how big is this crate" — a warm run reports 213 of them with nothing to do (measured) —
# and counting `fresh:false` units answers "how many build units" (52 for one touched library, measured,
# because a test target is a unit of its own). The budget asks **how many crates were compiled**, so the
# `package_id` of each non-fresh artifact is deduplicated: one touched crate is 1, a full rebuild is one
# per package. Everything cargo printed that is not the stream is echoed back into the transcript (for
# `cargo test` that is the harness's own output), and the count is printed as one line:
#
#     cargo build: 2 crate(s) built
#
# `local-ci.sh` reads exactly that line, so the number a run reports is cargo's own, not a grep of a
# format cargo never promised to keep.
cargo_compiled() { # cargo_compiled <subcommand> <cargo args...>
    local label="$1"; shift
    local stream status=0 artifacts arg
    # The label is the subcommand, and it stays in `before`: consuming it as a label only was a
    # measured mistake — `cargo -p rigorix-engine --quiet` with no `build`.
    local -a before=("$label") after=()
    local seen_separator=0
    # `--message-format` is a cargo flag, so it must go before the `--` that hands the rest to rustc
    # (`cargo clippy … -- -D warnings` otherwise receives it as a rustc argument and fails).
    for arg in "$@"; do
        if [[ "$seen_separator" -eq 0 && "$arg" == "--" ]]; then
            seen_separator=1
            after+=("--")
            continue
        fi
        if [[ "$seen_separator" -eq 0 ]]; then before+=("$arg"); else after+=("$arg"); fi
    done
    stream="$(mktemp "${TMPDIR:-/tmp}/cargo-stream.XXXXXX")"
    # An empty `after` array is unbound under `set -u` in bash 3.2, so the two shapes are spelled out.
    if [[ ${#after[@]} -gt 0 ]]; then
        cargo "${before[@]}" --message-format=json-render-diagnostics "${after[@]}" >"$stream" 2>&1 || status=$?
    else
        cargo "${before[@]}" --message-format=json-render-diagnostics >"$stream" 2>&1 || status=$?
    fi
    artifacts="$({ grep '"reason":"compiler-artifact"' "$stream" 2>/dev/null \
        | grep '"fresh":false' \
        | sed -E 's/.*"package_id":"([^"]*)".*/\1/' \
        | sort -u; } | wc -l | tr -d ' ' || true)"
    # What is not the machine stream is the reason a human reads this transcript.
    grep -v '^{"reason":' "$stream" || true
    printf '  cargo %s: %s crate(s) built\n' "$label" "${artifacts:-0}"
    rm -f "$stream"
    return "$status"
}
