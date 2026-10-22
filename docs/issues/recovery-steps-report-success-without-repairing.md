---
guardian_issue:
  id: "ISSUE-RECOVERY-SIMULATES-REPAIRS"
  title: "recovery: five steps return Ok without performing anything, and the run then marks the node Ready — a simulated repair is recorded as a real one"
  epic: "recovery correctness"
  epic_id: "EPIC-RECOVERY-CORRECTNESS"
  tracking_issue: 1157
  component: "RecoveryRecipes"
  module: "recovery-recipes"
  status: implemented
  priority: high
  created_at: "2026-10-23T06:00:00Z"
  updated_at: "2026-10-23T14:00:00Z"

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

When a node fails the engine asks a recovery service for a recipe and executes its steps. Five steps
return `Ok(())` without doing anything, and the service says so in its own doc
(`engine/src/recovery_recipes/application/service_impl.rs:64-75`): *"returns success for steps that
don't require external execution, and simulates basic steps."* The five are `CleanBuild`,
`ExpandContext`, `RebaseBranch`, `AcceptTrust`, `RestartWorker`. The other three
(`RetryConnection`, `RestartService`, `EscalateToHuman`) are argument-validation steps and are
honest — they do not claim to act.

The caller trusts that (`engine/src/execution_engine/application/service_impl/node.rs:285-320`):

```
attempt_recovery(...)                    // :285
  ctx.record_attempt(scenario);          // :300  recorded before the outcome is checked
  if is_recovered() {
      state.status = NodeStatus::Ready;  // :319  as if repaired
      continue;                          // :320  re-enter the retry loop
  }
```

Two consequences, the second worse:

1. **The signed record gains an attempt for a repair that never happened** — and the record is the
   artifact operators use to explain a failure.
2. **Control flow treats the node as repaired.** A node that needed a clean build is set `Ready`
   without one, re-enters the retry loop in the same broken state, and spends the retry budget on a
   repair that cannot work.

The fix in one sentence: `Result<(), String>` cannot say "I did nothing, and that is not an error" —
so introduce an outcome, classify the five, implement the implementable ones, and mark the rest
explicitly unsupported.

## Step dispositions (decided — implement, do not re-open)

| step | needs | disposition |
|---|---|---|
| `CleanBuild` | spawn a build | **Unsupported** — process-spawning sink, own security surface, separate issue |
| `RebaseBranch` | spawn `git rebase` | **Unsupported** — same |
| `RestartWorker` | restart a component | **Unsupported** — no supervisor exists in this engine |
| `ExpandContext` | none | **implement honestly** if side-effect-free |
| `AcceptTrust` | records a decision | **implement honestly** if it needs no external call |
| `RetryConnection` / `RestartService` / `EscalateToHuman` | validation, by design | keep |

Implementing the two in-process steps is what turns this from a rename into a repair: the service
becomes partly real rather than uniformly declared.

## In Scope

- `RecoveryStepOutcome` (`Performed` | `Unsupported { reason }`) replacing the bare `Ok(())`
- `execute_step` returns it; `attempt_recovery` aggregates it
- `is_recovered()` requires every step `Performed`
- The outcome is recorded, so the record distinguishes performed from unsupported
- Tests for both paths

## Out of Scope

- Process spawning for `CleanBuild` / `RebaseBranch` / `RestartWorker`
- Recipe catalogue, scenario→recipe mapping, retry budget policy
- Redesigning what the retry loop does when recovery fails (confirm it falls through; do not redesign)

## Affected Layers

### Domain
- New `RecoveryStepOutcome`; `RecoveryResult::is_recovered` becomes "all performed"

### Application
- `execute_step`, `attempt_recovery`, and the `node.rs` gate on `NodeStatus::Ready`

### Infrastructure
- None — nothing new is spawned

### API
- None

## Canonical References

- **Module:** `.pi/architecture/modules/recovery-recipes.md`
- **ADR:** `.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md` (the record is signed)
- **Evidence:** `recovery_recipes/application/service_impl.rs:64-105`;
  `execution_engine/application/service_impl/node.rs:285,300,319-320`
- **Policy:** `.pi/architecture/modules/tool-system.md#process-spawning-sinks`

## Acceptance Criteria

| # | Criterion | How it is checked |
|---|-----------|-------------------|
| 1 | No step reports success without an action or an explicit `Unsupported` | `validate-tests.sh` |
| 2 | An `Unsupported` step cannot set `NodeStatus::Ready` | `validate-tests.sh` |
| 3 | The record distinguishes performed from unsupported | record field + test |
| 4 | Each of the five steps has a stated disposition | MR description |
| 5 | The retry loop terminates via the retry budget, does not spin | `validate-tests.sh` |
| 6 | No new process-spawning site | `validate-security.sh` |

## Implementation

1. Read `service_impl.rs:55-115` and the `attempt_recovery` aggregation; then `node.rs:260-330`.
2. Add `RecoveryStepOutcome`; change the signature.
3. Implement `ExpandContext` / `AcceptTrust` if side-effect-free; otherwise they join the unsupported
   set with a reason.
4. Mark the three side-effect steps unsupported with a reason naming the missing capability.
5. Make `is_recovered()` require all-performed; move or extend `record_attempt` so the outcome
   travels with it; gate `Ready` on the outcome.
6. Tests: all-performed → `Ready`; any-unsupported → not `Ready`, normal failure path; record carries
   the outcome.

## Pitfalls

- **Do not half-implement the side-effect steps.** Faking them is how this issue was created;
  implementing them badly creates a worse one — they need the allowlist discipline in
  `tool-system.md#process-spawning-sinks`.
- **`record_attempt` runs before the outcome is known**, so the record cannot be corrected
  afterwards. Move it or extend it.
- **Watch the `continue`** — confirm it cannot re-enter the loop with the same scenario without
  consuming the retry budget.
- **Do not delete the recovery feature.** Five steps that say "unsupported" are honest and usable;
  the type is the defect.
- **A bare `success: false` is not enough** — the record needs the reason, or the next reader cannot
  tell "not implemented" from "ran and failed".

## Why this is high

It corrupts control flow and the signed record together: a node marked `Ready` after a repair that
never ran fails again for the same reason, the audit trail says it was recovered, and the retry
budget is spent proving it.
