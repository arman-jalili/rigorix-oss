# ADR-008: RAII Budget Reservation for LLM Cost Control

**Status:** Accepted
**Date:** 2026-06-13
**Session:** 63c25384-1902-4b72-83bb-257f3f682af5

**Tech Stack:** Rust

## Context

LLM API calls are the primary cost driver in Rigorix. The system must enforce hard caps on both call count and token usage per execution. If a call completes but the response cannot be used (parse error, validation failure), the reserved budget must not leak. If a future is dropped mid-flight, the reservation must roll back.

## Decision

Use a **RAII reservation pattern** where every LLM call must reserve budget before invoking the API. The reservation auto-rolls back on Drop if not explicitly committed.

```rust
// Phase 1: Reserve
let mut reservation = budget.reserve(estimated_tokens)?;

// Phase 2: Use (API call)
let response = call_llm(&prompt).await?;
let actual_tokens = extract_token_usage(&response);

// Phase 3: Commit or let Drop auto-rollback
reservation.commit(actual_tokens);

// If call_llm() panics or is cancelled mid-flight:
// reservation drops → calls_used and tokens decremented automatically
```

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **RAII reservation (chosen)** | No leaks on panic/drop/cancel; explicit commit; simple usage pattern | Few lines of boilerplate per call site | **Chosen** |
| **Pre-check then increment** | Minimal code | Leaks on mid-flight cancellation; can't handle dropped futures | Rejected — unsafe under cancellation |
| **Post-hoc accounting** | Simple implementation (count after response) | Can exceed budget before detection; no ability to prevent overshoot | Rejected — violates hard-cap requirement |
| **Async-scoped tracking** | Leverages tokio tracing for automatic accounting | Complex implementation; not portable | Rejected — over-engineered |

## Consequences

### Positive
- Budget caps are enforced strictly (no overshoot possible)
- Dropped/cancelled futures never leak budget
- Pattern is uniform across all three LLM operations (classify, extract, generate)
- CancellationToken ties budget exhaustion to the cancellation system

### Negative
- Every call site must `let mut reservation = budget.reserve()?`
- Estimated token count may be inaccurate (commit corrects it)
- Reservation failure stops the pipeline (desired behavior, but can be surprising)

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/budget-tracking.md`
- `.pi/architecture/modules/planning-pipeline.md`
- `.pi/architecture/modules/template-generation.md`

**Files to Update:**
- `rigorix/src/enforcement.rs` — LlmBudget + LlmBudgetReservation

---

*Decision date: 2026-06-13*
