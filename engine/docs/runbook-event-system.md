# Runbook: event-system Module

<!--
Canonical Reference: .pi/architecture/modules/event-system.md
Last Updated: 2026-06-13
-->

## Overview

The `event-system` module provides a central pub-sub event bus for capturing all
execution events as an append-only log. It uses `tokio::sync::broadcast` for
real-time delivery to subscribers and synchronous `Mutex<Vec<PersistedEvent>>`
for in-memory persistence with monotonic sequence numbers. At execution end,
events are drained into `ExecutionRecord`.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async runtime for broadcast channel |
| chrono | Yes | Event timestamps (ISO 8601 UTC) |
| serde | Yes | Event serialization/deserialization |

### Initialization

1. Create an `EventBusConfig` with desired capacity
2. Create an `EventBusFactoryImpl`
3. Call `create_default()` or `create(config)` to get an `EventBusService`

```rust
use rigorix::event_system::application::*;

let factory = EventBusFactoryImpl;
let bus = factory.create_default().await.unwrap();
```

### Quick Start

```rust
use rigorix::event_system::application::*;
use rigorix::event_system::domain::ExecutionEvent;

let factory = EventBusFactoryImpl;
let bus = factory.create_default().await.unwrap();

// Publish events
let eid = uuid::Uuid::new_v4();
bus.publish(PublishEventInput {
    event: ExecutionEvent::new_planning_started(eid, "Build project".into()),
}).await.unwrap();

bus.publish(PublishEventInput {
    event: ExecutionEvent::new_node_started(eid, "compile".into(), "Compile source".into()),
}).await.unwrap();

// Drain all events at execution end
let drained = bus.drain_persisted(DrainPersistedInput { clear: true }).await.unwrap();
println!("Drained {} events", drained.count);
```

## Graceful Shutdown

### Normal Shutdown

1. Drain all persisted events before shutdown:
   ```rust
   let events = bus.drain_persisted(DrainPersistedInput { clear: true }).await?;
   ```
2. Events can be stored in `ExecutionRecord` for audit/replay
3. Drop the `EventBusService` — all resources are cleaned up automatically

### Forced Shutdown

If the process terminates without draining, all in-memory events are lost.
This is acceptable for a CLI tool — events are ephemeral. For production use,
implement a persistent `PersistedEventRepository` backend.

## Common Failure Modes

### Subscriber Lagged

**Symptom:** `EventSystemError::SubscriberLagged` — a subscriber is too slow
and the broadcast channel is at capacity.

**Cause:** A subscriber (e.g., audit, TUI) is not polling fast enough.

**Resolution:**
1. Verify the subscriber is actively polling with `recv()`
2. Increase `channel_capacity` in `EventBusConfig` (default: 1000)
3. Consider moving heavy processing to a separate task

### Already Drained

**Symptom:** `EventSystemError::AlreadyDrained` — `drain_persisted()` was called
twice.

**Cause:** The event bus was already drained; calling drain again is invalid.

**Resolution:**
1. Check `event_count()` before draining to see if events remain
2. Use `clear()` on the repository to reset the drain state (in-memory only)
3. Structure code to drain exactly once at execution end

### Buffer Full

**Symptom:** Old events are silently evicted when the buffer capacity is exceeded.

**Cause:** The `buffer_capacity` limit (default: 10,000) is reached and oldest events
are removed to make room for new ones.

**Resolution:**
1. Increase `buffer_capacity` in `EventBusConfig` for workloads with high event volume
2. Drain periodically during long-running executions
3. Monitor `persisted_count` via `status()` and alert if approaching capacity

### No Active Subscribers

**Symptom:** Events are published but no subscriber receives them (events are still
persisted).

**Cause:** No active broadcast receivers.

**Resolution:**
1. This is informational — events are still persisted and drainable
2. Verify subscribers are created before events are published
3. Subscribe early in the startup sequence

## Configuration Reference

### EventBusConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `channel_capacity` | `usize` | `1000` | Tokio broadcast channel capacity. Affects backpressure — slow subscribers lag if channel fills. |
| `buffer_capacity` | `usize` | `10000` | Maximum in-memory persisted events. Oldest events evicted when full. |

### Minimum Values

| Field | Minimum | Reason |
|-------|---------|--------|
| `channel_capacity` | 16 | Tokio broadcast requires at least 1 |
| `buffer_capacity` | 64 | Reasonable minimum for any workload |

## Performance Characteristics

| Metric | Target | Notes |
|--------|--------|-------|
| Publish latency | < 1µs | Synchronous Mutex write to in-memory Vec; broadcast is non-blocking |
| Subscriber fan-out | O(n) | Each subscriber receives via tokio channel — n = subscriber count |
| Drain complexity | O(m) | Returns m events as a clone of the buffer |
| Query complexity | O(b) | Linear scan of buffer (b = buffer size) — acceptable for < 100K events |
| Memory per event | ~256 bytes | Approximate, varies by variant and payload size |

## Health Checks

The event system exposes health information via:

1. **`status()` method** — returns `EventBusStatus` with persisted_count, current_sequence,
   active_subscriber_count, channel_capacity, buffer_capacity
2. **`event_count()` method** — returns total published, persisted, and drained counts
3. **HTTP endpoint** — `GET /api/v1/events/status` (if HTTP server is configured)

## Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| `events_published_total` | `event_count().total` | Total events published since bus creation |
| `events_persisted_current` | `status().persisted_count` | Current persisted event count |
| `events_drained_total` | `event_count().drained` | Total events drained |
| `active_subscribers` | `status().active_subscriber_count` | Current active subscriber count |
| `buffer_utilization_pct` | `persisted_count / buffer_capacity * 100` | Buffer fullness percentage |

---

*Last updated: 2026-06-13*
