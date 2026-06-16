# Budget Tracking

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Monitors and enforces LLM token and call budgets per execution with RAII reservation pattern. Every LLM call must reserve budget before invocation; unreserved budget auto-rolls back on Drop. Coordinates with cancellation system for budget exhaustion.

The CLI surfaces budget warnings in the TUI via EventBus `BudgetWarning` events.

## Components

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| LlmBudget (aggregate root) | `engine/src/budget_tracking/domain/budget.rs` | `# Contract (Frozen)` |
| LlmBudgetReservation | `engine/src/budget_tracking/domain/reservation.rs` | RAII guard entity |
| LlmBudgetService (trait) | `engine/src/budget_tracking/application/service.rs` | Budget tracking service |
| LlmBudgetError | `engine/src/budget_tracking/domain/error.rs` | Typed error enum |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| LlmBudget | Aggregate root for token and call budget tracking with usage tracking and thresholds. |
| LlmBudgetReservation | RAII guard: reserves budget on creation, auto-returns on Drop. Ensures atomic consumption. |
| RAIIReservation | Rust idiom where budget reservations auto-release on Drop, preventing resource leaks. |

## Dependencies

- Depends on: `engine::budget_tracking` (all contracts frozen)
- Depends on: `Cancellation` (coordinated budget exhaustion)
- Used by: `Planning Pipeline` (budget pre-check phase)
- Used by: `Enforcement` (budget enforcement)
