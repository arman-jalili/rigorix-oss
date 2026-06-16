# Execution Engine Architecture

<!--
Canonical Reference: .pi/architecture/modules/execution-engine.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Executes sealed TaskGraphs from the DAG Engine using a concurrent worker pool (tokio JoinSet). Handles parallel node dispatch respecting dependency ordering, per-node retry loops with configurable strategies, cooperative cancellation, enforcement limits, and execution lifecycle events.

## Responsibilities

- Parallel node execution: dequeue ready nodes, dispatch to workers up to concurrency limit
- Inline retry loop per node: SameOperation → ExpandContext → AlternateApproach escalation
- Configurable backoff: Fixed, Exponential, Linear, Immediate
- Session management: execute, pause, resume, abort
- Cancellation integration: cooperative shutdown via CancellationToken
- Enforcement integration: concurrency limits, max failures before abort
- Fallback execution: execute fallback node when retries exhausted
- Execution event emission: 11 event types for observability
- Progress callbacks: per-terminal-state notifications for TUI

## Dependencies

- `dag_engine` — consumes sealed TaskGraphs
- `cancellation` — CancellationToken for graceful shutdown
- `enforcement` — ExecutionEnforcer for operation limits
- `risk_gating` — RiskClassifier for tool execution gates
- `tool_system` — Tool trait for executing node tool bindings
- `failure_classification` — FailureType classification
- `event_system` — EventBus for execution event emission
- `state_persistence` — ExecutionState for crash recovery

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| ParallelExecutorConfig | `engine/src/execution_engine/domain/parallel_executor.rs` | Global executor configuration (concurrency, retry defaults, enforcement) | #config |
| NodeExecutionState | `engine/src/execution_engine/domain/parallel_executor.rs` | Per-node lifecycle tracking (Pending → Ready → Running → Terminal) | #node-state |
| NodeStatus | `engine/src/execution_engine/domain/parallel_executor.rs` | Status enum: Pending, Ready, Running, Completed, Failed, Skipped | #node-status |
| TaskResult | `engine/src/execution_engine/domain/parallel_executor.rs` | Single node execution result with output, duration, retry count | #task-result |
| ExecutionResult | `engine/src/execution_engine/domain/parallel_executor.rs` | Aggregate DAG execution result with summary statistics | #exec-result |
| RetryPolicy | `engine/src/execution_engine/domain/retry.rs` | Per-node retry config: max_attempts, strategies, backoff, skip conditions | #retry-policy |
| RetryStrategy | `engine/src/execution_engine/domain/retry.rs` | Strategy enum: SameOperation, ExpandContext, SimplifyOperation, AlternateApproach, SkipAndContinue | #retry-strategy |
| BackoffStrategy | `engine/src/execution_engine/domain/retry.rs` | Backoff enum: Fixed, Exponential, Linear, Immediate | #backoff |
| RetryDecision | `engine/src/execution_engine/domain/retry.rs` | Decision enum: Retry, Fallback, Skip, Abort | #retry-decision |
| FailureContext | `engine/src/execution_engine/domain/retry.rs` | Failure details for retry decision-making | #failure-context |
| ExecutionError | `engine/src/execution_engine/domain/error.rs` | Error enum: 10 variants with structured context | #errors |
| ExecutionEngineEvent | `engine/src/execution_engine/domain/event/mod.rs` | Event enum: 11 lifecycle event payloads | #events |
| ParallelExecutionService | `engine/src/execution_engine/application/service.rs` | Service trait: execute, pause, resume, abort, progress callbacks | #exec-service |
| RetryEvaluationService | `engine/src/execution_engine/application/service.rs` | Service trait: evaluate_retry, compute_backoff, validate_policy, decide | #retry-service |
| ParallelExecutionServiceImpl | `engine/src/execution_engine/application/service_impl.rs` | JoinSet-based concurrent executor with session management | #exec-impl |
| RetryEvaluationServiceImpl | `engine/src/execution_engine/application/service_impl.rs` | Stateless retry decision engine with strategy escalation | #retry-impl |
| ParallelExecutionFactory | `engine/src/execution_engine/application/factory.rs` | Factory trait for executor construction | #exec-factory |
| RetryEvaluationFactory | `engine/src/execution_engine/application/factory.rs` | Factory trait for retry service construction | #retry-factory |
| ExecutionResultRepository | `engine/src/execution_engine/infrastructure/repository/mod.rs` | Repository trait for execution result persistence | #exec-repo |
| RetryDecisionRepository | `engine/src/execution_engine/infrastructure/repository/mod.rs` | Repository trait for retry decision audit trail | #retry-repo |

---

## Component Details

### ParallelExecutorConfig

**Purpose:** Controls executor behaviour: concurrency, retry defaults, cancellation/enforcement integration

**Implementation File:** `engine/src/execution_engine/domain/parallel_executor.rs`

**Dependencies:**
- RetryPolicy (default_retry_policy fallback)

**Configuration Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| max_concurrent_executions | u32 | 4 | Max nodes executing concurrently (0 = unlimited) |
| default_retry_policy | RetryPolicy | 4 attempts, exponential backoff | Fallback when node has no policy |
| enable_cancellation | bool | true | Check cancellation signals |
| enable_enforcement | bool | true | Apply enforcement limits |
| max_total_retries_per_session | u32 | 100 | Total retries across all nodes |
| max_failures_before_abort | u32 | 0 (unlimited) | Abort after N failures |
| enable_fallback | bool | true | Execute fallback nodes |
| enable_validation | bool | true | Run post-execution validation |

---

### ParallelExecutionService

**Purpose:** Parallel DAG execution with session management

**Implementation File:** `engine/src/execution_engine/application/service.rs` (trait)
`engine/src/execution_engine/application/service_impl.rs` (implementation)

**Methods:**

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| execute_graph | ExecuteGraphInput | ExecuteGraphOutput | Execute a sealed TaskGraph end-to-end |
| execute_node | ExecuteNodeInput | ExecuteNodeOutput | Execute single node with inline retry loop |
| get_execution_state | GetExecutionStateInput | GetExecutionStateOutput | Poll in-flight execution state |
| pause_execution | PauseExecutionInput | PauseExecutionOutput | Pause dispatch (in-flight nodes complete) |
| resume_execution | ResumeExecutionInput | ResumeExecutionOutput | Resume paused execution |
| abort_execution | AbortExecutionInput | AbortExecutionOutput | Cancel execution, skip remaining nodes |
| on_progress | callback | — | Register terminal-state notification callback |

---

### Inline Retry Loop

The retry loop is embedded directly in `execute_node()`, not a separate wrapper:

```
for attempt in 0..policy.max_attempts:
    1. Check skip conditions (if policy.has_skip_conditions)
    2. Check cancellation signal
    3. Execute node (ToolSystem)
    4. If success → return TaskResult
    5. Build FailureContext (failure_type, error, attempt, max_attempts)
    6. Evaluate retry via RetryEvaluationService.decide()
    7. Match decision:
       - Retry → compute backoff, sleep, loop
       - Fallback → execute fallback node, return
       - Skip → return skipped TaskResult
       - Abort → return aborted TaskResult
return exhausted TaskResult  (all attempts used)
```

---

### Retry Policy Defaults

```rust
RetryPolicy {
    max_attempts: 4,
    retry_strategies: [SameOperation, SameOperation, ExpandContext],
    backoff: Exponential(100ms base, 2.0x multiplier, 30s cap),
    enable_fallback: true,
    skip_on_exhaustion: false,
    retryable_failures: [],  // all failures retriable
}
```

---

### DTOs

| DTO | Input/Output | Fields |
|-----|:-----------:|--------|
| ExecuteGraphInput | Input | dag_id, config_override |
| ExecuteGraphOutput | Output | result, completed_at |
| ExecuteNodeInput | Input | dag_id, node_id, retry_policy |
| ExecuteNodeOutput | Output | result, retry_decision |
| GetExecutionStateInput | Input | dag_id |
| GetExecutionStateOutput | Output | dag_id, node_states, counts, paused, is_complete |
| PauseExecutionInput | Input | dag_id |
| PauseExecutionOutput | Output | dag_id, in_flight_count, pending_count, paused_at |
| ResumeExecutionInput | Input | dag_id |
| ResumeExecutionOutput | Output | dag_id, ready_count, resumed_at |
| AbortExecutionInput | Input | dag_id, reason |
| AbortExecutionOutput | Output | dag_id, completed_count, skipped_count, aborted_at |
| EvaluateRetryInput | Input | failure_context, policy, fallback_node_id |
| EvaluateRetryOutput | Output | decision, is_terminal |

---

### Execution Events

| Event | Fields | When Emitted |
|-------|--------|-------------|
| ExecutionStarted | dag_id, total_nodes | execute_graph called |
| NodeExecutionStarted | dag_id, node_id, node_name, attempt | Node dispatched to worker |
| NodeExecutionCompleted | dag_id, node_id, node_name, duration_ms | Node succeeds |
| NodeExecutionFailed | dag_id, node_id, node_name, failure_type, error, retries_remaining | Node fails |
| NodeRetried | dag_id, node_id, node_name, attempt, strategy, backoff_ms | Retry triggered |
| FallbackExecuted | dag_id, original_node_id, fallback_node_id, fallback_node_name | Fallback dispatched |
| NodeSkipped | dag_id, node_id, node_name, reason | Node skipped |
| DependencyResolutionConflict | dag_id, node_id, node_name, unsatisfied_deps | Dependency issue |
| ExecutionCompleted | dag_id, total_nodes, completed/failed/skipped counts, total_duration_ms | All nodes terminal |
| ExecutionCancelled | dag_id, completed_count, remaining_count, reason | Cancellation received |

---

### HTTP API Endpoints

| Method | Path | Request | Response | Description |
|--------|------|---------|----------|-------------|
| POST | /api/v1/execution/graphs/{id}/execute | ExecuteRequest | 202 ExecuteResponse | Start execution |
| GET | /api/v1/execution/graphs/{dag_id}/state | — | ExecutionStateResponse | Get execution state |
| GET | /api/v1/execution/graphs/{dag_id}/nodes | ?status= | NodeStatesResponse | Get per-node states |
| POST | /api/v1/execution/graphs/{dag_id}/pause | — | PauseResponse | Pause execution |
| POST | /api/v1/execution/graphs/{dag_id}/resume | — | ResumeResponse | Resume execution |
| POST | /api/v1/execution/graphs/{dag_id}/abort | AbortRequest | AbortResponse | Abort execution |
| GET | /api/v1/execution/history | ?limit=&offset= | ExecutionHistoryResponse | List execution history |
| GET | /api/v1/execution/graphs/{dag_id}/result | — | full ExecutionResult | Get completed result |
| GET | /api/v1/execution/health | — | HealthResponse | Health check |

### Error Codes

| Error Code | HTTP Status | Description |
|------------|-------------|-------------|
| EXEC_NOT_FOUND | 404 | Execution not found |
| EXEC_GRAPH_NOT_SEALED | 400 | Graph not sealed |
| EXEC_ALREADY_RUNNING | 409 | Execution in progress |
| EXEC_ALREADY_COMPLETED | 409 | Already completed |
| EXEC_NOT_RUNNING | 400 | Not running |
| EXEC_CANCELLED | 200 | Was cancelled |
| EXEC_ABORTED | 200 | Was aborted |
| EXEC_ENFORCEMENT_LIMIT | 429 | Enforcement limit exceeded |
| EXEC_INTERNAL_ERROR | 500 | Internal error |

---

## Module Registration

The `execution_engine` module is registered in `engine/src/lib.rs` and declares four sub-modules:

- `domain` — entities, errors, events
- `application` — service traits, DTOs, factories, implementations
- `infrastructure` — repository interfaces
- `interfaces` — HTTP API contracts

---

## Testing

45 unit tests covering:
- 28 contract compliance tests (domain entity construction, serialization, lifecycle)
- 17 implementation tests (service operations, retry decisions, factories)

Proofing scripts in `.pi/scripts/ci/`:
- `check_execution-engine_contracts.sh` — 43 contract validation points
- `check_execution-engine_coverage.sh` — 12 entities + 12 operations coverage check

CI stage: stage 25 — `execution-engine_proofing` in `run_hardening_stages.sh`

---

**Status:** Implemented  
**Last verified:** 2026-06-15  
**Module version:** 1.0.0
