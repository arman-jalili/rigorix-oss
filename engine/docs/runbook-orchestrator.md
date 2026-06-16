# Orchestrator Module Runbook

> **Last updated:** 2026-06-16
> **Module:** Orchestrator (`engine/src/orchestrator/`)
> **Components:** OrchestratorService, OrchestratorBuilder, ExecutionRecord

## Overview

The Orchestrator is the top-level entry point that wires the full Rigorix execution lifecycle into a single operation. It sequences: config loading → planning → TaskGraph execution → state persistence → event emission → audit envelope building.

## Startup Sequence

1. **OrchestratorBuilder** creates an `OrchestratorServiceImpl` with all dependencies:
   - `PlanningPipelineService` — for intent → plan → graph generation
   - `ParallelExecutionService` — for DAG execution
   - `StateManagerService` — for crash-safe state saves
   - `CancellationService` — for graceful/immediate shutdown
   - `EventBusService` — for event pub/sub
   - `LlmBudgetService` — for LLM call/token budget checking
   - `AuditService` (optional) — for envelope delivery
2. **Configuration** from `OrchestratorConfig`:
   - `event_buffer_capacity`: EventBus capacity (default: 10,000)
   - `audit_enabled`: Whether to send audit envelopes (default: true)
   - `execution_timeout_secs`: Full run timeout (default: 300s)
   - `planning_timeout_secs`: Planning phase timeout (default: 60s)
   - `state_persistence_timeout_secs`: State save timeout (default: 10s)
   - `save_intermediate_state`: Save state after each DAG node (default: false)
   - `propagate_cancellation`: Cancel all services on failure (default: true)

## Dependencies

| Dependency | Required | Source |
|-----------|----------|--------|
| `PlanningPipelineService` | Yes | Intent classification, parameter extraction, graph gen |
| `ParallelExecutionService` | Yes | DAG node execution |
| `StateManagerService` | Yes | Execution state persistence |
| `CancellationService` | Yes | Graceful and immediate shutdown |
| `EventBusService` | Yes | Event pub/sub and drain |
| `LlmBudgetService` | Yes | LLM call/token budget |
| `AuditService` | No | Audit envelope delivery (best-effort) |

## Lifecycle (The `run()` Method)

```
1. Generate execution_id (UUIDv7)
2. Publish PlanningStarted event
3. Run PlanningPipeline::plan_with_graph(intent, budget)
4. Publish PlanningCompleted event
5. Save initial ExecutionState (Pending)
6. Execute DAG via ParallelExecutionService
7. Save final ExecutionState (Completed/Failed)
8. Drain EventBus → build ExecutionRecord
9. Send audit envelope (best-effort)
10. Return ExecutionRecord
```

## Graceful Shutdown

1. **Cancel current execution** via `OrchestratorService::cancel()`:
   - `CancellationService::request_graceful_shutdown()` — signals all sub-services
   - `CancellationService::await_shutdown()` — waits for running tasks to finish
   - Saves Cancelled state via `StateManagerService`
2. **Drain pending events** from EventBus
3. **Return Cancelled ExecutionRecord** (if cancel was in response to user input)

## Common Failure Modes

| Failure Mode | Symptom | Recovery |
|-------------|---------|----------|
| Planning pipeline fails | `OrchestratorError::PlanningFailed` | Retry with different intent or template |
| DAG execution fails | `OrchestratorError::ExecutionFailed` | Check task_results for partial completion |
| State persistence fails | `OrchestratorError::StatePersistenceFailed` | State may be recoverable from disk |
| Cancellation fails | `OrchestratorError::CancellationFailed` | Retry cancel |
| Audit delivery fails | Logged as warning (non-fatal) | Record still returned successfully |
| Sub-service timeout | Hanging `run()` call | Cancel execution, check sub-service health |

## Monitoring

### Key Metrics

- `orchestrator.runs.total` — Total execution runs started
- `orchestrator.runs.completed` — Successful completions
- `orchestrator.runs.failed` — Failed executions
- `orchestrator.runs.cancelled` — Cancelled executions
- `orchestrator.run.duration_ms` — Execution duration (histogram)
- `orchestrator.planning.duration_ms` — Planning phase duration

### Key Logs

- `"Starting orchestrator run"` — execution_id in context
- `"Orchestrator run completed"` — execution_id + status
- `"Audit envelope sent"` — execution_id
- `"Audit delivery failed (non-fatal)"` — execution_id + error

## Configuration Reference

```rust
OrchestratorConfig {
    event_buffer_capacity: 10_000,     // usize
    audit_enabled: true,               // bool
    execution_timeout_secs: 300,       // u64
    planning_timeout_secs: 60,         // u64
    state_persistence_timeout_secs: 10, // u64
    save_intermediate_state: false,    // bool
    propagate_cancellation: true,      // bool
}
```
