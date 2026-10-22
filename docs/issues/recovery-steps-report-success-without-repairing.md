---
guardian_issue:
  id: "ISSUE-RECOVERY-SIMULATES-REPAIRS"
  title: "recovery: five steps return Ok without performing anything, and the run then marks the node Ready — a simulated repair is recorded as a real one"
  epic: "recovery correctness"
  epic_id: "EPIC-RECOVERY-CORRECTNESS"
  tracking_issue: 1157
  component: "RecoveryRecipes"
  module: "recovery-recipes"
  status: planned
  priority: high
  created_at: "2026-10-23T06:00:00Z"
  updated_at: "2026-10-23T06:00:00Z"

  intent: |
    Separate "I performed the repair" from "I have no implementation for this repair", and stop
    treating the second as the first.

    `RecoveryServiceImpl::execute_step` (`recovery_recipes/application/service_impl.rs:67`) matches on
    `RecoveryStep` and returns `Ok(())` for five variants without doing anything:

    ```rust
    RecoveryStep::CleanBuild | RecoveryStep::ExpandContext | RecoveryStep::RebaseBranch
    | RecoveryStep::AcceptTrust | RecoveryStep::RestartWorker => Ok(()),
    ```

    The function's own doc says so: *"For now, returns success for steps that don't require external
    execution, and simulates basic steps."* The other three variants — `RetryConnection`,
    `RestartService`, `EscalateToHuman` — only validate their arguments.

    `attempt_recovery` therefore reports recovery, and the caller acts on it
    (`execution_engine/application/service_impl/node.rs:285-320`):

    ```rust
    if let Ok(recovery_out) = recovery_svc.attempt_recovery(recovery_input).await {
        ctx.record_attempt(scenario);              // :300 — recorded before checking
        if recovery_out.result.is_recovered() {
            state.status = NodeStatus::Ready;      // :319 — as if repaired
            continue;                              // re-enter the retry loop
        }
    }
    ```

    Two consequences, and the second is worse than the first:

    1. The signed audit record contains a recovery **attempt** for a repair that never happened.
    2. Control flow treats the node as repaired. A node whose failure needed a clean build is set to
       `Ready` without a clean build, so it re-enters the retry loop with the same broken state — and
       the retry budget is consumed by a repair that cannot work.

  dependencies:
    - name: "RecoveryService::execute_step"
      type: internal
      note: "recovery_recipes/application/service_impl.rs:67-107 — the five `Ok(())` arms."
    - name: "AttemptRecovery / is_recovered"
      type: internal
      note: "The result the caller trusts; it must distinguish performed from unsupported."
    - name: "node.rs recovery branch"
      type: internal
      note: "execution_engine/application/service_impl/node.rs:285-320 — sets NodeStatus::Ready and continues."
    - name: "ISS-ENFORCEMENT/record integrity"
      type: external
      note: "The audit record is signed; recording a recovery that did not occur corrupts the record's meaning, not just its display."

  in_scope:
    - "Introduce an outcome: Performed | Unsupported { reason } — never a bare Ok"
    - "Classify the five steps: which are implementable in-process, which need a side effect that does not exist"
    - "Implement the implementable ones honestly"
    - "Mark the rest Unsupported with a reason naming what is missing"
    - "`is_recovered()` requires every step Performed; Unsupported never yields Ready"
    - "Record the attempt with its outcome, so the signed record says what actually happened"
    - "Test: an Unsupported step does not set Ready, does not consume the recovery path as success, and is visible in the record"

  out_of_scope:
    - "Implementing process spawning for CleanBuild / RebaseBranch / RestartWorker — that is a separate issue with its own security surface (tool-system process-spawning sinks)"
    - "Changing the recipe catalogue or scenario→recipe mapping"
    - "Retry budget policy"

  affected_layers:
    domain:
      - "New: RecoveryStepOutcome (Performed / Unsupported)"
      - "Modified: RecoveryResult::is_recovered semantics"
    application:
      - "Modified: execute_step returns an outcome, not Result<(), String>"
      - "Modified: attempt_recovery aggregates outcomes"
      - "Modified: node.rs decides Ready only on performed"
    infrastructure:
      - "None for the in-process steps; side-effect steps stay unsupported"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/recovery-recipes.md"
    - adr:
        - ".pi/architecture/decisions/ADR-009-error-handling.md"
        - ".pi/architecture/decisions/ADR-016-audit-integrity-anchor.md"
    - last_sync: "2026-10-23"
    - evidence: "recovery_recipes/application/service_impl.rs:64-67 — doc comment: 'simulates basic steps'"
    - evidence: "recovery_recipes/application/service_impl.rs:70-75 — the five `=> Ok(())` arms"
    - evidence: "recovery_recipes/application/service_impl.rs:77-105 — the other three only validate arguments"
    - evidence: "execution_engine/application/service_impl/node.rs:285 — the only production call site"
    - evidence: "execution_engine/application/service_impl/node.rs:300 — record_attempt before the outcome is checked"
    - evidence: "execution_engine/application/service_impl/node.rs:319-320 — state.status = Ready; continue"

  acceptance_criteria:
    - "No RecoveryStep variant can report success without an action or an explicit Unsupported"
    - "An Unsupported step cannot set NodeStatus::Ready"
    - "The outcome is recorded, and a reader of the signed record can tell performed from unsupported"
    - "Each of the five steps has a stated disposition (implemented, or unsupported with the reason)"
    - "Tests cover: all-supported path reaches Ready; any-unsupported path does not; the record distinguishes them"
    - "validate-ci.sh, validate-tests.sh, validate-security.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - security
    - canonical

  implementation_notes: |
    **Do not half-implement the side-effect steps.** `CleanBuild` and `RebaseBranch` require spawning
    processes, which puts them under the process-spawning allowlist discipline
    (`.pi/architecture/modules/tool-system.md#process-spawning-sinks`). `RestartWorker` restarts a
    component. Faking those is what created this issue; implementing them badly would create a worse
    one. Return `Unsupported { reason }` and file the follow-up.

    `ExpandContext` and `AcceptTrust` are likely implementable in-process — the first adjusts what
    the retry carries, the second records a trust decision. Implement those for real if they can be
    done without side effects; that is what makes the fix more than a rename.

    **`Ok(())` is the lie, not the missing feature.** A repository with five unimplemented recovery
    steps that says "unsupported" is honest and usable. The defect is the type: `Result<(), String>`
    has no way to say "I did nothing, and that is not an error".

    Move `record_attempt` after the outcome is known, or record the outcome with it. As written, the
    attempt is recorded before anyone checks whether anything happened, so the record cannot be
    corrected afterwards.

    Watch the retry loop. If an unsupported step no longer reports recovered, the node falls through
    to the normal failure path — which is the correct behaviour, but confirm it does not silently
    loop: `continue` at :320 re-enters the loop with the same scenario, and the loop must terminate
    via the retry budget rather than spinning.

  file_changes:
    - "modify: engine/src/recovery_recipes/application/service_impl.rs"
    - "modify: engine/src/recovery_recipes/domain/ (new RecoveryStepOutcome)"
    - "modify: engine/src/execution_engine/application/service_impl/node.rs"
    - "create: engine/tests/recovery_outcomes.rs"
    - "modify: engine/.pi/architecture/modules/recovery-recipes.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---
# ISSUE-1157: five recovery steps report success without repairing, and the node is then marked `Ready`

## What you are looking at

When a node fails, the engine consults a recovery service, which looks up a recipe for the failure
scenario and executes its steps. Five of those steps return `Ok(())` without doing anything:

```rust
// engine/src/recovery_recipes/application/service_impl.rs:67
/// Execute a single recovery step.
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
        | RecoveryStep::RestartWorker => Ok(()),          // :70-75  ← nothing happens
        RecoveryStep::RetryConnection { timeout_ms } => { /* validates only */ }
        RecoveryStep::RestartService { name } => { /* validates only */ }
        RecoveryStep::EscalateToHuman { reason } => { /* validates only */ }
    }
}
```

The caller trusts that (`engine/src/execution_engine/application/service_impl/node.rs:285-320`):

```rust
if let Ok(recovery_out) = recovery_svc.attempt_recovery(recovery_input).await {
    ctx.record_attempt(scenario);            // :300  recorded before the outcome is checked
    if recovery_out.result.is_recovered() {
        state.status = NodeStatus::Ready;    // :319  as if repaired
        continue;                            // :320  re-enter the retry loop
    }
}
```

Two consequences. The second is worse:

1. **The signed record gains a recovery attempt for a repair that never happened.** The record is the
   artifact operators use to explain a failure, so it now explains it wrongly.
2. **Control flow treats the node as repaired.** A node whose failure needed a clean build is set to
   `Ready` without a clean build, re-enters the retry loop in the same broken state, and burns the
   retry budget on a repair that cannot work.

Note that `RetryConnection`, `RestartService` and `EscalateToHuman` are *honest*: they are
parameter-validation steps that do not claim to act. The defect is confined to the five above.

## The fix, in one sentence

`Result<(), String>` cannot say "I did nothing, and that is not an error" — so introduce an outcome
type, classify the five steps, implement the ones that can be implemented in-process, and mark the
rest explicitly unsupported.

## Step dispositions (decided — implement, do not re-open)

| step | needed side effect | disposition |
|---|---|---|
| `CleanBuild` | spawn a build process | **Unsupported** — process-spawning sink, own security surface, separate issue |
| `RebaseBranch` | spawn `git rebase` | **Unsupported** — same |
| `RestartWorker` | restart a component | **Unsupported** — no supervisor exists in this engine |
| `ExpandContext` | none — widens what the retry carries | **implement honestly** if it can be done with no side effect |
| `AcceptTrust` | records a decision | **implement honestly** if it needs no external call |
| `RetryConnection` / `RestartService` / `EscalateToHuman` | argument validation, by design | keep as-is |

The point of implementing the two in-process steps is that it turns the change from a rename into a
repair: the recovery service becomes partly real rather than uniformly declared.

## In Scope

- A `RecoveryStepOutcome` (`Performed` | `Unsupported { reason }`) replacing the bare `Ok(())`
- `execute_step` returns it; `attempt_recovery` aggregates it
- `is_recovered()` requires every step `Performed`; one `Unsupported` means not recovered
- The outcome is recorded, so the signed record distinguishes performed from unsupported
- Tests for both paths

## Out of Scope

- Implementing process spawning for `CleanBuild` / `RebaseBranch` / `RestartWorker`
- Recipe catalogue, scenario→recipe mapping, retry budget policy
- Changing what the retry loop does when recovery is not recovered (it should fall through to normal
  failure handling — confirm it does, do not redesign it)

## Affected Layers

### Domain
- New: `RecoveryStepOutcome`
- Modified: `RecoveryResult::is_recovered` semantics — "all performed", not "no errors"

### Application
- `execute_step` signature and body
- `attempt_recovery` aggregation
- `node.rs:285-320` gates `NodeStatus::Ready` on the outcome

### Infrastructure
- None for the in-process steps; nothing new is spawned

### API
- None

## Canonical References

| what | where |
|---|---|
| the simulate-and-return | `engine/src/recovery_recipes/application/service_impl.rs:64-105` |
| the only production call site | `engine/src/execution_engine/application/service_impl/node.rs:285` |
| the record write | `node.rs:300` (`ctx.record_attempt`) |
| the Ready assignment | `node.rs:309-320` |
| process-spawning policy | `.pi/architecture/modules/tool-system.md#process-spawning-sinks` |
| record integrity | `.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md` |

## Acceptance Criteria

| # | Criterion | How it is checked |
|---|-----------|-------------------|
| 1 | No step can report success without an action or an explicit `Unsupported` | `validate-tests.sh` |
| 2 | An `Unsupported` step cannot set `NodeStatus::Ready` | `validate-tests.sh` |
| 3 | The record distinguishes performed from unsupported | `validate-canonical.sh` (record field) + test |
| 4 | Every one of the five steps has a stated disposition | MR description |
| 5 | The retry loop terminates via the retry budget and does not spin | `validate-tests.sh` |
| 6 | No new process-spawning site appears | `validate-security.sh` |

## Implementation

1. Read `service_impl.rs:55-115` (the match), then `:180-260` (`attempt_recovery`).
2. Read `node.rs:260-330` — the branch that sets `Ready` and `continue`s.
3. Add `RecoveryStepOutcome`; change `execute_step` to return it.
4. Implement `ExpandContext` / `AcceptTrust` if side-effect-free; otherwise they join the unsupported
   set with a reason.
5. Mark `CleanBuild` / `RebaseBranch` / `RestartWorker` unsupported with a reason that names the
   missing capability ("no build runner", "no git mutator", "no worker supervisor").
6. Aggregate in `attempt_recovery`; make `is_recovered()` require all-performed.
7. Move or extend `record_attempt` so the outcome travels with it.
8. Gate `NodeStatus::Ready` on the outcome.
9. Tests: all-performed → Ready; any-unsupported → not Ready, normal failure path; record contains
   the outcome.

## Pitfalls

- **Do not half-implement the process-spawning steps.** Faking them is how this issue was created;
  implementing them badly creates a worse one. They have their own allowlist discipline under
  `tool-system.md#process-spawning-sinks`.
- **`record_attempt` currently runs before the outcome is known** (`node.rs:300`). As written the
  record cannot be corrected afterwards. Move it or extend it.
- **Watch the `continue` at :320.** With an unsupported step the node should fall through to the
  failure path; confirm it cannot re-enter the loop with the same scenario without consuming the
  retry budget.
- **Do not delete the recovery feature.** Five unimplemented steps that say "unsupported" are honest
  and usable; the type is the defect.
- **A boolean `success: false` is not enough.** The record needs the *reason*, or the next reader
  cannot tell "not implemented" from "ran and failed".

## Why this is high

It corrupts control flow and the signed record at the same time. A node marked `Ready` after a repair
that never ran will fail again for the same reason, and the audit trail will say it was recovered —
so the one artifact that explains a failure explains it wrongly, and the retry budget is spent
proving it.
