# Event System

## Module Status

**Status:** ✅ Implemented — contract freeze complete, proofing scripts active
**Issues:** #331 (contract freeze), #333 (proofing), #334 (architecture readiness)
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Captures all execution events as an append-only log via tokio broadcast channel with synchronous in-memory persistence. Supports subscriber fan-out for real-time monitoring (TUI, ConsoleEventPrinter, LogFormatter) and drain-at-end for ExecutionRecord persistence.

11 `ExecutionEvent` variants cover the full execution lifecycle. The CLI's TUI subscribes to the EventBus for live rendering.

## Components

**CLI-facing:**
| Component | File (planned) | Module | Purpose |
|-----------|---------------|--------|---------|
| EventSubscriberService (trait) | `cli/src/event_system/infrastructure/service.rs` | event_system | Service trait for subscribing to EventBus events |
| EventSubscriberHandler | `cli/src/event_system/infrastructure/event_subscriber_impl.rs` | event_system | Subscribes to EventBus, feeds events to TUI render loop and console output |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| EventBus (aggregate root) | `engine/src/event_system/application/` | Pub-sub with in-memory append-only log |
| ExecutionEvent | `engine/src/event_system/domain/event.rs` | Tagged union of 11 variants (frozen) |
| PersistedEvent | `engine/src/event_system/domain/event.rs` | Event with monotonic sequence number |
| EventBusService (trait) | `engine/src/event_system/application/service.rs` | Publish, subscribe, query, drain |
| EventSystemError | `engine/src/event_system/domain/error.rs` | Typed error enum |

## Domain Events

The 11 canonical ExecutionEvent variants (all defined in engine):
1. PlanningStarted — Plan generation begins
2. PlanningCompleted — Plan generated successfully
3. NodeStarted — A DAG node begins execution
4. NodeCompleted — A DAG node finishes successfully
5. NodeFailed — A DAG node fails
6. NodeRetrying — A failed node is retried
7. ToolExecuted — A tool was called (allowed or skipped)
8. ExecutionCompleted — Entire execution finished
9. ExecutionFailed — Execution terminated with error
10. ExecutionCancelled — Execution was cancelled
11. BudgetWarning — Resource budget threshold hit

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| ExecutionEvent | One of 11 typed lifecycle events emitted during a Rigorix run. Tagged JSON union. |
| PersistedEvent | ExecutionEvent with monotonic sequence number for ordered replay. |
| EventBus | Central pub-sub channel (tokio broadcast) with in-memory append-only log. |

## Dependencies

- Depends on: `engine::event_system` (all contracts frozen)
- Used by: All other contexts (emit and consume events)
- Used by: `CLI Boundary` (TUI subscribes to EventBus)
