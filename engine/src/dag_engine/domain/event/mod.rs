//! Event payload schemas for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#events
//! Implements: Contract Freeze — DagEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the `EventBus` whenever significant DAG
//! lifecycle events occur — graph constructed, sealed, node execution,
//! cycle detected, plan compared. Consumers (orchestrator, audit, TUI)
//! subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `dag_id` correlates to the originating TaskGraph

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the DAG Engine module.
///
/// Wrapped in `ExecutionEvent::dag_event(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DagEvent {
    /// A new TaskGraph was constructed.
    ///
    /// Emitted after `TaskGraph::seal()` completes successfully.
    GraphConstructed {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// Number of nodes in the constructed graph.
        node_count: u32,
        /// ISO 8601 timestamp of construction.
        timestamp: DateTime<Utc>,
    },

    /// A cycle was detected during graph sealing.
    ///
    /// Emitted when `TaskGraph::seal()` encounters a cycle during
    /// topological sort.
    CycleDetected {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// Number of nodes successfully processed before cycle detection.
        processed_count: u32,
        /// Total number of nodes in the graph.
        total_nodes: u32,
        /// ISO 8601 timestamp of detection.
        timestamp: DateTime<Utc>,
    },

    /// A node has been queued for execution.
    ///
    /// Emitted when all dependencies of a node are satisfied and it
    /// enters the ready queue.
    NodeQueued {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that was queued.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// ISO 8601 timestamp of queueing.
        timestamp: DateTime<Utc>,
    },

    /// A node has started execution.
    ///
    /// Emitted when the executor picks up a node from the ready queue.
    NodeExecutionStarted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that started execution.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// The retry attempt number (0 = first attempt).
        attempt: u8,
        /// ISO 8601 timestamp of start.
        timestamp: DateTime<Utc>,
    },

    /// A node has completed execution successfully.
    ///
    /// Emitted when a node finishes execution and passes all
    /// validation rules.
    NodeExecutionCompleted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that completed.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// Duration of execution in milliseconds.
        duration_ms: u64,
        /// ISO 8601 timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// A node has failed execution.
    ///
    /// Emitted when a node fails. If retries remain, a retry will
    /// follow; otherwise, fallback or error propagation occurs.
    NodeExecutionFailed {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that failed.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// The failure type classification.
        failure_type: String,
        /// The error message from the failure.
        error_message: String,
        /// Number of retries remaining before permanent failure.
        retries_remaining: u8,
        /// ISO 8601 timestamp of failure.
        timestamp: DateTime<Utc>,
    },

    /// A node was retried after a failure.
    ///
    /// Emitted when the retry policy triggers a retry attempt
    /// for a failed node.
    NodeRetried {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID being retried.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// The retry attempt number (1-indexed).
        attempt: u8,
        /// The retry strategy being applied.
        strategy: String,
        /// ISO 8601 timestamp of retry.
        timestamp: DateTime<Utc>,
    },

    /// A fallback node was executed.
    ///
    /// Emitted when a node fails permanently and its configured
    /// fallback node is executed instead.
    FallbackExecuted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The original node ID that failed.
        original_node_id: Uuid,
        /// The fallback node ID being executed.
        fallback_node_id: Uuid,
        /// ISO 8601 timestamp of fallback execution.
        timestamp: DateTime<Utc>,
    },

    /// A plan comparison was performed.
    ///
    /// Emitted when two execution plans are compared for audit
    /// or approval workflows.
    PlanCompared {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// Number of nodes added in the new plan.
        added_count: u32,
        /// Number of nodes removed from the old plan.
        removed_count: u32,
        /// Number of nodes modified between plans.
        modified_count: u32,
        /// The computed impact level of the changes.
        impact_level: String,
        /// ISO 8601 timestamp of comparison.
        timestamp: DateTime<Utc>,
    },

    /// A validation rule failed for a node.
    ///
    /// Emitted when a post-execution validation rule fails,
    /// potentially triggering a retry or fallback.
    ValidationFailed {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID whose validation failed.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// The validation rule that failed.
        rule: String,
        /// Details about the validation failure.
        message: String,
        /// ISO 8601 timestamp of failure.
        timestamp: DateTime<Utc>,
    },

    /// The DAG execution has completed (all nodes done).
    ///
    /// Emitted when all nodes in the graph have reached a terminal
    /// state (Completed, Failed, or Skipped).
    DagExecutionCompleted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// Total number of nodes in the graph.
        total_nodes: u32,
        /// Number of nodes completed successfully.
        completed_count: u32,
        /// Number of nodes that failed.
        failed_count: u32,
        /// Number of nodes skipped.
        skipped_count: u32,
        /// Total execution duration in milliseconds.
        total_duration_ms: u64,
        /// ISO 8601 timestamp of completion.
        timestamp: DateTime<Utc>,
    },
}
