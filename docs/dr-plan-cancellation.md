# Cancellation Module DR Plan

> **Last updated:** 2026-06-13
> **Module:** Cancellation (`engine/src/cancellation/`)
> **RTO:** < 1 second (pure in-memory — instant recovery on process restart)
> **RPO:** 0 (no persistent state)

## System State

The cancellation module maintains **ephemeral in-memory state only**.
No persistent storage is used. All state is lost on process restart.

| State Type | Storage | Persistence | Recovery |
|-----------|---------|------------|----------|
| CancellationToken | In-memory (tokio-util) | Lost on restart | Fresh token created |
| Shutdown signal | In-memory (watch channel) | Lost on restart | Initialized to `None` |
| Task counters | In-memory (AtomicU32) | Lost on restart | Reset to zero |
| Cleanup handlers | In-memory (Vec) | Lost on restart | Re-registered during init |
| Request timestamp | In-memory (Instant) | Lost on restart | None at startup |

## Backup Strategy

| Asset | Frequency | Method | Retention |
|-------|-----------|--------|-----------|
| None | N/A | All state is ephemeral | N/A |

**No backup is needed** — the cancellation module has zero persistent state.
On restart, a fresh `CancellationManagerImpl` is created with default state.

## Restore Procedure

### Scenario 1: Process restart (normal)
1. **Start the process** — cancellation module initializes with fresh state
2. **CancellationToken** is created in uncancelled state
3. **Watch channel** initialized with `None` (no signal)
4. **Task counters** reset to zero
5. **Cleanup handlers** must be re-registered during module initialization

```rust
// Recovery is automatic on restart:
let mgr = CancellationManagerImpl::new(30);
// Re-register cleanup handlers:
mgr.register_cleanup_handler("file-writer", Box::new(FileCleanupHandler)).await;
mgr.register_cleanup_handler("network-conn", Box::new(NetworkCleanupHandler)).await;
```

### Scenario 2: Shutdown timeout during graceful shutdown
1. `CancellationError::ShutdownTimeout` is returned
2. **System reaction**: orchestrator can retry with longer timeout or force-abort
3. **Force-abort**: set `force_abort_on_timeout: true` in `ShutdownInput`
4. **After force-abort**: tasks that check `CancellationToken` will exit
5. **Cleanup**: remaining cleanup handlers still run

```rust
// Retry with force-abort
let result = mgr.await_shutdown(ShutdownInput {
    execution_id: "exec-1".to_string(),
    timeout_secs: 10,          // shorter timeout after initial failure
    force_abort_on_timeout: true,
}).await;

// If still failing, escalate
if result.is_err() {
    error!("Shutdown failed — manual intervention needed");
    // Possible actions:
    // 1. Log all running task IDs for investigation
    // 2. Hard-kill the process if needed
    // 3. Investigate why tasks are not responding to cancellation
}
```

### Scenario 3: Duplicate cancellation request
1. `CancellationError::AlreadyCancelling` is returned
2. **System reaction**: this is a soft error — cancellation is already in progress
3. **Resolution**: check `is_cancelled()` before requesting, or catch the error

## Failover Plan

### Single-instance recovery
1. Cancellation is inherently single-instance (per-process)
2. On failure, process restart creates fresh cancellation state
3. Running tasks from previous instance are orphaned — orchestrator handles cleanup
4. Watch channel subscribers must re-subscribe after restart

### Multi-instance deployments
1. Each instance has its own independent cancellation manager
2. No cross-instance cancellation coordination (by design)
3. External orchestration (e.g., Kubernetes) handles process-level cancellation
4. OS signals (SIGINT/SIGTERM) are handled per-instance

## Monitoring

### Key metrics
- **Cancellation requests**: Rate of graceful and immediate cancellation requests
- **Task state**: Running / completed / cancelled task counts
- **Shutdown duration**: Time from request to completion
- **Shutdown timeouts**: Count of failed graceful shutdowns
- **Cleanup failures**: Count of failed cleanup handler invocations

### Health check
The cancellation module reports its health through the orchestrator:
```
Health status:
  cancellation_healthy: true (no in-progress timeout)
  shutdown_in_progress: false (no active cancellation)
```

## RTO/RPO

| Metric | Target | Notes |
|--------|--------|-------|
| RTO | < 1 second | Pure in-memory — instant recovery on process restart |
| RPO | 0 | No persistent state — zero data loss exposure |

The cancellation module has the **best possible** RTO/RPO profile since it
maintains no persistent state and recovers instantly on process restart.
The only operational risk is orphaned tasks from a crashed process, which
the orchestrator handles through its own lifecycle management.
