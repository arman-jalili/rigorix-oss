# Runbook: budget-tracking Module

<!--
Canonical Reference: .pi/architecture/modules/budget-tracking.md
Last Updated: 2026-06-14
-->

## Overview

The `budget-tracking` module monitors and enforces LLM token and call budgets per
execution using a RAII reservation pattern. Every LLM call must reserve budget
before invocation; unreserved budget auto-rolls back on Drop. A `CancellationToken`
coordinates budget exhaustion with the cancellation system.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `LlmBudget` | Domain entity | Budget value object — tracks call count, token consumption, limits |
| `LlmBudgetImpl` | Application service | Runtime budget tracker using atomic counters with `CancellationToken` |
| `LlmBudgetReservationImpl` | Application service | RAII guard — auto-rollbacks call + token counters on Drop if not committed |
| `LlmBudgetFactoryImpl` | Factory | Constructs budget instances from preset modes or custom limits |
| `LlmBudgetRepository` | Repository | Persistence interface for budget snapshots and reservation records |

### Preset Profiles

| Preset | Max Calls | Max Tokens | Use Case |
|--------|-----------|------------|----------|
| Default | 5 | 10,000 | Normal operation — ample safety margin |
| Advanced | 20 | 100,000 | Complex tasks with multiple LLM calls |
| Aggressive | 50 | 500,000 | Long-running analysis with many iterations |

### RAII Reservation Lifecycle

```
┌──────────────┐     ┌──────────────────────┐     ┌──────────────┐
│  reserve()   │────>│  LLM API Call        │────>│ commit()    │
│ (estimated)  │     │  (actual tokens)     │     │ (finalize)  │
└──────────────┘     └──────────────────────┘     └──────────────┘
                            │                                        
                            │ error / panic                        
                            ▼                                        
                     ┌──────────────┐                                
                     │  Drop        │                                
                     │ (auto-       │                                
                     │  rollback)   │                                
                     └──────────────┘                                
```

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async runtime for cancellation token |
| tokio-util | Yes | `CancellationToken` for coordinated shutdown |
| uuid | Yes | Execution ID generation for budget tracking |
| serde | Yes | Serialization for DTOs and events |

### Initialization

1. **Select a preset** or configure custom limits:
   ```rust
   let factory = LlmBudgetFactoryImpl;
   let budget = factory.create_default().await?; // 5 calls, 10K tokens
   ```
2. **Create from enforcement config** (recommended):
   ```rust
   let budget = factory.create_from_enforcement_config(
       config.execution_limits.max_tool_calls,
       config.execution_limits.max_tokens,
   ).await?;
   ```
3. **Budget is ready** — reservations can begin immediately

### Quick Start

```rust
use rigorix::budget_tracking::application::*;

// Create a budget with default limits
let factory = LlmBudgetFactoryImpl;
let budget = factory.create_default().await.unwrap();

// Reserve budget for an LLM call
let output = budget.reserve(ReserveBudgetInput {
    execution_id: uuid::Uuid::new_v4(),
    estimated_tokens: 500,
    call_label: Some("classify".to_string()),
}).await.unwrap();

// Create the RAII guard
let mut guard = LlmBudgetReservationImpl::new(
    // ... obtain budget Arc<BudgetState> reference ...
    output.reservation.call_id,
    output.reservation.reserved_tokens,
);

// Execute LLM call...
// Commit with actual token consumption
guard.commit(420).await.unwrap();

// On panic/early return without commit → auto-rollback
```

## Graceful Shutdown

### Normal Shutdown

1. **Check remaining capacity** before shutdown:
   ```rust
   let status = budget.get_status(GetBudgetStatusInput {
       execution_id: eid,
   }).await?;
   if status.active_warnings.is_empty() {
       // No warnings — safe to shut down
   }
   ```
2. **Drop the budget** — all `LlmBudgetReservationImpl` guards should be
   committed or dropped first. Remaining uncommitted guards auto-rollback.
3. The `CancellationToken` can be polled for exhaustion state:
   ```rust
   if budget.cancel_token().is_cancelled() {
       // Budget was exhausted during execution
   }
   ```

### Forced Shutdown

If the process terminates without cleanup:
- In-memory budget counters are lost (acceptable — fresh per execution)
- Open reservations are rolled back by the `Drop` implementation
- No persistent side effects

## Common Failure Modes

### Max Calls Exceeded

**Symptom:** `reserve()` returns `LlmBudgetError::MaxCallsExceeded`.

**Cause:** All allocated LLM calls have been used.

**Resolution:**
1. Check `remaining_calls()` to verify capacity is zero
2. Increase `max_calls` in the budget preset or custom config
3. Reduce the number of LLM calls per execution
4. The `CancellationToken` has been triggered — coordinate with the
   cancellation module for graceful termination

### Max Tokens Exceeded

**Symptom:** `reserve()` returns `LlmBudgetError::MaxTokensExceeded`.

**Cause:** The estimated tokens for the requested call would exceed the remaining
token capacity.

**Resolution:**
1. Check `remaining_tokens()` to verify capacity
2. Reduce the estimated tokens for future calls
3. Increase `max_tokens` in the budget configuration
4. If actual tokens were significantly less than estimated, the
   reservation system refunds the difference on commit

### Reservation Already Committed

**Symptom:** `commit()` returns `LlmBudgetError::ReservationFailed`.

**Cause:** `commit()` was called twice on the same reservation guard.

**Resolution:**
1. Protect against double-commit with `is_committed()` check
2. Ensure commit is only called once per reservation

### Reservation Auto-Rollback

**Symptom:** Budget counters decrease unexpectedly.

**Cause:** A `LlmBudgetReservationImpl` guard was dropped without an explicit
`commit()` call.

**Resolution:**
1. This is normal behavior — the RAII pattern guarantees no budget leakage
2. Check the reservation was not inadvertently dropped early
3. Verify `commit()` is called before the guard goes out of scope

### Zero Estimated Tokens

**Symptom:** `reserve()` returns `LlmBudgetError::ReservationFailed`.

**Cause:** `estimated_tokens` was 0, which is not allowed.

**Resolution:**
1. Always provide a positive estimated token count
2. If unsure, use a minimum estimate (1 token) — the commit will
   adjust to the actual value

## Configuration Reference

### Budget Limits

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_calls` | `u32` | 5 (default preset) | Maximum number of LLM calls allowed |
| `max_tokens` | `u32` | 10,000 (default preset) | Maximum LLM tokens consumed (input + output) |

### Reservation DTOs

**ReserveBudgetInput:**

| Field | Type | Description |
|-------|------|-------------|
| `execution_id` | `uuid::Uuid` | Execution identifier for correlation |
| `estimated_tokens` | `u32` | Best-effort token estimate (must be > 0) |
| `call_label` | `Option<String>` | Optional human-readable label (e.g., "classify", "extract") |

**CommitReservationInput:**

| Field | Type | Description |
|-------|------|-------------|
| `execution_id` | `uuid::Uuid` | Execution identifier |
| `call_id` | `u32` | Call identifier from the reservation |
| `actual_tokens` | `u32` | Actual tokens consumed by the LLM call |

**GetBudgetStatusOutput:**

| Field | Type | Description |
|-------|------|-------------|
| `max_calls` | `u32` | Configured call limit |
| `max_tokens` | `u32` | Configured token limit |
| `calls_used` | `u32` | Calls consumed so far |
| `tokens_used` | `u32` | Tokens consumed so far |
| `remaining_calls` | `u32` | Remaining call capacity |
| `remaining_tokens` | `u32` | Remaining token capacity |
| `call_usage_ratio` | `f64` | Fraction of calls used (0.0–1.0) |
| `token_usage_ratio` | `f64` | Fraction of tokens used (0.0–1.0) |
| `active_warnings` | `Vec<BudgetWarningInfo>` | Resources near their limits |
| `label` | `String` | Budget preset label |

## Performance Characteristics

| Metric | Target | Notes |
|--------|--------|-------|
| Reservation latency | < 1µs | Atomic counters — no locks |
| Commit latency | < 1µs | Atomic counter adjustments |
| Status query latency | < 1µs | Atomic counter reads |
| Memory per budget | ~100 bytes | BudgetState struct + CancellationToken |
| Concurrent capacity | Unlimited | Atomic operations — no lock contention |

## Health Checks

The budget module exposes health information via:

1. **`get_status()`** — returns full budget snapshot (usage, limits, warnings)
2. **`has_capacity()`** — returns whether any capacity remains
3. **`active_warnings()`** — returns list of resources crossing soft thresholds
4. **`cancel_token().is_cancelled()`** — whether budget was hard-exhausted
5. **HTTP endpoints** — `GET /api/v1/budget/status/{execution_id}`

## Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| `budget_calls_used` | `get_status()` | Current call consumption |
| `budget_tokens_used` | `get_status()` | Current token consumption |
| `budget_calls_remaining` | `remaining_calls()` | Remaining call capacity |
| `budget_tokens_remaining` | `remaining_tokens()` | Remaining token capacity |
| `budget_active_warnings` | `active_warnings()` | Number of active threshold warnings |
| `budget_exhausted` | `cancel_token().is_cancelled()` | Whether budget was fully exhausted |
| `budget_reservations_created` | Runtime counter (impl) | Total reservations made |
| `budget_reservations_rolled_back` | Runtime counter (impl) | Total auto-rollbacks on Drop |

---

*Last updated: 2026-06-14*
