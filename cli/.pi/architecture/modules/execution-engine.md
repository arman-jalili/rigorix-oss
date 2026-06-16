# Execution Engine

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Parallel DAG execution with tokio JoinSet, configurable concurrency (default 4), per-node retry logic with exponential backoff, fallback node execution, and cancellation coordination.

Consumes sealed TaskGraph from the DAG Engine. Emits ExecutionEvents for real-time monitoring via the Event System.

## Components

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| ParallelExecutorConfig | `engine/src/execution_engine/domain/parallel_executor.rs` | `# Contract (Frozen)` |
| ExecutionResult (aggregate root) | `engine/src/execution_engine/domain/parallel_executor.rs` | Aggregate result: per-node TaskResults, summary counts |
| TaskResult | `engine/src/execution_engine/domain/parallel_executor.rs` | Result of one node: success, output, duration, retries |
| NodeExecutionState | `engine/src/execution_engine/domain/parallel_executor.rs` | Runtime lifecycle tracker: status, retries, timing |
| NodeStatus | `engine/src/execution_engine/domain/parallel_executor.rs` | Status enum: Pending, Ready, Running, Completed, Failed, Skipped |
| RetryPolicy | `engine/src/execution_engine/domain/retry.rs` | Session/node retry config: max_attempts, strategies, backoff |
| RetryStrategy | `engine/src/execution_engine/domain/retry.rs` | Retry strategy enum |
| BackoffStrategy | `engine/src/execution_engine/domain/retry.rs` | Backoff enum: Fixed, Exponential, Linear, Immediate |
| RetryDecision | `engine/src/execution_engine/domain/retry.rs` | Retry evaluation outcome |
| FailureContext | `engine/src/execution_engine/domain/retry.rs` | Failure details for retry decision |
| ExecutionError | `engine/src/execution_engine/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| NodeStarted | A DAG node begins execution | ParallelExecutor |
| NodeCompleted | A DAG node finishes successfully | ParallelExecutor |
| NodeFailed | A DAG node fails (may trigger retry) | ParallelExecutor |
| NodeRetrying | A failed node is being retried | RetryEvaluationService |
| NodeSkipped | A node was skipped via strategy | RetryEvaluationService |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| ParallelExecutorConfig | Configuration for concurrent node execution and retry behavior. |
| ExecutionResult | Aggregate result of full DAG execution with per-node results and statistics. |
| NodeExecutionState | Runtime state tracking for a single node through execution lifecycle. |
| RetryPolicy | Configuration for retry behavior: max attempts, strategies, backoff. |
| RetryDecision | Outcome of retry evaluation: Retry, Fallback, Skip, or Abort. |

## Dependencies

- Depends on: `engine::execution_engine` (all contracts frozen)
- Depends on: `DAG Engine` (consumes sealed TaskGraph)
- Depends on: `Enforcement` (gate for tool execution)
- Depends on: `Risk Gating` (tool risk classification)
- Depends on: `Cancellation` (cooperative cancellation)
- Depends on: `Failure Classification` (retry routing)
- Depends on: `Event System` (emits execution events)
- Depends on: `State Persistence` (crash recovery)
- Depends on: `Tool System` (node tool resolution)
