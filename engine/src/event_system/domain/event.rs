//! ExecutionEvent domain enum — all 11 variants + PersistedEvent.
//!
//! @canonical .pi/architecture/modules/event-system.md#events
//! Implements: Contract Freeze — ExecutionEvent tagged union with 11 variants
//! Issue: #46
//!
//! This is the central event type for the entire execution lifecycle.
//! Every phase — planning, node execution, tool calls, completion — emits
//! an `ExecutionEvent` variant through the `EventBus`.
//!
//! # Contract (Frozen)
//! - Exactly 11 variants, no more, no less
//! - Every variant carries `execution_id` for correlation
//! - Every variant carries `timestamp` (ISO 8601 UTC)
//! - Serialized as tagged union with `#[serde(tag = "type", rename_all = "snake_case")]`
//! - No implementation logic — pure data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All possible execution events emitted during a Rigorix run.
///
/// # Variants
///
/// | # | Variant | Emitted By | Purpose |
/// |---|---------|------------|---------|
/// | 1 | `PlanningStarted` | Orchestrator | Plan generation begins |
/// | 2 | `PlanningCompleted` | Orchestrator | Plan generated successfully |
/// | 3 | `NodeStarted` | ParallelExecutor | A DAG node begins execution |
/// | 4 | `NodeCompleted` | ParallelExecutor | A DAG node finishes successfully |
/// | 5 | `NodeFailed` | ParallelExecutor | A DAG node fails (may retry) |
/// | 6 | `NodeRetrying` | ParallelExecutor | A failed node is being retried |
/// | 7 | `ToolExecuted` | ExecutionEnforcer | A tool was called (allowed or skipped) |
/// | 8 | `ExecutionCompleted` | Orchestrator | Entire execution finished successfully |
/// | 9 | `ExecutionFailed` | Orchestrator | Execution terminated with error |
/// | 10 | `ExecutionCancelled` | Orchestrator | Execution was cancelled (user/graceful) |
/// | 11 | `BudgetWarning` | ExecutionEnforcer | A resource budget threshold was hit |
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEvent {
    /// Execution plan generation has started.
    ///
    /// Emitted when the orchestrator begins generating the execution plan
    /// from the selected template and user intent.
    PlanningStarted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// The user's high-level intent / prompt.
        intent: String,
        /// ISO 8601 timestamp of when planning started.
        timestamp: DateTime<Utc>,
    },

    /// Execution plan generation completed successfully.
    ///
    /// Emitted when a valid execution plan has been produced.
    /// Carries the template ID, confidence score, and resolved parameters.
    PlanningCompleted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// The template that was selected for this execution.
        template_id: String,
        /// Model confidence score (0.0–1.0) for the generated plan.
        confidence: f64,
        /// Key-value pairs of resolved template parameters.
        parameters: HashMap<String, String>,
        /// ISO 8601 timestamp of when planning completed.
        timestamp: DateTime<Utc>,
    },

    /// A DAG node has started execution.
    ///
    /// Emitted before the node's action (tool call, sub-pipeline, script) begins.
    NodeStarted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node being executed.
        node_id: String,
        /// Human-readable name of the node.
        node_name: String,
        /// ISO 8601 timestamp of when the node started.
        timestamp: DateTime<Utc>,
    },

    /// A DAG node completed successfully.
    ///
    /// Emitted when the node's action returns without error.
    NodeCompleted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node that completed.
        node_id: String,
        /// Duration of node execution in milliseconds.
        duration_ms: u64,
        /// Structured output produced by this node.
        output: serde_json::Value,
        /// ISO 8601 timestamp of when the node completed.
        timestamp: DateTime<Utc>,
    },

    /// A DAG node failed during execution.
    ///
    /// Emitted when the node's action throws an error.
    /// May be followed by `NodeRetrying` if retries are configured.
    NodeFailed {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node that failed.
        node_id: String,
        /// Error description.
        error: String,
        /// Which attempt this was (1-indexed).
        attempt: u32,
        /// ISO 8601 timestamp of when the failure occurred.
        timestamp: DateTime<Utc>,
    },

    /// A failed node is being retried.
    ///
    /// Emitted when the executor applies the configured retry strategy
    /// and schedules a re-run of the failed node.
    NodeRetrying {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node being retried.
        node_id: String,
        /// Which retry attempt this is (1-indexed).
        attempt: u32,
        /// Delay before this retry in milliseconds.
        delay_ms: u64,
        /// ISO 8601 timestamp of when the retry was scheduled.
        timestamp: DateTime<Utc>,
    },

    /// A tool was executed (or skipped).
    ///
    /// Emitted by the ExecutionEnforcer every time a tool call is evaluated.
    /// `skipped` indicates the tool was blocked by policy or budget limits.
    ToolExecuted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node that requested the tool.
        node_id: String,
        /// Name of the tool being executed.
        tool: String,
        /// Risk level of the tool at time of execution.
        risk_level: String,
        /// Whether the tool was skipped (blocked by policy/budget).
        skipped: bool,
        /// ISO 8601 timestamp of tool execution.
        timestamp: DateTime<Utc>,
    },

    /// Execution completed successfully.
    ///
    /// Emitted when all DAG nodes have completed without unrecoverable errors.
    ExecutionCompleted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Total execution duration in milliseconds.
        duration_ms: u64,
        /// Number of DAG nodes that were executed (not skipped).
        nodes_executed: u32,
        /// ISO 8601 timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// Execution terminated with an unrecoverable error.
    ///
    /// Emitted when the execution cannot continue and is aborted.
    ExecutionFailed {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Final error detail describing why execution failed.
        error: String,
        /// ISO 8601 timestamp of the failure.
        timestamp: DateTime<Utc>,
    },

    /// Execution was cancelled (user-initiated or graceful shutdown).
    ///
    /// Emitted when the orchestrator receives a cancellation signal
    /// (e.g., Ctrl+C, shutdown request, graceful timeout) and terminates
    /// the execution before all nodes complete.
    ExecutionCancelled {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// ISO 8601 timestamp of when cancellation was processed.
        timestamp: DateTime<Utc>,
    },

    /// A resource budget warning was triggered.
    ///
    /// Emitted by the ExecutionEnforcer when a tracked resource exceeds
    /// one of its budget thresholds (e.g., token usage, tool call count,
    /// execution time). Warnings are informational — the execution continues
    /// unless the hard limit is also hit.
    BudgetWarning {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// The resource that exceeded its budget threshold
        /// (e.g., "tokens", "tool_calls", "execution_time_ms").
        resource: String,
        /// Current usage of the resource.
        used: u64,
        /// Budget limit for the resource.
        limit: u64,
        /// ISO 8601 timestamp of the warning.
        timestamp: DateTime<Utc>,
    },
}

/// A persisted execution event with a monotonic sequence number.
///
/// Wraps an `ExecutionEvent` with a globally unique, monotonically
/// increasing sequence number assigned at publish time. This enables
/// exact replay ordering and deduplication.
///
/// # Contract (Frozen)
/// - `sequence` is monotonically increasing within an `EventBus` instance
/// - `event` is the original `ExecutionEvent` with full payload
/// - Stored in `Vec<PersistedEvent>` inside `EventBus` for drain-at-end
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEvent {
    /// Monotonically increasing sequence number (1-indexed).
    pub sequence: u64,

    /// The original execution event.
    pub event: ExecutionEvent,
}
