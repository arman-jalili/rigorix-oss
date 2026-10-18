---
guardian_issue:
  id: "ISSUE-PRECONDITION-RUNNER-TESTS-FLAKY-UNDER-PARALLELISM"
  title: "tests: three precondition runner tests spawn /bin/sh on a wall-clock budget and time out under full-suite parallelism"
  epic: "test reliability"
  component: "PreconditionRunner"
  module: "precondition"
  status: planned
  priority: medium
  dependencies: []

  intent: |
    Make the precondition runner tests deterministic. Three of them spawn a real `/bin/sh`
    and assert on wall-clock behaviour inside a suite of ~2200 tests running in parallel. They
    pass in isolation and intermittently fail in the full run.

    This is not a product defect. It is a defect in the thing a green CI run means: a suite that
    fails ~sometimes teaches everyone to re-run until green, and that habit is what lets a real
    regression through later. It also cannot distinguish "the runner is broken" from "the machine
    was busy", which is the only question CI exists to answer.

  in_scope:
    - "Identify the three affected tests and the exact failure mode (timeout vs spawn failure)"
    - "Remove dependence on wall-clock scheduling where possible (deterministic fake runner)"
    - "Where a real process is genuinely required, raise the budget and/or isolate the test"
    - "Prove determinism: the affected tests run repeatedly under full-suite parallelism without flaking"
    - "Record why the flake was tolerated before, if it was"

  out_of_scope:
    - "Changing runner behaviour (only its tests)"
    - "The rest of the suite's parallelism"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: tests only — no production code"
    api:
      - "None"

  canonical_references:
    - evidence: "engine/tests/unit/precondition/preconditionrunner/preconditionrunner_test.rs:135 — timeout_and_spawn_failure_refuse_never_pass spawns `sleep 5` with timeout_ms = 100"
    - evidence: "same file:80 argv_exit_codes_are_mapped, :103 json_stdin_and_env_are_provided — both spawn /bin/sh"
    - evidence: "same file:15 — the precondition() helper sets no timeout_ms, so the domain default applies"
    - evidence: "engine/src/precondition/infrastructure/runner.rs:558,575 — tokio::time::timeout around child.wait()"

  acceptance_criteria:
    - "The three tests no longer fail in a full-suite parallel run"
    - "Each test's failure mode is deterministic when the behaviour it asserts is broken (verified by fault injection)"
    - "A fault-injected runner change makes the tests fail; the current flake is not masking an assertion that never runs"
    - "The suite passes N consecutive full runs (N recorded in the PR)"

  validators:
    - ci
    - tests

  implementation_notes: |
    - The signal to preserve is the fail-closed behaviour: timeout must map to
      PreconditionError::Timeout and must not be retriable. Keep that assertion; change how the
      time passes, not what is asserted.
    - `timeout_and_spawn_failure_refuse_never_pass` is trying to assert two things at once
      (timeout mapping AND spawn-failure mapping). Splitting it removes the timing coupling from
      half of it.
    - Under parallel load the likely mis-behaviour is not a late timeout but a *spawn* failure
      (fork/EAGAIN), which yields a different error variant — so the assertion can fail for a
      reason that has nothing to do with the property under test. Confirm which it is before
      choosing the fix; the two need different remedies.
    - Prefer a deterministic injected clock or a fake child process where the runner allows it.
      A generous timeout alone hides the flake rather than removing it.

  file_changes:
    - "modify: engine/tests/unit/precondition/preconditionrunner/preconditionrunner_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-PRECONDITION-RUNNER-TESTS-FLAKY-UNDER-PARALLELISM

## The flake

Three tests in `preconditionrunner_test.rs` spawn a real `/bin/sh`:

| line | test |
|---|---|
| 80 | `argv_exit_codes_are_mapped` |
| 103 | `json_stdin_and_env_are_provided` |
| 135 | `timeout_and_spawn_failure_refuse_never_pass` |

The last one is the clearest:

```rust
let mut slow = precondition("p1", vec!["/bin/sh", "-c", "sleep 5"]);
slow.timeout_ms = 100;
let timeout = runner().run(&slow, &input()).await.expect_err("timeout");
assert!(matches!(timeout, PreconditionError::Timeout { .. }));
```

A 100 ms wall-clock budget competing with ~2200 other tests for CPU. The helper
`precondition()` (`:15`) sets no `timeout_ms`, so the others use the domain default.

## Why it matters more than it looks

CI's only job is to answer *"is the code still correct?"*. A test that intermittently fails for
scheduling reasons makes that answer unreadable, and the workaround everyone adopts — re-run until
green — is exactly the habit that lets a real failure through later.

There is a second hazard specific to this shape: under load the failure may not be a late timeout
at all but a **spawn failure** (`fork`/`EAGAIN`), which produces a *different* error variant. The
assertion would then fail for a reason unrelated to the property under test. So the flake may also
be masking whether the assertion is meaningful at all — worth fault-injecting to confirm the test
can still catch the regression it exists for.

## Why it was filed here rather than left in the PR

It was reported in PR #1077's description, which is the right instinct — not papering over it —
but a PR description is read once. An untracked flake is indistinguishable from no flake the
moment the PR is merged.

## Acceptance

See front matter. Load-bearing: **fault injection must still make the test fail.** Removing the
flake by loosening the assertion would be worse than the flake.
