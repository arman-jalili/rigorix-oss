# Budget Tracking Architecture

<!--
Canonical Reference: .pi/architecture/modules/budget-tracking.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Monitors and enforces LLM token and call budgets per session with RAII reservation pattern. Every LLM call must reserve budget before invocation; unreserved budget auto-rolls back on Drop. CancellationToken coordinates budget exhaustion with the cancellation system.

## Responsibilities

- Track LLM call count and token consumption per execution
- Enforce hard caps: `max_llm_calls` and `max_llm_tokens`
- Provide RAII reservation pattern: reserve → use → commit (or auto-rollback on Drop)
- Emit cancellation via CancellationToken when budget exhausted
- Support multiple budget presets matching enforcement modes

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| LlmBudget | `rigorix/src/enforcement.rs` | Budget tracker with RAII reservation | #budget |
| LlmBudgetReservation | `rigorix/src/enforcement.rs` | RAII guard, commits or auto-rollbacks | #reservation |

---

## Component Details

### LlmBudget

**Purpose:** Track and enforce LLM usage with RAII reservation

**Implementation File:** `rigorix/src/enforcement.rs`

```rust
pub struct LlmBudget {
    max_calls: u32,
    max_tokens: u32,
    used_calls: AtomicU32,
    used_tokens: AtomicU32,
    cancel_token: CancellationToken,
}

impl LlmBudget {
    pub fn new(max_calls: u32, max_tokens: u32) -> Self;
    pub fn default_mode() -> Self;     // 5 calls, 10K tokens
    pub fn advanced_mode() -> Self;    // 20 calls, 100K tokens
    pub fn aggressive_mode() -> Self;  // 50 calls, 500K tokens
    pub fn reserve(&self, tokens: u32) -> Result<LlmBudgetReservation<'_>, LlmBudgetError>;
    pub fn cancel_token(&self) -> CancellationToken;
    pub fn calls_used(&self) -> u32;
    pub fn tokens_used(&self) -> u32;
    pub fn remaining_calls(&self) -> u32;
    pub fn remaining_tokens(&self) -> u32;
    pub fn max_calls(&self) -> u32;
    pub fn max_tokens(&self) -> u32;
}
```

### LlmBudgetReservation

**Purpose:** RAII guard that holds budget capacity

```rust
pub struct LlmBudgetReservation<'a> {
    budget: &'a LlmBudget,
    call_id: u32,
    reserved_tokens: u32,
    committed: bool,
}

impl<'a> Drop for LlmBudgetReservation<'a> {
    fn drop(&mut self) {
        if !self.committed {
            // Auto-rollback: decrement calls and tokens
            self.budget.used_calls.fetch_sub(1, ...);
            self.budget.used_tokens.fetch_sub(self.reserved_tokens, ...);
        }
    }
}

impl<'a> LlmBudgetReservation<'a> {
    pub fn commit(&mut self, actual_tokens: u32);
}
```

---

## Data Flow

```mermaid
flowchart TB
    CALL["LLM call needed"] --> RESERVE["budget.reserve
(estimated_tokens)"]
    
    RESERVE -->|Ok| RES["LlmBudgetReservation
(RAII guard)"]
    RESERVE -->|Err| EXHAUSTED{"Which limit?"]
    
    RES --> API["LLM API call"]
    API -->|success| COMMIT["reservation.commit
(actual_tokens)"]
    API -->|panic/drop| ROLLBACK["Drop rollback:
calls -1, tokens -reserved"]
    
    COMMIT --> DONE["Budget consumed
calls_used +1
tokens_used +actual"]
    
    EXHAUSTED -->|calls| CERR["MaxCallsExceeded"]
    EXHAUSTED -->|tokens| TERR["MaxTokensExceeded"]
    CERR --> WARN["Emit BudgetWarning
→ Stop planning"]
    TERR --> WARN
```

**Flow Description:**
1. Caller reserves estimated tokens before any LLM API call
2. On success: API call executes, then reservation commits actual tokens
3. On panic/drop: RAII auto-rollback decrements calls and tokens
4. On exhaustion: BudgetWarning event emitted, planning halts
```

---

## Dependencies

### Depends On
- **Enforcement**: Shares cap values from EnforcementConfig
- **Cancellation**: CancellationToken for coordinated shutdown

### Used By
- **Planning Pipeline**: Budget check in Phase 1, reservation in classify/extract
- **Template Generation**: Budget reservation before LLM call
- **Orchestrator**: Creates LlmBudget from config at execution start

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 100% | `rigorix/src/enforcement.rs` (inline tests) |

**Key Test Scenarios:**
- Reserve within limits → Ok
- Reserve exceeding max_calls → Err(MaxCallsExceeded)
- Reserve exceeding max_tokens → Err(MaxTokensExceeded)
- RAII rollback on Drop without commit → calls/tokens decremented
- Commit with different actual_tokens → adjusts correctly
- Default/Advanced/Aggressive mode presets

---

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum LlmBudgetError {
    #[error("Max LLM calls exceeded: {0}")]
    MaxCallsExceeded(String),
    #[error("Max tokens exceeded: {0}")]
    MaxTokensExceeded(String),
    #[error("Budget reservation failed: {0}")]
    ReservationFailed(String),
}
```

---

*Last updated: 2026-06-13*
*Module version: 1.0.0*
