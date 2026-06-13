# Execution Engine Architecture

<!--
Canonical Reference: .pi/architecture/modules/execution-engine.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Executes compiled DAGs with task scheduling via tokio JoinSet, concurrency control, retry logic with exponential backoff/jitter, and result collection. Drives the actual task execution lifecycle: start → run → retry (if needed) → complete/fail.

## Responsibilities

- Execute TaskGraph nodes in topological order with configurable parallelism
- Manage per-node retry loops with exponential backoff and ±25% jitter
- Check ExecutionEnforcer limits before each retry
- Propagate CancellationToken for coordinated shutdown
- Emit ExecutionEvents (NodeStarted, NodeCompleted, NodeFailed, NodeRetrying)
- Track per-node duration, attempts, and output
- Report results back to Orchestrator for state persistence

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| ParallelExecutor | `rigorix/src/dag/executor.rs` | Top-level DAG executor with JoinSet | #executor |
| TaskExecutor | `rigorix/src/dag/executor.rs` | Per-node execution with retry loop | #task-exec |
| RetryStrategy | `rigorix/src/retry.rs` | Retry strategy enum + backoff calculator | #retry |

---

## Component Details

### ParallelExecutor

**Purpose:** Schedule and run all DAG nodes concurrently respecting dependencies

**Implementation File:** `rigorix/src/dag/executor.rs`

**Interface:**

```rust
pub struct ParallelExecutor { /* registry, symbol_graph, enforcer, event_bus, risk_config, repo_root */ }

impl ParallelExecutor {
    pub fn new() -> Self;
    pub fn with_registry(self, registry: Arc<ToolRegistry>) -> Self;
    pub fn with_symbol_graph(self, graph: SharedSymbolGraph) -> Self;
    pub fn with_enforcer(self, enforcer: Arc<ExecutionEnforcer>) -> Self;
    pub fn with_event_bus(self, bus: Arc<EventBus>) -> Self;
    pub fn with_risk_config(self, config: Arc<RiskConfig>) -> Self;
    pub fn with_repo_root(self, root: PathBuf) -> Self;
    pub async fn execute(
        &self, graph: &mut TaskGraph, cancel: &CancellationToken, execution_id: Option<Uuid>
    ) -> Result<Vec<TaskResult>, CoreOrchestratorError>;
}
```

### Retry Logic

The executor implements an **inline retry loop** per node (not using `execute_with_retry`) because it must:
1. Interact with the task graph (fallback node dispatch)
2. Signal replanning on build/test failures
3. Track retries via ExecutionEnforcer

Backoff calculation is shared via `calculate_backoff(attempt, base_delay_ms, jitter_percent)`:

```rust
pub fn calculate_backoff(attempt: u8, base_delay_ms: u64, jitter_percent: f64) -> Duration;
// exponential: base_delay_ms * 2^attempt
// jitter: ±jitter_percent% of exponential delay
```

---

## Data Flow

```mermaid
flowchart TB
    IN["TaskGraph + CancellationToken"] --> Q["Ready Queue
(topological order)"]
    Q --> N["For each ready node
(up to max_concurrency)"]
    
    N --> S1["Emit NodeStarted"]
    S1 --> S2["Execute tool
via ToolRegistry"]
    S2 --> S3["Emit ToolExecuted"]
    
    S3 --> CHECK{"Success?"]
    CHECK -->|yes| DONE["Emit NodeCompleted
mark_completed
check next ready"]
    CHECK -->|no| CLASS["Classify FailureType"]
    
    CLASS --> RETRYABLE{"Retryable?"]
    RETRYABLE -->|yes & can_retry| BACKOFF["calculate_backoff
sleep + jitter
Emit NodeRetrying"]
    BACKOFF --> S2
    
    RETRYABLE -->|exhausted| FAIL["Emit NodeFailed
check fallback"]
    RETRYABLE -->|no| FAIL
```

**Flow Description:**
1. Ready queue yields nodes whose dependencies are all satisfied
2. Each node executes its tool via ToolRegistry with risk gate check
3. On success: emit NodeCompleted, mark node done, advance ready queue
4. On failure: classify FailureType, check ExecutionEnforcer for retry permission
5. If retryable: calculate exponential backoff with jitter, sleep, retry
6. If exhausted or non-retryable: emit NodeFailed, execute fallback if configured
```

---

## Dependencies

### Depends On
- **DAG Engine**: Consumes TaskGraph, uses ExecutionPolicy
- **Tool System**: ToolRegistry for tool execution
- **Risk Gating**: RiskConfig for tool gating
- **Enforcement**: ExecutionEnforcer for cap checking
- **Event System**: EventBus for event emission
- **Cancellation**: CancellationToken for shutdown
- **Failure Classification**: FailureType for retry routing

### Used By
- **Orchestrator**: Orchestrator::run() invokes executor

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90% | `rigorix/src/dag/executor.rs` |

**Key Test Scenarios:**
- Execute simple linear DAG → all nodes complete in order
- Execute parallel DAG → concurrent node execution
- Node with retry → succeeds on retry
- Node exhausts retries → NodeFailed, fallback executed

---

## Performance Considerations

| Metric | Target | Monitoring |
|--------|--------|------------|
| Node scheduling overhead | < 1ms per node | Tracing spans |
| Max concurrency | Configurable via ExecutionEnforcer | Runtime config |

---

*Last updated: 2026-06-13*
*Module version: 1.0.0*
