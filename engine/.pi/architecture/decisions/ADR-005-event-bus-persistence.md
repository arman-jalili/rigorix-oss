# ADR-005: Event Bus with Synchronous In-Memory Persistence

**Status:** Accepted
**Date:** 2026-06-13
**Session:** 63c25384-1902-4b72-83bb-257f3f682af5

**Tech Stack:** Rust

## Context

Rigorix must provide full execution observability: every planning step, node execution, tool call, and state transition must be observable. Events must be available to multiple subscribers (console printer, TUI, audit) and persistable into ExecutionRecord at execution end.

## Decision

Use **tokio::sync::broadcast** for fan-out delivery with **synchronous in-memory Mutex-based persistence**.

```rust
pub struct EventBus {
    sender: broadcast::Sender<ExecutionEvent>,       // Real-time delivery
    persisted: Arc<Mutex<Vec<PersistedEvent>>>,       // Synchronous persistence
    sequence: AtomicU64,                               // Monotonic ordering
}
```

Events are:
1. Broadcast to all active subscribers via tokio channel (non-blocking)
2. Synchronously persisted to `Vec<PersistedEvent>` via std::sync::Mutex (no tokio spawn)
3. Drained at execution end into ExecutionRecord

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **broadcast + Mutex (chosen)** | Simple, synchronous, no spawn needed; deterministic ordering; non-blocking publish | Mutex contention under extreme load; buffer subject to capacity | **Chosen** |
| **tokio::sync::broadcast + channel persistence** | Fully async | Events may be lost if subscriber not polled; ordering complexity; spawn overhead | Rejected — need synchronous guarantee |
| **File-based append log** | Crash-recoverable | I/O overhead per event; complex buffering | Rejected — over-engineered for current needs |
| **SQLite persistence** | Queryable, durable | Heavy dependency; setup cost | Rejected — future consideration |

## Consequences

### Positive
- Events are never dropped on the floor (always persisted)
- Monotonic sequence numbers enable exact replay ordering
- Multiple subscribers see all events from subscription point
- `drain_persisted()` at end produces complete, ordered record
- No tokio::spawn needed for persistence (sync Mutex is sufficient)

### Negative
- In-memory only — events lost on process crash (acceptable for CLI tool)
- Mutex lock contention under high event throughput (>10K events in tight loop)
- Buffer capacity limit (default 1,000; configurable)

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/event-system.md`
- `.pi/architecture/modules/state-persistence.md`

**Files to Update:**
- `rigorix/src/event_bus.rs` — EventBus with broadcast + persisted vec

---

*Decision date: 2026-06-13*
