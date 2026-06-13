# Disaster Recovery Plan: event-system Module

<!--
Canonical Reference: .pi/architecture/modules/event-system.md
Last Updated: 2026-06-13
-->

## Scope

This DR plan covers the `event-system` module — a pub-sub event bus with
synchronous in-memory persistence for execution events. The default implementation
stores events in memory only; events are lost on process crash.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless at startup — EventBusService is created fresh |
| RPO (Recovery Point Objective) | 0 (in-memory) / configurable (persistent) | Default implementation is in-memory only; persistent backend available via `PersistedEventRepository` trait |

## Backup Strategy

### Default (In-Memory)

**No backups required for the default in-memory implementation.**

Events flow through the bus in real-time:
1. Published → broadcast to subscribers → persisted in memory
2. At execution end → drained into `ExecutionRecord`
3. `ExecutionRecord` is persisted by the State Persistence module (see `dr-plan-state-persistence.md`)

The event-system acts as a transient buffer. The durable record is `ExecutionRecord`,
which is persisted separately.

### Persistent Backend (If Configured)

If a persistent `PersistedEventRepository` is implemented (e.g., SQLite, file-based):

| Data | Backup Strategy | Frequency | Retention |
|------|----------------|-----------|-----------|
| Persisted events | Full database backup | Per execution cycle | Until drained + archived |

## Restore Procedure

### Without Persistent Backend

1. **Restart application** — the EventBus is created fresh with an empty buffer
2. **Verify subscribers reconnect** — TUI, console printer, audit, etc.
3. **Verify event flow** — publish a test event and confirm subscribers receive it
4. **No data loss** — execution events are transient; durable records are in `ExecutionRecord`

### With Persistent Backend

1. **Restore database** from backup if needed
2. **Re-initialize repository** — `InMemoryEventRepository::configure()` or equivalent
3. **Re-attach to EventBus** — `EventBusFactoryImpl::create(config)`
4. **Verify event query** — `query_events()` returns expected data
5. **Verify subscriber delivery** — new events are received by all subscribers

## Failover Plan

The event-system module has no hot standby or failover mechanism. It is a
single-instance, in-process component. Key considerations:

1. **Single point of failure:** The EventBus lives inside the orchestrator process.
   If the process crashes, the in-memory buffer is lost.
2. **Mitigation:** Drain events to `ExecutionRecord` at execution end.
   `ExecutionRecord` is persisted atomically (see ADR-006: Atomic Write-Rename).
3. **Future enhancement:** A persistent `PersistedEventRepository` implementation
   (e.g., append-only log file) would provide crash recovery.

### Impact of Failures

| Failure | Impact | Mitigation |
|---------|--------|------------|
| Process crash (in-memory) | All un-drained events lost | Ensure `drain_persisted()` is called on every execution path (success, failure, cancellation) |
| Process crash (persistent) | No data loss | Events recoverable from persistent backend |
| Subscriber failure | Subscriber misses events while offline | Subscriber re-connects and receives only new events; missed events available via `query_events()` |
| Channel capacity exceeded | Slow subscriber misses oldest events | Monitor `had_laggers` in publish output; increase channel capacity |

## Monitoring and Alerting

### Health Check Endpoint

If HTTP server is configured:
- **`GET /api/v1/events/status`** — returns current event bus status

### Key Health Indicators

| Indicator | Warning | Critical | Action |
|-----------|---------|----------|--------|
| Buffer utilization | > 80% | > 95% | Increase buffer_capacity or drain more frequently |
| Subscriber lag | Any | Repeated | Investigate subscriber throughput |
| Double drain attempt | Any | Any | Fix code to drain exactly once |
| No subscribers | All events | Persistent | Verify subscriber initialization order |

## Testing the DR Plan

### Scenario 1: Process Restart

1. Start application
2. Publish events through EventBus
3. Kill process before drain
4. Restart application
5. Verify: EventBus is empty (no recovery needed for in-memory)
6. Verify: `ExecutionRecord` from State Persistence has the complete record

### Scenario 2: Subscriber Reconnection

1. Subscribe to EventBus
2. Drop the receiver
3. Publish events
4. Subscribe again
5. Verify: New subscriber receives only new events (not missed ones)
6. Verify: `query_events()` can retrieve missed events

### Scenario 3: Capacity Overflow

1. Create EventBus with small `channel_capacity` (e.g., 16)
2. Subscribe with a slow receiver
3. Publish many events rapidly
4. Verify: Fast subscriber receives all events, slow subscriber lags
5. Verify: Published events are still persisted despite subscriber lag

---

*Last updated: 2026-06-13*
