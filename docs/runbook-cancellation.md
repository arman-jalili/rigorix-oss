# Cancellation Module Runbook

> **Last updated:** 2026-06-13
> **Module:** Cancellation (`engine/src/cancellation/`)
> **Components:** CancellationManager, ShutdownSignal

## Startup Sequence

1. **Cancellation initialization** happens at process start:
   - `CancellationManagerImpl` created with configurable graceful timeout (default: 30s)
   - `CancellationToken` created (tokio-util) for coordinated task cancellation
   - Watch channel (`tokio::sync::watch`) initialized for shutdown signal subscribers
   - Task counters (running, completed, cancelled) initialized to zero
   - Cleanup handler registry initialized (empty)
2. **Configuration** is embedded in the orchestration layer:
   - Graceful shutdown timeout
   - Parent `CancellationToken` (for child scopes)
3. **No external dependencies** — pure in-memory state management

## Dependencies

| Dependency | Required | Source |
|-----------|----------|--------|
| `tokio` | Yes | Async runtime (watch channel, mutex, sleep) |
| `tokio-util` | Yes | `CancellationToken` for coordinated cancellation |

The cancellation module has **zero external service dependencies** — all state
is purely in-memory.

## Graceful Shutdown

### Sequence

1. **Request cancellation** via `request_graceful_shutdown()` or `request_immediate_abort()`
2. **CancellationToken** is triggered — all tasks polling `.cancelled()` receive the signal
3. **Watch channel** broadcasts the `ShutdownSignal` to all subscribers
4. **Await completion** via `await_shutdown()`:
   - Polls until all running tasks reach zero
   - Respects configurable timeout
   - On timeout: returns `ShutdownTimeout` error for caller to handle
5. **Cleanup handlers** are invoked for all registered task types

```rust
// Shutdown sequence (conceptual)
let mgr = CancellationManagerImpl::new(30); // 30s graceful timeout

// Request graceful shutdown
mgr.request_graceful_shutdown(input).await?;

// Await completion with timeout
match mgr.await_shutdown(ShutdownInput {
    execution_id: "exec-1".to_string(),
    timeout_secs: 30,
    force_abort_on_timeout: true,
}).await {
    Ok(output) => info!("Shutdown complete: {:?}", output),
    Err(CancellationError::ShutdownTimeout { pending_tasks, .. }) => {
        warn!("{pending_tasks} tasks did not finish in time");
    }
    Err(e) => error!("Shutdown failed: {e}"),
}
```

### Graceful vs Immediate

| Signal | Behavior | Use Case |
|--------|----------|----------|
| `Graceful` | Running tasks finish naturally. No new tasks started. | Normal cancellation (SIGINT, user cancel) |
| `Immediate` | Tasks aborted via `JoinSet::abort()`. Cleanup handlers must run. | Emergency stop (SIGTERM, enforcement limit) |

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Duplicate cancellation | `CancellationError::AlreadyCancelling` | Check `is_cancelled()` before requesting |
| Shutdown timeout | `CancellationError::ShutdownTimeout` | Increase timeout or force-abort remaining tasks |
| Task not found | `CancellationError::TaskNotFound` | Verify task ID is registered before notifying |
| No subscribers | `CancellationError::NoSubscribers` | Ensure at least one subscriber exists on watch channel |
| Cleanup failure | Logged warning | Non-fatal — task state may be inconsistent |

## Operations

### Check cancellation status
```rust
let status = mgr.status().await;
info!("Is cancelled: {}, Running: {}, Completed: {}, Cancelled: {}",
    status.is_cancelled,
    status.running_tasks,
    status.completed_tasks,
    status.cancelled_tasks,
);
```

### Subscribe to cancellation signals
```rust
let mut rx = mgr.subscribe();
tokio::spawn(async move {
    while rx.changed().await.is_ok() {
        let signal = *rx.borrow();
        match signal {
            ShutdownSignal::Graceful => info!("Graceful shutdown requested"),
            ShutdownSignal::Immediate => info!("Immediate abort requested"),
        }
    }
});
```

### Register a task for tracking
```rust
let result = mgr.register_task(&RegisterTaskInput {
    execution_id: "exec-1".to_string(),
    task_id: "task-42".to_string(),
    description: Some("LLM call".to_string()),
}).await;

if !result.accepted {
    warn!("Task rejected — execution is already cancelled");
}
```

### Register a cleanup handler
```rust
mgr.register_cleanup_handler("file-writer", Box::new(FileCleanupHandler)).await;
```

## Configuration Reference

The cancellation module has **no external configuration** — all parameters are
set at construction time:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `graceful_timeout_secs` | u64 | 30 | Max time to wait for graceful shutdown |
| `parent_token` | CancellationToken | None | Parent token for child scope propagation |

## Key Environment Variables

None. The cancellation module has no environment variable dependencies.

## Observability

### Metrics (conceptual — to be instrumented)
- `cancellation.requests.total` — Total cancellation requests received
- `cancellation.requests.graceful` — Graceful shutdown requests
- `cancellation.requests.immediate` — Immediate abort requests
- `cancellation.tasks.running` — Currently running tasks
- `cancellation.tasks.completed` — Completed tasks
- `cancellation.tasks.cancelled` — Cancelled tasks
- `cancellation.shutdown.duration_ms` — Shutdown duration histogram
- `cancellation.shutdown.timeouts` — Shutdown timeout count

### Health Check
Propagated through the orchestrator's `/health` endpoint:
- `cancellation_healthy`: true if no in-progress shutdown timeout
- `shutdown_in_progress`: true if cancellation is active
