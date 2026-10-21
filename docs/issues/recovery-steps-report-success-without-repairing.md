---
guardian_issue:
  id: "ISSUE-RECOVERY-STEPS-REPORT-SUCCESS-WITHOUT-REPAIRING"
  title: "recovery: execute_step returns Ok for five repairs without performing them — the seam is wired, reports success, and repairs nothing"
  epic: "execution engine"
  component: "Recovery"
  module: "recovery-recipes"
  status: planned
  priority: high
  dependencies:
    - "#1111 — the recovery seam is now reachable (opt-in via RIGORIX_RECOVERY)"

  intent: |
    With recovery armed, the engine attempts a recipe on failure. RecoveryServiceImpl::execute_step
    returns Ok(()) for CleanBuild, ExpandContext, RebaseBranch, AcceptTrust and RestartWorker without
    doing any of them. The recipe then proceeds as if the repair succeeded, and the retry runs
    against an unrepaired state.

    The seam being wired makes this reachable: before #1111 a user could not enable it. Now they can,
    and the reported outcome is wrong in the most expensive direction — a repair that claims success.

  in_scope:
    - "Either execute each step for real, or fail loudly for the ones that are not implemented"
    - "Never report success for a step that did not run"
    - "Make the distinction visible in the attempt outcome so a retry is not attempted on a false premise"

  out_of_scope:
    - "Designing new recipes"
    - "The recovery decision policy (when to attempt, which recipe) — accepted"
    - "RebaseBranch semantics beyond reporting an honest failure"

  affected_layers:
    domain:
      - "recovery_recipes/domain — RecoveryStep, attempt outcome"
    application:
      - "recovery_recipes/application/service_impl.rs:69 execute_step"
    infrastructure:
      - "the tool/subprocess layer each step would use"
    api:
      - "None"

  canonical_references:
    - evidence: "recovery_recipes/application/service_impl.rs:69-78 — 'Safe steps — simulated as successful' then Ok(()) for CleanBuild | ExpandContext | RebaseBranch | AcceptTrust | RestartWorker"
    - evidence: "the same file's doc comment: 'In a production implementation, this would actually execute the step ... For now, returns success for steps that don't require external execution'"
    - evidence: "engine/.pi/architecture/modules/recovery-recipes.md:22 — 'Execution itself is simulated today (RecoveryServiceImpl::execute_step returns ...'"
    - evidence: "the only non-simulated branch validates RetryConnection's timeout > 0 and then returns Ok"
    - evidence: "#1111 made the seam reachable: recovery_service is now attached from cli, mcp and actions"

  acceptance_criteria:
    - "No step returns success unless it performed its action"
    - "Unimplemented steps return an explicit 'unsupported' outcome that the recipe surfaces, rather than Ok"
    - "The attempt outcome distinguishes repaired / unsupported / failed, and only 'repaired' permits the retry"
    - "A test asserts that a simulated step is not reported as a repair"
    - "The module doc's 'simulated' admission is either removed (because it runs) or reworded to name which steps are unsupported"

  validators:
    - tests
    - integration

  implementation_notes: |
    The minimal correct change is to make the honesty explicit: return an outcome enum instead of
    Result<(), String>, where the success variant means "performed". That converts five silent lies
    into five loud unsupported results, which is a one-line-per-step change and immediately makes
    the feature's real coverage visible.

    Do not implement a half-repair to make the test pass. CleanBuild that runs `cargo clean` without
    the project's build command, or RebaseBranch that runs `git rebase` without conflict handling,
    is more dangerous than reporting unsupported.

    Note the ordering trap: if the recipe retries only on success, and success stops being returned
    for these steps, recovery will simply stop retrying. That is the correct behaviour — an
    unrepaired retry is a wasted attempt and can double an effect — but it will look like a
    regression in retry counts, so say so in the change.

  file_changes:
    - "modify: engine/src/recovery_recipes/application/service_impl.rs"
    - "modify: engine/src/recovery_recipes/domain/ (outcome type)"
    - "modify: engine/.pi/architecture/modules/recovery-recipes.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-RECOVERY-STEPS-REPORT-SUCCESS-WITHOUT-REPAIRING

## Verified

```rust
/// In a production implementation, this would actually execute the step
/// (run clean build, retry connection, etc.). For now, returns success
/// for steps that don't require external execution, and simulates basic steps.
async fn execute_step(&self, step: &RecoveryStep) -> Result<(), String> {
    match step {
        // Safe steps — simulated as successful
        RecoveryStep::CleanBuild
        | RecoveryStep::ExpandContext
        | RecoveryStep::RebaseBranch
        | RecoveryStep::AcceptTrust
        | RecoveryStep::RestartWorker => Ok(()),
```

Five of six steps return `Ok(())` having done nothing. The sixth (`RetryConnection`) validates
`timeout_ms > 0` and then also returns `Ok(())`.

## Why this is high now, and was not before

The report recorded it as "recorded, not hidden: 'wired' does not mean 'repairs'". That was accurate
and appropriately scoped **while the seam was unreachable** — nothing could enable it, so nothing
could be misled.

#1111 changed that. `recovery_service` is now attached from cli, mcp and actions when
`RIGORIX_RECOVERY` arms it. So the state changed from "a dormant inaccuracy in a component nobody
invokes" to "a shipped feature that reports success for work it did not do".

That is precisely the distinction the wiring audit exists to catch: reachability converts a dormant
claim into an active one, and the claim's accuracy has to be re-checked at that moment.

## The failure shape

A repair that returns success is worse than a repair that fails:

| outcome | what happens next |
|---|---|
| repaired | retry proceeds — correct |
| failed | run reports the failure — correct, if unhelpful |
| **claims repaired, did nothing** | retry proceeds against the unrepaired state, the original failure recurs, and the recovery layer is recorded as having handled it |

The third row also corrupts the evidence: the signed record says recovery was attempted and
succeeded, which is false.

## What "done" looks like

`execute_step` returns an outcome that distinguishes **performed** from **unsupported**. Only
*performed* permits the retry. A test asserts that a simulated step produces `Unsupported`, never a
success — and therefore that no retry follows it.
