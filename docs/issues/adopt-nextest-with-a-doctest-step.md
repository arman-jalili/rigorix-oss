---
guardian_issue:
  id: "ISSUE-ADOPT-NEXTEST-WITH-A-DOCTEST-STEP"
  title: "Adopt cargo-nextest for the test loops, with an explicit doctest step — per-test timing so the 43 s outlier names itself, process isolation so the thread-count flakiness class dies"
  epic: "portable-slice"
  component: "Tooling"
  module: "wasm-seam"
  status: planned
  priority: low
  tracking_issue: 1373
  dependencies: []

  intent: |
    Two measured facts decide this:

    1. `cargo test -p rigorix-engine` warm is **61 s for 49 suites**, and the slowest single target
       is **43 s** — `reconciliation_policy_integration`, four tests, retry waits. One target is
       **70 % of the wall time** and a suite-level number cannot say which test pays it.
    2. The repo has already been bitten by harness thread counts: `--test-threads=16` was the
       configuration whose flakiness had to be disproved by repetition (`20 runs → 0 failures` in
       the precondition-runner work). A shared-process harness makes that class of defect a
       property of the whole binary rather than of one test.

    `cargo-nextest` gives both: per-test timing, and one process per test, so a test that depends on
    a sibling's global state fails alone and loudly instead of intermittently and in company.

    **What it costs, stated before it is adopted:** nextest runs **no doctests**. `cargo test --doc`
    is a separate step and must be a *required* one with a reported count, or this change silently
    deletes a coverage channel — the same shape as every gate in this repository that was measured
    on the wrong subject.

    **What it does not buy:** a faster cold build. 186 s and 248 crates are compilation, not tests.

  in_scope:
    - "`cargo nextest run` replaces `cargo test` in the fast loop and in the CI test stage; the tool version is pinned the way the toolchain is (a test tool that floats is a test result that floats)"
    - "A separate required `cargo test --doc` step, with its count printed, in the same job — a suite that runs zero doctests fails rather than passing quietly"
    - "Per-test timing is published for the slowest tests, so the 43 s outlier is named rather than inferred from a 61 s suite total"
    - "The 46 engine integration binaries stay as they are; this changes the runner, not the test layout"

  out_of_scope:
    - "**Sharing `CARGO_TARGET_DIR` between the repository root and `wasm/`.** Measured and declined: both workspaces resolve the identical 485 packages, but only the **34 proc-macro crates** and build scripts are compiled for the *host* in both, so the deduplication is tens of seconds on a cold wasm build. The cost is a single `.cargo-lock` shared across two workspace roots — the artifact whose held lock caused the silent multi-minute stall already observed. Not worth the trade, and ADR-011 chose the separate workspace root deliberately."
    - "Reducing the dependency graph or feature-unifying crates. That is a separate, larger decision with its own ADR"
    - "Retrying tests. `retries` exists in nextest and must not be enabled: a retried pass is a normalised comparison"
    - "Touching `reconciliation_policy_integration`'s retry waits"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - ".github/workflows/ci.yml — the test stage runs nextest plus the doctest step"
      - ".pi/scripts — the fast-loop wrapper, if one is added"
    interfaces:
      - "None"

  citations:
    - file: ".github/workflows/ci.yml"
      note: "the `test` job; `preflight_step.sh` budgets apply to whichever runner is used"
    - file: "engine/tests"
      note: "46 integration binaries, each linking the whole engine — the runner change does not alter that"

  verification:
    - "`cargo nextest run -p rigorix-engine` reports the same test count as `cargo test -p rigorix-engine` did minus doctests: 2916 tests, 0 failed, and the count is printed"
    - "`cargo test --doc -p rigorix-engine` is a step in the same job and prints a non-zero count; seeded: an empty doctest selection fails the job rather than passing"
    - "Seed: a deliberately failing test → nextest names that test, its file and line, and no other; restored, the run is green"
    - "Retries are off: `grep -c 'retries' .config/nextest.toml` is 0, or the file does not exist"
    - "Timing: the run prints its slowest tests, and `reconciliation_policy_integration`'s 43 s is attributable to a named test rather than a suite"
    - "A doctest failure fails the job: seeded broken doc example → CI-equivalent command exits non-zero"
---

# The runner, not the layout

| probe | value |
|---|---|
| warm `cargo test -p rigorix-engine` | 61 s · 49 suites · 2916 tests |
| slowest target | 43 s · `reconciliation_policy_integration` · 4 tests |
| cold `cargo test -p rigorix-cli` | 186 s · 248 crates |
| engine, one file changed | 27 s · 1 crate + 46 relinked binaries |

Two-thirds of the warm wall time is one suite that a suite-level number cannot decompose. That is the
argument: the tool's value here is **attribution**, not throughput.

## Why the doctest step is mandatory, not nice

`cargo nextest` does not run doc tests. Adopting it without a separate `cargo test --doc` step would
delete a channel while turning the job green — the same defect as a gate that passes because it never
had a subject. The step must therefore be required, must print a count, and must fail on zero, not
only on failure.

## Why the target-dir idea was measured and dropped

The obvious deduplication is not there. Both workspaces resolve 485 identical packages, but a wasm
build compiles only 34 proc-macro crates plus build scripts for the host; everything else is a
different target triple and lands in its own place regardless of where the directory lives. Tens of
seconds, paid once per clean, against sharing one cargo lock between two roots — and that lock is
already on record as having blocked a build for fifty minutes.
