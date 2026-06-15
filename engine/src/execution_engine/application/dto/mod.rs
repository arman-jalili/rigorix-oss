//! Data Transfer Objects for the Execution Engine module.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — DTO schemas for execution and retry operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::execution_engine::domain::{
    ExecutionResult, NodeExecutionState, NodeStatus, ParallelExecutorConfig, RetryDecision,
    RetryPolicy, TaskResult,
};

// ---------------------------------------------------------------------------
// Execute Graph DTOs
// ---------------------------------------------------------------------------

/// Input for executing a sealed TaskGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteGraphInput {
    /// The ID of the sealed graph to execute.
    pub dag_id: Uuid,
    /// Optional override for the executor configuration.
    pub config_override: Option<ParallelExecutorConfig>,
}

/// Output from executing a sealed TaskGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteGraphOutput {
    /// The aggregate execution result.
    pub result: ExecutionResult,
    /// ISO 8601 timestamp of completion.
    pub completed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Execute Node DTOs
// ---------------------------------------------------------------------------

/// Input for executing a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteNodeInput {
    /// The ID of the DAG containing the node.
    pub dag_id: Uuid,
    /// The ID of the node to execute.
    pub node_id: Uuid,
    /// The retry policy to apply (defaults to session policy if None).
    pub retry_policy: Option<RetryPolicy>,
}

/// Output from executing a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteNodeOutput {
    /// The result of the node execution.
    pub result: TaskResult,
    /// The retry decision (if the node was retried).
    pub retry_decision: Option<RetryDecision>,
}

// ---------------------------------------------------------------------------
// Get Execution State DTOs
// ---------------------------------------------------------------------------

/// Input for getting the execution state of a DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetExecutionStateInput {
    /// The ID of the DAG execution.
    pub dag_id: Uuid,
}

/// Output from getting the execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetExecutionStateOutput {
    /// The ID of the DAG execution.
    pub dag_id: Uuid,
    /// Per-node execution states.
    pub node_states: HashMap<Uuid, NodeExecutionState>,
    /// Number of completed nodes.
    pub completed_count: u32,
    /// Number of failed nodes.
    pub failed_count: u32,
    /// Number of skipped nodes.
    pub skipped_count: u32,
    /// Total number of nodes.
    pub total_nodes: u32,
    /// ISO 8601 timestamp when execution started.
    pub started_at: Option<DateTime<Utc>>,
    /// Whether the execution is paused.
    pub paused: bool,
    /// Whether the execution is complete.
    pub is_complete: bool,
}

// ---------------------------------------------------------------------------
// Pause / Resume DTOs
// ---------------------------------------------------------------------------

/// Input for pausing an in-flight execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseExecutionInput {
    /// The ID of the DAG execution to pause.
    pub dag_id: Uuid,
}

/// Output from pausing an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseExecutionOutput {
    /// The ID of the paused execution.
    pub dag_id: Uuid,
    /// Number of nodes that were in-flight when paused.
    pub in_flight_count: u32,
    /// Number of nodes remaining in the ready queue.
    pub pending_count: u32,
    /// ISO 8601 timestamp when execution was paused.
    pub paused_at: DateTime<Utc>,
}

/// Input for resuming a paused execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeExecutionInput {
    /// The ID of the DAG execution to resume.
    pub dag_id: Uuid,
}

/// Output from resuming an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeExecutionOutput {
    /// The ID of the resumed execution.
    pub dag_id: Uuid,
    /// Number of ready nodes that will be dispatched.
    pub ready_count: u32,
    /// ISO 8601 timestamp when execution was resumed.
    pub resumed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Abort Execution DTOs
// ---------------------------------------------------------------------------

/// Input for aborting an in-flight execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortExecutionInput {
    /// The ID of the DAG execution to abort.
    pub dag_id: Uuid,
    /// Reason for the abort.
    pub reason: String,
}

/// Output from aborting an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortExecutionOutput {
    /// The ID of the aborted execution.
    pub dag_id: Uuid,
    /// Number of nodes that were completed before abort.
    pub completed_count: u32,
    /// Number of nodes that were skipped due to abort.
    pub skipped_count: u32,
    /// ISO 8601 timestamp when execution was aborted.
    pub aborted_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Evaluate Retry DTOs
// ---------------------------------------------------------------------------

/// Input for evaluating whether a failed node should be retried.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateRetryInput {
    /// The failure context from the node execution.
    pub failure_context: crate::execution_engine::domain::FailureContext,
    /// The retry policy governing this node.
    pub policy: RetryPolicy,
    /// Optional fallback node ID to execute if retries exhausted.
    pub fallback_node_id: Option<Uuid>,
}

/// Output from evaluating a retry decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateRetryOutput {
    /// The retry decision.
    pub decision: RetryDecision,
    /// Whether the node has reached a terminal state.
    pub is_terminal: bool,
}

// ---------------------------------------------------------------------------
// Execution Summary DTO
// ---------------------------------------------------------------------------

/// Summary of an execution for display and listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// The DAG execution ID.
    pub dag_id: Uuid,
    /// Total number of nodes.
    pub total_nodes: u32,
    /// Number of completed nodes.
    pub completed_count: u32,
    /// Number of failed nodes.
    pub failed_count: u32,
    /// Number of skipped nodes.
    pub skipped_count: u32,
    /// Total execution duration in milliseconds.
    pub total_duration_ms: u64,
    /// Total number of retries across all nodes.
    pub total_retries: u32,
    /// ISO 8601 timestamp when execution started.
    pub started_at: Option<DateTime<Utc>>,
    /// ISO 8601 timestamp when execution completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// Whether the execution was cancelled.
    pub cancelled: bool,
    /// Whether the execution is complete.
    pub is_complete: bool,
    /// Whether the execution is paused.
    pub paused: bool,
}

impl ExecutionSummary {
    /// Create an ExecutionSummary from an ExecutionResult.
    pub fn from_result(result: &ExecutionResult) -> Self {
        Self {
            dag_id: result.dag_id,
            total_nodes: result.total_nodes,
            completed_count: result.completed_count,
            failed_count: result.failed_count,
            skipped_count: result.skipped_count,
            total_duration_ms: result.total_duration_ms,
            total_retries: result.total_retries,
            started_at: Some(result.started_at),
            completed_at: Some(result.completed_at),
            cancelled: result.cancelled,
            is_complete: true,
            paused: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Node Execution State DTO (API-facing)
// ---------------------------------------------------------------------------

/// API-facing representation of a node's execution state.
///
/// Mirrors NodeExecutionState but without domain-internal fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateSummary {
    /// The node UUID.
    pub node_id: Uuid,
    /// The node's name.
    pub node_name: String,
    /// Current lifecycle status.
    pub status: NodeStatus,
    /// Number of retry attempts made.
    pub retry_attempts: u8,
    /// Duration of the last execution attempt in milliseconds.
    pub last_duration_ms: Option<u64>,
    /// Error message from the last failure (if failed).
    pub last_error: Option<String>,
    /// ISO 8601 timestamp when the node entered Ready state.
    pub ready_at: Option<DateTime<Utc>>,
    /// ISO 8601 timestamp of the most recent execution start.
    pub started_at: Option<DateTime<Utc>>,
    /// ISO 8601 timestamp when the node reached a terminal state.
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<NodeExecutionState> for NodeStateSummary {
    fn from(state: NodeExecutionState) -> Self {
        Self {
            node_id: state.node_id,
            node_name: state.node_name,
            status: state.status,
            retry_attempts: state.retry_attempts,
            last_duration_ms: state.last_duration_ms,
            last_error: state.last_error,
            ready_at: state.ready_at,
            started_at: state.started_at,
            completed_at: state.completed_at,
        }
    }
}
