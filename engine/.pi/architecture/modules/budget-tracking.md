# Budget Tracking Architecture

<!--
Canonical Reference: .pi/architecture/modules/budget-tracking.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Monitors and enforces LLM token and call budgets per execution with RAII reservation
pattern. Every LLM call must reserve budget before invocation; unreserved budget
auto-rolls back on Drop. CancellationToken coordinates budget exhaustion with the
cancellation system.

## Implementation Status

| Component | Status | Issue | Files |
|-----------|--------|-------|-------|
| LlmBudget (contract) | ✅ Frozen | #68 | `src/budget_tracking/domain/budget.rs` |
| LlmBudget (impl) | ✅ Implemented | #69 | `src/budget_tracking/application/llm_budget_impl.rs` |
| LlmBudgetReservation (contract) | ✅ Frozen | #68 | `src/budget_tracking/domain/reservation.rs` |
| LlmBudgetReservation (impl) | ✅ Implemented | #70 | `src/budget_tracking/application/llm_budget_impl.rs` |
| Proofing + CI | ✅ Implemented | #71 | `.pi/scripts/ci/check_budget-tracking_*.sh` |
| Architecture Readiness | ✅ Complete | #72 | `docs/runbook-budget-tracking.md`, `docs/dr-plan-budget-tracking.md` |

## Responsibilities

- Track LLM call count and token consumption per execution
- Enforce hard caps: `max_llm_calls` and `max_llm_tokens`
- Provide RAII reservation pattern: reserve → use → commit (or auto-rollback on Drop)
- Emit cancellation via CancellationToken when budget exhausted
- Support multiple budget presets matching enforcement modes

## Architecture

```text
budget_tracking/
├── domain/                    # Value objects, errors, event schemas
│   ├── budget.rs              # LlmBudget value object (LlmBudgetService)
│   ├── reservation.rs         # LlmBudgetReservationState snapshot
│   ├── error.rs               # LlmBudgetError enum (5 variants)
│   └── event/                 # BudgetEvent payload schemas
├── application/               # Service traits, DTOs, implementations
│   ├── service.rs             # LlmBudgetService, LlmBudgetReservation traits
│   ├── factory.rs             # LlmBudgetFactory trait
│   ├── dto/mod.rs             # Input/Output DTOs (reserve, commit, status)
│   ├── llm_budget_impl.rs     # LlmBudgetImpl + LlmBudgetReservationImpl
│   └── llm_budget_factory_impl.rs  # LlmBudgetFactoryImpl
├── infrastructure/            # Persistence interfaces
│   └── repository/mod.rs      # LlmBudgetRepository trait
└── interfaces/                # HTTP API contracts
    └── http/mod.rs            # 5 REST endpoints
```

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| LlmBudget | `src/budget_tracking/domain/budget.rs` | Budget value object with limits | #budget |
| LlmBudgetImpl | `src/budget_tracking/application/llm_budget_impl.rs` | Atomic counter-based runtime budget tracker | #budget-impl |
| LlmBudgetReservationImpl | `src/budget_tracking/application/llm_budget_impl.rs` | RAII guard, commits or auto-rollbacks | #reservation |
| LlmBudgetFactoryImpl | `src/budget_tracking/application/llm_budget_factory_impl.rs` | Factory with preset and custom creation | #factory |
| LlmBudgetRepository | `src/budget_tracking/infrastructure/repository/mod.rs` | Persistence interface for budget snapshots | #repository |

---

## Component Details

### LlmBudget

**Purpose:** Track and enforce LLM usage with RAII reservation

**Domain Entity (Interface Contract):** `src/budget_tracking/domain/budget.rs`

```rust
pub struct LlmBudget {
    pub max_calls: u32,
    pub max_tokens: u32,
    pub used_calls: u32,
    pub used_tokens: u32,
    pub label: String,
}

impl LlmBudget {
    pub fn would_exceed_calls(&self) -> bool;
    pub fn would_exceed_tokens(&self, tokens: u32) -> bool;
    pub fn remaining_calls(&self) -> u32;
    pub fn remaining_tokens(&self) -> u32;
    pub fn has_capacity(&self) -> bool;
    pub fn call_usage_ratio(&self) -> f64;
    pub fn token_usage_ratio(&self) -> f64;
}
```

**Runtime Implementation:** `src/budget_tracking/application/llm_budget_impl.rs`

```rust
pub(crate) struct BudgetState {
    max_calls: u32,
    max_tokens: u32,
    next_call_id: AtomicU32,
    used_calls: AtomicU32,
    used_tokens: AtomicU32,
    calls_warning_emitted: AtomicBool,
    tokens_warning_emitted: AtomicBool,
    cancel_token: CancellationToken,
    label: String,
}
```

**Service Methods (from `LlmBudgetService` trait):**
- `reserve(input) -> Result<ReserveBudgetOutput, LlmBudgetError>` — Checks capacity, atomically increments counters, returns reservation snapshot
- `commit(input) -> Result<CommitReservationOutput, LlmBudgetError>` — Adjusts token counter, checks warnings, triggers CancellationToken on exhaustion
- `get_status(input) -> Result<GetBudgetStatusOutput, LlmBudgetError>` — Returns full budget snapshot
- `has_capacity() -> bool` — Quick check for remaining calls AND tokens
- `active_warnings() -> Vec<BudgetWarningInfo>` — Resources crossing soft threshold

**Preset Profiles (from `LlmBudgetFactoryImpl`):**

| Preset | Max Calls | Max Tokens | Label |
|--------|-----------|------------|-------|
| Default | 5 | 10,000 | "default" |
| Advanced | 20 | 100,000 | "advanced" |
| Aggressive | 50 | 500,000 | "aggressive" |
| Custom | Arbitrary | Arbitrary | User-defined |
| From EnforcementConfig | From config | From config | "enforcement" |

### LlmBudgetReservation

**Purpose:** RAII guard that holds budget capacity

**Domain Snapshot:** `src/budget_tracking/domain/reservation.rs`

```rust
pub struct LlmBudgetReservationState {
    pub call_id: u32,
    pub reserved_tokens: u32,
    pub actual_tokens: Option<u32>,
    pub committed: bool,
    pub rolled_back: bool,
}
```

**Runtime Implementation (in `llm_budget_impl.rs`):**

```rust
pub(crate) struct LlmBudgetReservationImpl {
    budget: Arc<BudgetState>,
    call_id: u32,
    reserved_tokens: u32,
    committed: AtomicBool,
    rolled_back: AtomicBool,
}
```

**RAII Behavior:**
- On `commit(actual_tokens)`: Adjusts token counter (refund if actual < reserved, charge extra if actual > reserved). Marks committed.
- On `Drop` without commit: Decrements `used_calls` by 1, `used_tokens` by `reserved_tokens`. Marks rolled_back.
- Double commit returns `LlmBudgetError::ReservationFailed`.

---

## Data Flow

```mermaid
flowchart TB
    CALL["LLM call needed"] --> RESERVE["budget.reserve
(estimated_tokens)"]
    
    RESERVE -->|Ok| RES["ReserveBudgetOutput
+ LlmBudgetReservationImpl"]
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
    CERR --> EXHAUST["CancellationToken.cancel()"]
    TERR --> EXHAUST
```

**Flow Description:**
1. Caller reserves estimated tokens before any LLM API call — counters atomically incremented
2. On success: API call executes, then reservation commits actual tokens — counter adjusted for delta
3. On panic/drop: RAII auto-rollback decrements calls and tokens — no budget leakage
4. On exhaustion: `CancellationToken.cancel()` triggered, coordinating with cancellation module
5. Soft warning threshold (80%) emits `BudgetWarningInfo` but execution continues

---

## Dependencies

### Depends On
- **Enforcement**: Shares cap values from `EnforcementConfig.execution_limits`
- **Cancellation**: `CancellationToken` for coordinated shutdown on budget exhaustion
- **Event System**: `BudgetEvent` correlated with `ExecutionEvent::BudgetWarning`

### Used By
- **Planning Pipeline**: Budget check in Phase 1, reservation in classify/extract
- **Template Generation**: Budget reservation before LLM call
- **Orchestrator**: Creates `LlmBudgetImpl` from config at execution start
- **ExecutionEnforcer**: Resource budget tracking in tool evaluation pipeline

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | ≥90% | `src/budget_tracking/application/llm_budget_impl.rs` (22 tests) |
| Unit | ≥80% | `src/budget_tracking/application/llm_budget_factory_impl.rs` (7 tests) |
| CI Proofing | All contracts | `.pi/scripts/ci/check_budget-tracking_contracts.sh` |

### Key Test Scenarios
- Reserve within limits → Ok
- Reserve exceeding max_calls → Err(MaxCallsExceeded)
- Reserve exceeding max_tokens → Err(MaxTokensExceeded)
- Reserve with zero tokens → Err(ReservationFailed)
- RAII rollback on Drop without commit → calls/tokens decremented
- Commit with different actual_tokens → adjusts correctly
- Commit with more tokens than reserved → extra charged
- Double commit → Err(ReservationFailed)
- Commit prevents rollback on Drop
- Default/Advanced/Aggressive mode presets return correct limits
- Custom factory creation
- Zero max_calls/max_tokens → Err on factory

---

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum LlmBudgetError {
    #[error("Max LLM calls exceeded: used {used}/{max}")]
    MaxCallsExceeded { used: u32, max: u32 },

    #[error("Max tokens exceeded: used {used}/{max} (requested {requested})")]
    MaxTokensExceeded { used: u32, max: u32, requested: u32 },

    #[error("Budget reservation failed: {detail}")]
    ReservationFailed { detail: String, requested_tokens: u32 },

    #[error("Budget not initialized: {detail}")]
    NotInitialized { detail: String },

    #[error("Internal budget error: {detail}")]
    Internal { detail: String },
}
```

### Error to HTTP Mapping

| Error Variant | HTTP Status | Error Code |
|---------------|-------------|-------------|
| `MaxCallsExceeded` | 429 Too Many Requests | `BUDGET_MAX_CALLS_EXCEEDED` |
| `MaxTokensExceeded` | 429 Too Many Requests | `BUDGET_MAX_TOKENS_EXCEEDED` |
| `ReservationFailed` | 400 Bad Request | `BUDGET_RESERVATION_FAILED` |
| `NotInitialized` | 503 Service Unavailable | `BUDGET_NOT_INITIALIZED` |
| `Internal` | 500 Internal Server Error | `BUDGET_INTERNAL_ERROR` |

---

## API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/v1/budget/reserve` | Reserve budget for an LLM call |
| POST | `/api/v1/budget/commit` | Commit a reservation with actual tokens |
| GET | `/api/v1/budget/status/{execution_id}` | Get current budget status |
| GET | `/api/v1/budget/presets` | List available budget presets |
| POST | `/api/v1/budget/reset` | Reset budget for an execution |

---

## CI Enforcement

| Stage # | Script | Check |
|---------|--------|-------|
| 17 | `check_budget-tracking_contracts.sh` | All 14 contracts have implementations |
| 17 | `check_budget-tracking_coverage.sh` | ≥80% coverage (≥20 tests) |

---

Last updated: 2026-06-15
*Module version: 1.1.0*
