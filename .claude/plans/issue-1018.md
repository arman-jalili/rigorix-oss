# Issue #1018 — the runnable capacity-ledger demo, then the README section

**Branch:** `issue/1018-capacity-demo`

## Ordering (the issue's own)
1. build the demo (Docker-free, one command) — `engine/examples/capacity_ledger_demo.rs`
   + `demo/capacity-ledger/run.sh` (mirrors the existing `demo/anchor-e2e/` convention)
2. point the README at it — new `## See it run` section with **executed** output
3. close

## Why an example and not a CLI subcommand
The capacity seam has no CLI or server surface (`CapacitySetup::from_env` is the only
composition root), and the in-process adapter's state is per-process — so a
`rigorix capacity …` command would read an empty ledger in a fresh process. The
decision and the observability consequence are recorded in
`docs/runbook-capacity-ledger.md` §"Observing refusals".

## Evidence discipline
Every number in the recorded transcript is printed *after* the operation that
changed it. The README snippet is checked line-by-line against a real run (only the
elision marker `…` is not verbatim). Timing claim measured: 1.4 s warm.

## What building it found
The demo failed one run in three at the commit step → #1090 (retry replaced the
owning guard). Fixed and tested separately in #1092; this branch assumes it.
