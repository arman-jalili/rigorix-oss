# Disaster Recovery Plan: recovery-recipes Module

<!--
Canonical Reference: .pi/architecture/modules/recovery-recipes.md
Last Updated: 2026-06-19
-->

## Overview

The recovery-recipes module is a **stateless, configuration-driven** system that executes
recovery procedures for known failure scenarios. It does not store persistent data —
all state is held in the per-session `RecoveryContext` which is ephemeral.

This DR plan covers failure of the recovery system itself and its integration points.

## Recovery Objectives

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 5 minutes | Full module restoration |
| RPO (Recovery Point Objective) | 0 | No persistent state to lose |
| MTD (Maximum Tolerable Downtime) | 30 minutes | Execution continues without recovery |

## Failure Scenarios

### F1: Recovery Recipe Repository Corruption

**Symptoms:** Recipes return unexpected steps or invalid configurations.

**Impact:** Recovery steps may execute wrong operations or skip recovery entirely.

**Detection:** `RecoveryEvent::RecipeNotFound` or validation errors from
`RecoveryService::validate_recipe()`.

**Recovery Steps:**
1. Clear custom recipes: call `repository.clear_recipes()`
2. Verify default catalog loads correctly: `RecoveryRecipe::default_catalog()`
3. Re-register custom recipes from backup configuration
4. Validate: run contract check script

```bash
# Clear and reset
cd engine
bash .pi/scripts/ci/check_recovery-recipes_contracts.sh
```

**RTO:** 2 minutes

**Prevention:**
- Validate all custom recipes before registration
- Use `RecoveryService::validate_recipe()` to check recipe validity

### F2: RecoveryContext Corruption

**Symptoms:** Incorrect attempt counts or duplicate events in the event log.

**Impact:** Recovery may be incorrectly blocked (escalation triggered too early)
or allowed to run indefinitely (infinite loop).

**Detection:** Unexpected `RecoveryEvent::Escalated` events or anomalous
attempt counts in logs.

**Recovery Steps:**
1. Reset the current session's `RecoveryContext`: `context.reset()`
2. Log the corruption incident for post-mortem analysis
3. Re-create a fresh `RecoveryContext` for the session

**RTO:** 1 minute

**Prevention:**
- Construct fresh `RecoveryContext` per execution session
- `RecoveryContext` is not shared across sessions

### F3: Default Recipe Catalog Desync

**Symptoms:** Recipes in the default catalog don't match the architecture
specification.

**Impact:** Wrong recovery steps executed for known failure scenarios.

**Detection:** Periodic audit via contract check scripts comparing catalog
against architecture module.

**Recovery Steps:**
1. Compare `RecoveryRecipe::default_catalog()` against
   `.pi/architecture/modules/recovery-recipes.md` built-in recipe table
2. Update `recipe.rs` to match specification
3. Re-run all tests
4. Deploy updated module

```bash
# Verify catalog integrity
cargo test --lib recovery_recipes::domain::recipe::tests
```

**RTO:** 10 minutes

**Prevention:**
- Version-lock recipes to architecture module via canonical references
- CI stage checks catalog integrity

### F4: FailureScenario → FailureType Mapping Drift

**Symptoms:** `FailureScenario::from_failure_type()` returns incorrect mappings.

**Impact:** Wrong recipe selected for a given failure, or no recipe selected
when one should exist.

**Detection:** Mismatch between `FailureType` variants and `FailureScenario`
mapping table in architecture.

**Recovery Steps:**
1. Check `scenario.rs` for the `from_failure_type()` mapping
2. Compare against failure_classification's `FailureType` enum
3. Add/modify mappings as needed
4. Update the comment table in `scenario.rs` and architecture doc
5. Run tests to verify

**RTO:** 5 minutes

**Prevention:**
- Map both directions: every `FailureType` should map to a `FailureScenario`
  or explicitly return `None` with a documented reason
- CI stage validates mapping consistency

### F5: Recovery Service Failure

**Symptoms:** `RecoveryService::attempt_recovery()` panics or returns unexpected errors.

**Impact:** No recovery possible — all failures escalate immediately.

**Detection:** Error logs from `RecoveryError` variants.

**Recovery Steps:**
1. Check if the issue is in step execution or recipe lookup
2. If step execution: verify step validation logic in `execute_step()`
3. If recipe lookup: verify `find_recipe()` logic
4. If internal state: recreate `RecoveryServiceImpl` fresh

**RTO:** 5 minutes

**Prevention:**
- `RecoveryServiceImpl` is stateless (custom recipes via separate repository)
- Tests cover all error paths

## Backup and Restore

### What to Backup

| Data | Backup Strategy | Frequency |
|------|----------------|-----------|
| Custom recipes | Configuration management | Per custom recipe change |
| Default catalog | Source code (git) | Per commit |
| RecoveryContext | Not persisted (ephemeral) | N/A |

### Backup Locations

- **Custom recipes**: In code or config files committed to git
- **Default catalog**: `src/recovery_recipes/domain/recipe.rs` — in git

### Restore Procedure

```bash
# Restore default catalog from git
git log --oneline -- src/recovery_recipes/domain/recipe.rs
git checkout <commit> -- src/recovery_recipes/domain/recipe.rs

# Restore custom recipes from config
# (Re-register via repository)
```

## Failover

The recovery-recipes module has no failover mechanism — it is a single-instance
module. If the service fails, the execution engine falls through to standard
retry logic without automatic recovery.

### Degraded Mode

Without recovery-recipes:
- Execution engine uses standard retry logic only
- No automatic recovery for known failure scenarios
- All failures escalate immediately to retry policy

## Testing the DR Plan

| Test | Frequency | How |
|------|-----------|-----|
| Contract check | Per CI run | `.pi/scripts/ci/check_recovery-recipes_contracts.sh` |
| Coverage check | Per CI run | `.pi/scripts/ci/check_recovery-recipes_coverage.sh` |
| Full proofing stage | Per CI run | Stage 29 in hardening pipeline |
| Catalog integrity | Per release | Compare against architecture spec |
| Mapping consistency | Per release | Verify from_failure_type() mappings |

## Recovery Event Audit Trail

All recovery actions are recorded via `RecoveryEvent` on the `EventBus`:

```rust
RecoveryEvent::RecoveryAttempted { scenario, step, attempt_number }
RecoveryEvent::RecoverySucceeded { scenario, steps_taken, result }
RecoveryEvent::RecoveryFailed { scenario, failed_step, reason, is_final_attempt }
RecoveryEvent::Escalated { scenario, attempts_made, reason }
RecoveryEvent::RecipeNotFound { scenario, original_error }
```

These events enable post-mortem analysis and monitoring dashboards.

## Escalation Contacts

| Scenario | Escalation Path | Priority |
|----------|----------------|----------|
| Module doesn't load | DevOps / Platform | P1 |
| Incorrect recipes executed | Engineering lead | P2 |
| RecoveryContext corruption | Engineering team | P3 |
| Mapping drift | Architecture team | P3 |
