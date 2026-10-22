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

# ISSUE-1157: five recovery steps report success without repairing

## Intent

Make "I did nothing" expressible, and stop it from being treated as "fixed".

## The chain

```
node fails
  → recovery_svc.attempt_recovery            node.rs:285
      → execute_step(CleanBuild) => Ok(())   service_impl.rs:71   ← nothing happens
  → ctx.record_attempt(scenario)             node.rs:300          ← recorded anyway
  → is_recovered() == true
  → state.status = NodeStatus::Ready         node.rs:319
  → continue                                 node.rs:320          ← same broken state
```

## Step dispositions (required)

| step | side effect needed | proposed |
|---|---|---|
| `CleanBuild` | spawn a build | **Unsupported** — process-spawning sink, separate issue |
| `RebaseBranch` | spawn `git rebase` | **Unsupported** — same |
| `RestartWorker` | restart a component | **Unsupported** — supervisor does not exist here |
| `ExpandContext` | none (adjusts retry input) | **implement** |
| `AcceptTrust` | records a decision | **implement**, if it needs no external call |
| `RetryConnection` / `RestartService` / `EscalateToHuman` | validation only, by design | already honest — keep |

## In Scope

- `RecoveryStepOutcome`; no bare `Ok`
- Implement the in-process steps; mark the rest unsupported
- `is_recovered()` requires all performed
- Record the outcome, not just the attempt

## Out of Scope

- Implementing process spawning for the three side-effect steps
- Recipe catalogue changes

## Affected Layers

### Domain
- New `RecoveryStepOutcome`; `is_recovered` semantics

### Application
- `execute_step` returns an outcome; `node.rs` gates `Ready` on it

### Infrastructure
- None

### API
- None

## Canonical References

- **Module:** `.pi/architecture/modules/recovery-recipes.md`
- **ADR-016:** `.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md`
- **Evidence:** `service_impl.rs:64-105`; `node.rs:285,300,319-320`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | No success without action or explicit Unsupported | `validate-tests.sh` |
| 2 | Unsupported never sets Ready | `validate-tests.sh` |
| 3 | Record distinguishes performed from unsupported | `validate-canonical.sh` |
| 4 | Each step has a disposition | MR description |
| 5 | Loop terminates via retry budget, does not spin | `validate-tests.sh` |

## Implementation

> **Agent:** the type change is the fix. Do not implement process spawning here; that has its own
> issue and its own security review.

### Steps

1. Read `service_impl.rs:55-110` and `node.rs:270-330`
2. Add `RecoveryStepOutcome`; change the signature
3. Implement `ExpandContext` / `AcceptTrust` honestly if side-effect-free
4. Mark `CleanBuild` / `RebaseBranch` / `RestartWorker` unsupported with a reason
5. Gate `NodeStatus::Ready` on all-performed; record the outcome
6. Tests: all-performed, some-unsupported, record contents

## Notes

- **`Ok(())` is the lie, not the missing feature.** `Result<(), String>` cannot say "did nothing".
- Do not half-implement the process-spawning steps — that is how this issue was created.
- `record_attempt` runs before the outcome is known; move or extend it.
- Check the `continue` at :320 cannot spin without consuming the retry budget.
- The record is signed: a repair that did not happen is a false history entry, not a display bug.

## Why high

It corrupts both the control flow and the signed record. A node marked `Ready` after a repair that
never ran will fail again for the same reason, and the audit trail will say it was recovered — so the
one artifact operators trust to explain a failure will explain it wrongly.
