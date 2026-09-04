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

use crate::approval::domain::DecisionContext;
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
    /// The TaskGraph to execute (sealed and validated).
    /// When None, executor produces a placeholder result.
    #[serde(skip)]
    pub graph: Option<crate::dag_engine::domain::TaskGraph>,
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
    /// Whether execution paused awaiting human approval on one or more steps.
    #[serde(default)]
    pub approval_pending: bool,
    /// Names of steps paused awaiting human approval (when `approval_pending`).
    #[serde(default)]
    pub pending_approval_steps: Vec<String>,
}

// ---------------------------------------------------------------------------
// Approve Node DTOs
// ---------------------------------------------------------------------------

/// Input for approving one or more steps of a paused execution.
///
/// Grants human sign-off for steps that declared `requires_approval: true`
/// in the plan. After approval, `resume_execution` continues the paused DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveNodeInput {
    /// The ID of the paused DAG execution.
    pub dag_id: Uuid,
    /// Step (node) names to approve.
    pub step_names: Vec<String>,

    // ── ADR-011 approval binding (optional, engine-side) ───────────────────
    // When the execution engine carries an approval binding, approving a step
    // persists a single-use ApprovalRecord bound to the exact execution
    // intent. Identity is a required captured fact (R3): absent `approver_id`
    // with binding enabled denies the step.
    /// Required for binding capture — human identity subject (see identity
    /// module). `None` keeps the legacy approved-set path when no binding is
    /// configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approver_id: Option<String>,
    /// Role / policy id (captured fact, not a judgment).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    /// R4 — what the human was shown at approval time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_context: Option<DecisionContext>,
    /// IdP token/claims presented at approval (credential-substitution check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_claims_ref: Option<String>,
}

/// Output from approving steps of a paused execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveNodeOutput {
    /// The ID of the DAG execution.
    pub dag_id: Uuid,
    /// Step names that were approved.
    pub approved: Vec<String>,
    /// Step names that could not be found in this execution.
    pub not_found: Vec<String>,
    /// Step names still awaiting approval after this call.
    pub still_pending: Vec<String>,
    /// Step names rejected by the approval gate: the node is not
    /// approval-gated (`requires_approval == false`) or is not in
    /// `AwaitingApproval` (GAP-H-07).
    #[serde(default)]
    pub denied: Vec<String>,
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
    /// Total wall-clock duration in milliseconds (from the live session result).
    #[serde(default)]
    pub total_duration_ms: u64,
    /// ISO 8601 timestamp when execution completed (None if still running/paused).
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
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

// ---------------------------------------------------------------------------
// Hydrate Execution DTOs — cross-process resume (GAP-3)
// ---------------------------------------------------------------------------

/// Input for hydrating a session from persisted state.
///
/// Used when `approve_execution` is called from a DIFFERENT process than the
/// one that paused the run: the session (dag + node states) is not in this
/// process's memory, so the orchestrator loads the persisted ExecutionState
/// (which now carries the sealed TaskGraph + node states + approved set) and
/// rebuilds the in-memory session before approving + resuming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydrateExecutionInput {
    /// The ID of the DAG execution to hydrate.
    pub dag_id: Uuid,
    /// The sealed TaskGraph that was executing when the run paused.
    pub graph: crate::dag_engine::domain::TaskGraph,
    /// Per-node states (status, durations, errors) from the paused run.
    pub node_states: std::collections::HashMap<Uuid, NodeExecutionState>,
    /// Node IDs already granted human approval.
    pub approved: std::collections::HashSet<Uuid>,
    /// The original execution start time, preserved so resumed runs report
    /// an undistorted `duration_ms` in the audit envelope.
    pub started_at: DateTime<Utc>,
}

/// Output from hydrating an execution session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydrateExecutionOutput {
    /// The hydrated dag id.
    pub dag_id: Uuid,
    /// Number of node states loaded.
    pub node_count: usize,
    /// Whether the session was created (true) or already existed (false).
    pub created: bool,
}
