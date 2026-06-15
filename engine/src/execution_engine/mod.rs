//! Execution Engine — Parallel DAG execution with configurable retry logic.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: ParallelExecutor, RetryPolicy, RetryEvaluationService, ExecutionResult
//! Issue: issue-contract-freeze
//!
//! The Execution Engine consumes sealed TaskGraphs from the DAG Engine and
//! executes them using a concurrent worker pool (tokio JoinSet). It handles:
//!
//! 1. **Parallel Execution** — Dequeue ready nodes and dispatch them to workers
//!    up to the configurable concurrency limit.
//! 2. **Retry Logic** — Per-node and per-session retry policies with multiple
//!    strategies (SameOperation, ExpandContext, SkipAndContinue, etc.) and
//!    configurable backoff (Fixed, Exponential, Linear, Immediate).
//! 3. **Cancellation** — Cooperative cancellation via CancellationToken.
//! 4. **Enforcement** — Limits on concurrency, total operations, and failures.
//! 5. **Fallback** — Execute a fallback node when retries are exhausted.
//! 6. **Event Emission** — Execution lifecycle events for observability.
//!
//! # Dependencies
//!
//! - `dag_engine` (consumes sealed TaskGraphs)
//! - `cancellation` (CancellationToken for graceful shutdown)
//! - `enforcement` (ExecutionEnforcer for operation limits)
//! - `risk_gating` (RiskClassifier for tool execution gates)
//! - `tool_system` (Tool trait for executing node tool bindings)
//! - `failure_classification` (FailureType classification)
//! - `event_system` (EventBus for execution event emission)
//! - `state_persistence` (ExecutionState for crash recovery)
//!
//! # Architecture
//!
//! - `domain/`: Core entities (ParallelExecutorConfig, NodeExecutionState,
//!   ExecutionResult, TaskResult, NodeStatus, RetryPolicy, RetryStrategy,
//!   BackoffStrategy, RetryDecision, FailureContext)
//! - `application/`: Service traits (ParallelExecutionService,
//!   RetryEvaluationService), DTOs, factory interfaces
//! - `infrastructure/`: Repository interfaces for execution result persistence
//! - `interfaces/`: HTTP API contracts for execution management
//!
//! Contracts defined in issue-contract-freeze are frozen.
//! Implementation satisfies those contracts.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub(crate) mod tests;
