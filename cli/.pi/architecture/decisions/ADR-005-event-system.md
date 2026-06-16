# ADR-005: Event System for Cross-Context Communication

**Status:** Accepted
**Date:** 2026-06-16

## Context

Multiple engine contexts (Execution Engine, Enforcement, Orchestrator) need to emit lifecycle events. Consumers (TUI, Audit Service, State Persistence, Console Printer) need to receive them. This must be non-blocking and support fan-out.

## Decision

**Use tokio broadcast channel** for the EventBus with decoupled publisher/subscriber model.

Architecture:
1. **EventBus** wraps a `tokio::sync::broadcast::Sender` with bounded capacity
2. **Producers** call `EventBus::publish(event)` with one of the 11 `ExecutionEvent` variants
3. **Consumers** subscribe and receive events via `broadcast::Receiver`
4. **In-memory log** stores all `PersistedEvent`s with monotonic sequence numbers for replay
5. **Drain-at-end** after execution completes, the log is drained for audit envelope creation and state persistence

## Key Properties

- **Non-blocking**: broadcast channel is lock-free; slow consumers miss events (lagged)
- **Fan-out**: each subscriber gets all events independently
- **Ordered**: sequence numbers guarantee replay order
- **Crash-safe**: in-memory only during execution; persistence happens via drain + atomic write

## Alternatives

| Alternative | Reason Rejected |
|-------------|----------------|
| Crossbeam channel | No built-in broadcast semantic |
| Async channel per subscriber | Manual fan-out management, harder to maintain |
| External message broker | Over-engineering for single-process CLI |
| Callbacks | Tight coupling, no fan-out |

*Affects: Event System, CLI Boundary, Audit, State Persistence*
