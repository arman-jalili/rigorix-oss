# Runbook: execution-engine

## Overview

The execution-engine module is responsible for executing sealed TaskGraphs from the DAG Engine. It runs at the core of the orchestration pipeline (Phase 3), consuming graphs and producing execution results with retry support.

## Startup Sequence

### Dependencies (must be available before execution-engine starts)

1. **Configuration** — loaded and validated
2. **Cancellation** — CancellationManager initialised
3. **Enforcement** — ExecutionEnforcer configured
4. **Risk Gating** — RiskClassifier initialised with tool risk mappings
5. **Failure Classification** — classify_failure() registered
6. **Event System** — EventBus initialised
7. **State Persistence** — StateManager ready for crash recovery
8. **DAG Engine** — DagGraphService available to provide sealed graphs
9. **Tool System** — ToolRegistry ready with registered tools

### Startup Procedure

```rust
// 1. Create executor config
let config = ParallelExecutorConfig::default();

// 2. Create retry evaluation service
let retry_service = Box::new(RetryEvaluationServiceImpl::new());

// 3. Create executor
let executor = ParallelExecutionServiceImpl::new(config, retry_service);

// 4. Register progress callbacks (TUI, logging)
executor.on_progress(Box::new(|progress| {
    log::info!("[{}/{}] {} → {}", 
        progress.completed_count + progress.failed_count + progress.skipped_count,
        progress.total_nodes,
        progress.node_id,
        progress.state.status.as_str());
}));
```

### Health Check

```
GET /api/v1/execution/health
→ 200 OK { status: "ok", active_executions: 0, ... }
```

The health endpoint returns:
- `status`: "ok" if the executor can accept new executions
- `active_executions`: number of in-flight executions
- `completed_executions`: total completed executions since startup
- `total_retries`: total retries across all executions
- `max_concurrent`: configured concurrency limit

## Graceful Shutdown

### Step 1: Abort active executions

```rust
// For each active execution
executor.abort_execution(AbortExecutionInput {
    dag_id: active_dag_id,
    reason: "System shutting down".to_string(),
}).await;
```

In-flight nodes are allowed to complete. No new nodes are dispatched.

### Step 2: Persist remaining state

The ExecutionResultRepository saves in-flight state for crash recovery.

### Step 3: Wait for completions

Active executions return with `cancelled: true`. Callers detect this and retry on restart.

### Step 4: Drain event bus

Execution events are flushed before shutdown.

## Common Failure Modes

### Failure Mode 1: Graph Not Sealed

**Symptom:** `ExecutionError::GraphNotSealed`
**Cause:** `execute_graph()` called without calling `seal()` on the TaskGraph first.
**Recovery:** Ensure the DAG Engine's `seal_graph()` is called before `execute_graph()`.

### Failure Mode 2: Duplicate Execution

**Symptom:** `ExecutionError::InvalidState` — "already in progress"
**Cause:** `execute_graph()` called twice for the same dag_id without the first completing.
**Recovery:** Check `get_execution_state()` first; or use a unique dag_id per execution.

### Failure Mode 3: Retry Limit Exhaustion

**Symptom:** Node returns `TaskResult { success: false, failure_type: "exhausted" }`
**Cause:** Node failed all retry attempts with no fallback configured.
**Recovery:** 
- Increase `max_attempts` in the node's RetryPolicy
- Configure a fallback node
- Set `skip_on_exhaustion: true` to continue without failing the DAG

### Failure Mode 4: Cancellation During Execution

**Symptom:** `ExecutionResult { cancelled: true }`
**Cause:** External cancellation signal received.
**Recovery:** The execution result contains `cancellation_reason` and `completed_count`. The caller should check if partial results are usable or re-execute.

### Failure Mode 5: Enforcement Rejection

**Symptom:** `ExecutionError::EnforcementRejected`
**Cause:** Enforcement limits exceeded (concurrent operations, total operations).
**Recovery:** 
- Increase `max_concurrent_executions` or `max_total_retries_per_session`
- Wait for other executions to complete
- Check Enforcement configuration

### Failure Mode 6: Pause/Resume Inconsistency

**Symptom:** `ExecutionError::InvalidState` — "already paused" or "not paused"
**Cause:** Double-pause or resume without pause.
**Recovery:** Check `get_execution_state()` before pausing/resuming.

## Configuration Reference

### ParallelExecutorConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| max_concurrent_executions | u32 | 4 | Max parallel nodes (0 = unlimited) |
| default_retry_policy | RetryPolicy | 4 attempts, exponential | Fallback policy |
| enable_cancellation | bool | true | Check cancellation |
| enable_enforcement | bool | true | Apply enforcement |
| max_total_retries_per_session | u32 | 100 | Total retries across all nodes |
| max_failures_before_abort | u32 | 0 | Abort after N failures (0 = unlimited) |
| enable_fallback | bool | true | Allow fallback execution |
| enable_validation | bool | true | Run post-execution validation |

### RetryPolicy Default

| Field | Value |
|-------|-------|
| max_attempts | 4 |
| retry_strategies | [SameOperation, SameOperation, ExpandContext] |
| backoff_strategy | Exponential(100ms, 2.0x, 30s cap) |
| enable_fallback | true |
| skip_on_exhaustion | false |

## Logging Patterns

All lifecycle events are emitted as `ExecutionEngineEvent` variants on the EventBus.
Key log points:

```rust
// Execution start
log::info!("Execution started: dag_id={}, nodes={}", dag_id, total_nodes);

// Node completion
log::debug!("Node completed: dag_id={}, node={}, duration={}ms", 
    dag_id, node_id, duration_ms);

// Failure with retry
log::warn!("Node failed (retrying): dag_id={}, node={}, attempt={}/{}", 
    dag_id, node_id, attempt, max_attempts);

// Execution complete
log::info!("Execution complete: dag_id={}, ok={}, fail={}, skip={}, duration={}ms",
    dag_id, completed, failed, skipped, duration);
```

## Metrics

Key metrics to expose (not yet instrumented — planned for observability phase):

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| execution_total | Counter | dag_id | Total executions started |
| execution_duration_ms | Histogram | dag_id | Execution duration |
| execution_completed | Counter | dag_id | Successful completions |
| execution_failed | Counter | dag_id | Failed executions |
| execution_cancelled | Counter | reason | Cancelled executions |
| node_execution_duration_ms | Histogram | node_id, status | Per-node execution time |
| retry_total | Counter | node_id | Retry attempts |
| retry_strategy | Counter | strategy | Retry strategy distribution |
| concurrent_executions | Gauge | — | Current in-flight count |
