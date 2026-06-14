# Disaster Recovery Plan: budget-tracking Module

<!--
Canonical Reference: .pi/architecture/modules/budget-tracking.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `budget-tracking` module — the LLM call and token budget
tracking system with RAII reservation. The budget module is purely in-memory and
stateless at startup. All budget state is ephemeral and tied to a single execution
session.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless at startup — `LlmBudgetImpl` is created fresh from factory |
| RPO (Recovery Point Objective) | 0 (in-memory only) | Budget state is ephemeral; no persistent state to recover |

## Backup Strategy

**No backups required — budget state is ephemeral per execution.**

The budget module operates entirely in memory:
1. `LlmBudgetImpl` counters (`used_calls`, `used_tokens`) are atomic integers
2. Reservations are RAII guards — auto-rollback on Drop, no leak
3. Uncommitted reservations are automatically rolled back on process termination
4. At execution end, all budget state is discarded

The configuration (`max_calls`, `max_tokens`) is derived from factory presets or
the enforcement config, which is backed up by the Configuration module
(see `dr-plan-configuration.md`).

### What Gets Recreated

On budget creation, the following is built fresh:

| Component | Source | Recovery Method |
|-----------|--------|-----------------|
| `LlmBudgetImpl` | Factory preset or custom config | Rebuilt from `LlmBudgetFactoryImpl` |
| `used_calls` counter | Always starts at 0 | AtomicU32 initialized to 0 |
| `used_tokens` counter | Always starts at 0 | AtomicU32 initialized to 0 |
| `CancellationToken` | Always starts uncancelled | `CancellationToken::new()` |
| Active warnings | Runtime tracking | Tracked fresh — dedup flags start false |
| `LlmBudgetReservationImpl` | Per-call creation | Created on each `reserve()` call |

## Restore Procedure

### Scenario: Budget State Corruption

If the budget's internal state becomes corrupted (counters at unexpected values):

1. **Detect corruption:** `get_status()` returns suspicious usage values
   (e.g., `calls_used > max_calls` despite no error)
2. **Create new budget:**
   ```rust
   let factory = LlmBudgetFactoryImpl;
   let new_budget = factory.create_default().await?;
   ```
3. **Cancel old budget's token** if it was exhausted:
   ```rust
   // The old budget is dropped; new budget has fresh counters
   ```
4. **Log the incident:** Record the state corruption event with execution ID for audit

### Scenario: Reservation Leak

If reservations are created but never committed or dropped (theoretical — RAII
guarantees Drop on panic):

1. **Detect leak:** Budget counters increment without corresponding LLM calls
2. **Check for uncommitted reservations:** The counter values will be higher than
   expected for the number of completed LLM calls
3. **Create fresh budget** and retrack known consumption:
   ```rust
   let factory = LlmBudgetFactoryImpl;
   let new_budget = factory.create_default().await?;
   // Replay reservations for known completed calls
   ```
4. **This should not happen** — the RAII pattern ensures Drop is always called.
   If observed, it indicates a `std::mem::forget()` or similar escape

### Scenario: Conflicting Concurrent Budgets

If multiple tasks share the same `LlmBudgetImpl` incorrectly (atomics make this
safe, but counters may diverge from expected):

1. **Check combined budget status:**
   ```rust
   let status = budget.get_status(GetBudgetStatusInput {
       execution_id: eid,
   }).await?;
   ```
2. **Budget per task** — each task should have its own reservation guard
3. **Reset** by creating a new budget instance if counters diverge too far
4. **Add per-task budgeting** in future iterations to isolate budgets

## Failover Plan

### Single Instance Architecture

The budget module runs as part of the orchestrator process. Each execution creates
its own `LlmBudgetImpl` instance. There is no standby or failover — budget state
is recreated per execution.

### Failure Scenarios

| Scenario | Impact | Mitigation |
|----------|--------|------------|
| Budget panic in reserve/commit | Execution loses budget enforcement | Create new budget from factory |
| CancellationToken not triggered | Budget exhaustion not propagated | Check `has_capacity()` manually |
| Reservation dropped without commit | Minor counter rollback | Normal behavior — RAII guarantees rollback |
| Double commit attempted | Error returned | Guard has `is_committed()` check |
| Process crash mid-reservation | Reservation lost | In-memory only — fresh budget on restart |
| Factory returns error (zero limits) | Budget cannot be created | Use non-zero limits via preset builders |

### Monitoring Signs

Watch for these indicators that budget tracking is degraded:

1. **`has_capacity()` returns false too early** — Counters may be inflated by
   uncommitted reservations that were not dropped (theoretical)
2. **Token over-consumption** — If tokens_used grows faster than expected,
   commit may be passing incorrect actual_tokens values
3. **No budget warnings** — In long-running executions, some warnings are expected;
   zero may indicate warnings are not being emitted correctly

## DR Testing

### Test Scenarios

| Scenario | Test Method | Frequency |
|----------|-------------|-----------|
| Reserve within call limits succeeds | `test_reserve_within_limits` | Every CI run |
| Reserve exceeding max_calls fails | `test_reserve_exceeds_calls` | Every CI run |
| Reserve exceeding max_tokens fails | `test_reserve_exceeds_tokens` | Every CI run |
| RAII rollback on Drop resets counters | `test_drop_rollback` | Every CI run |
| Commit with different actual_tokens adjusts | `test_reservation_commit_adjusts_counters` | Every CI run |
| Double commit fails with error | `test_double_commit_fails` | Every CI run |
| Commit prevents auto-rollback | `test_commit_prevents_rollback` | Every CI run |
| Factory presets return correct limits | Factory preset tests (4 tests) | Every CI run |
| Has capacity checks edge cases | `test_has_capacity*` (3 tests) | Every CI run |

### CI Validation

22 budget-tracking-specific tests run as part of the standard test suite. These
cover all reservation lifecycle states, all factory presets, and edge cases.

Additionally, CI stage 17 (`budget-tracking_proofing`) runs:
- **Contract implementation check** — verifies all 14 interface contracts have implementations
- **Coverage threshold check** — enforces ≥80% coverage for the module

---

*Last updated: 2026-06-14*
