# Runbook: recovery-recipes Module

<!--
Canonical Reference: .pi/architecture/modules/recovery-recipes.md
Last Updated: 2026-06-19
-->

## Overview

The `recovery-recipes` module encodes known failure scenarios and their automatic recovery
procedures. The core rule is: **one automatic recovery attempt per scenario before human
escalation.** Each failure scenario has a `RecoveryRecipe` — a sequence of recovery steps
with a maximum attempt count and an escalation policy for when attempts are exhausted.

This transforms Rigorix's DR plans from prose documentation into **executable code** —
the engine can automatically recover from compile errors, test failures, connection issues,
and other known failure modes.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| failure_classification | Yes | Provides `FailureType` for scenario mapping |
| execution_engine | Yes | Re-executes nodes after recovery |
| event_system | Yes | Emits `RecoveryEvent` types for audit trail |
| cancellation | No | Aborts recovery if execution is cancelled |

### Initialization

1. Create a `RecoveryContext` for the execution session
2. Load `RecoveryRecipeRepository` (default catalog + custom overrides)
3. Create a `RecoveryServiceImpl` with recipe repository
4. Wire into the Execution Engine's retry loop

```rust
use rigorix::recovery_recipes::application::*;
use rigorix::recovery_recipes::infrastructure::*;

// Per execution session
let mut context = RecoveryContext::new();
let repository = InMemoryRecipeRepository::new();
let service = RecoveryServiceImpl::new();

// Wire into execution engine
let result = executor.execute_with_recovery(
    &node,
    &mut context,
    &service,
).await;
```

### Quick Start

```rust
use rigorix::recovery_recipes::domain::*;
use rigorix::recovery_recipes::application::*;

// Create a recovery context
let mut ctx = RecoveryContext::new();

// Check if recovery is possible
let recipe = RecoveryRecipe::default_catalog()
    .into_iter()
    .find(|r| r.scenario == FailureScenario::CompileError)
    .unwrap();

if ctx.can_attempt(FailureScenario::CompileError, &recipe) {
    // Execute recovery step
    let step = &recipe.steps[0];
    match step {
        RecoveryStep::CleanBuild => {
            // cargo clean && cargo build
            ctx.record_attempt(FailureScenario::CompileError);
            ctx.record_event(RecoveryEvent::RecoverySucceeded {
                scenario: FailureScenario::CompileError,
                steps_taken: 1,
                result: RecoveryResult::Recovered { steps_taken: 1 },
            });
        }
        _ => {}
    }
}
```

## Graceful Shutdown

1. **Drain recovery queue**: Allow any in-progress recovery steps to complete
2. **Persist context**: Save `RecoveryContext` events for audit trail
3. **Cancel pending**: Invoke cancellation signal on remaining recovery attempts
4. **Clean up**: Release recipe repository resources

## Common Failure Modes

### "No recipe for scenario"

**Cause:** A `FailureType` was encountered that has no corresponding `RecoveryRecipe`.

**Detection:** `RecoveryEvent::RecipeNotFound` emitted.

**Recovery:** The execution engine falls through to standard retry logic. No automatic
recovery is attempted.

**Prevention:** Add a recipe to the default catalog or register a custom override via
`RecoveryRecipeRepository::store_recipe()`.

### "Max recovery attempts reached"

**Cause:** A scenario exhausted its `max_attempts` limit without successful recovery.

**Detection:** `RecoveryEvent::Escalated` emitted, `RecoveryResult::EscalationRequired` returned.

**Recovery:** The escalation policy is applied — `AlertHuman` (notify operator),
`LogAndContinue` (skip node), or `Abort` (terminate session).

**Prevention:** Review recipe effectiveness. If recovery steps are insufficient, expand
the recipe with additional steps or increase `max_attempts`.

### "Recovery step failed"

**Cause:** A specific recovery step (e.g., `CleanBuild`, `RetryConnection`) failed during
execution.

**Detection:** `RecoveryEvent::RecoveryFailed` emitted.

**Recovery:** If this is not the final attempt, the engine retries from step 1 on the next
attempt. If it is the final attempt, escalation is triggered.

**Prevention:** Validate step parameters. Check that `RetryConnection` timeout is reasonable,
`RestartService` names are valid, and `CleanBuild` has the necessary build tools installed.

## Configuration Reference

### Default Recipe Catalog

| Scenario | Steps | Max Attempts | Escalation |
|----------|-------|-------------|------------|
| CompileError | CleanBuild → ExpandContext | 1 | AlertHuman |
| TestFailure | ExpandContext | 1 | AlertHuman |
| ToolConnectionError | RetryConnection(30s) → RestartService | 2 | AlertHuman |
| ProviderFailure | RetryConnection(10s) → RetryConnection(60s) | 2 | AlertHuman |
| PartialInitialization | RestartWorker | 1 | AlertHuman |
| AuthorizationError | AcceptTrust | 1 | AlertHuman |
| StaleBranch | RebaseBranch | 1 | LogAndContinue |

### Custom Overrides

Custom recipes can be registered via `RecoveryRecipeRepository::store_recipe()`:

```rust
let custom_recipe = RecoveryRecipe::new(
    FailureScenario::CompileError,
    vec![RecoveryStep::CleanBuild, RecoveryStep::ExpandContext],
    2, // Allow 2 attempts
    EscalationPolicy::AlertHuman,
)?;

repository.store_recipe(custom_recipe).await?;
```

## Monitoring

### Key Metrics

- `recovery_attempts_total{scenario}` — Count of recovery attempts per scenario
- `recovery_success_total{scenario}` — Count of successful recoveries per scenario
- `recovery_failures_total{scenario}` — Count of failed recoveries per scenario
- `recovery_escalations_total{scenario}` — Count of escalations per scenario

### Relevant Events

All events are emitted as `RecoveryEvent` on the `EventBus`:

- `RecoveryAttempted` — A recovery step was initiated
- `RecoverySucceeded` — A recovery completed successfully
- `RecoveryFailed` — A recovery step failed
- `Escalated` — All attempts exhausted, escalation applied
- `RecipeNotFound` — No recipe for a given scenario

## Troubleshooting

| Symptom | Likely Cause | Action |
|---------|-------------|--------|
| Infinite recovery loop | `max_attempts` too high or missing | Set `max_attempts = 1` (try once, escalate) |
| Recovery silently skipped | No recipe registered | Check `RecoveryRecipeRepository` |
| Escalation never triggered | Missing escalation policy | Set `EscalationPolicy::AlertHuman` |
| Wrong recipe applied | Custom override conflicts | Clear custom recipes via `clear_recipes()` |
| Recovery takes too long | Step timeout too high | Reduce `RetryConnection` timeout |

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| Recovery step causes more damage | Steps are predefined in config — no arbitrary command execution |
| Infinite recovery loop | `max_attempts` per scenario + escalation after exhaustion |
| Recovery runs with elevated permissions | Steps execute under same permission mode as the failed node |
