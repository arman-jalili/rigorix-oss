//! Data Transfer Objects for the State Persistence module.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — DTO schemas for state persistence operations
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
use uuid::Uuid;

use crate::state_persistence::domain::{ExecutionState, ExecutionStatus, NodeState, NodeStatus};

// ---------------------------------------------------------------------------
// Save State DTOs
// ---------------------------------------------------------------------------

/// Input for saving execution state to persistent storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveStateInput {
    /// The execution state to persist.
    pub state: ExecutionState,
}

/// Output from saving execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveStateOutput {
    /// The execution ID whose state was saved.
    pub execution_id: Uuid,
    /// The status at the time of save.
    pub status: ExecutionStatus,
    /// Number of node states saved.
    pub node_count: u32,
    /// ISO 8601 timestamp of the save.
    pub saved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Load State DTOs
// ---------------------------------------------------------------------------

/// Input for loading execution state from persistent storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadStateInput {
    /// The execution ID whose state to load.
    pub execution_id: Uuid,
}

/// Output from loading execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadStateOutput {
    /// The loaded execution state.
    pub state: ExecutionState,
    /// ISO 8601 timestamp of when the state was loaded.
    pub loaded_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Node State Changed DTOs
// ---------------------------------------------------------------------------

/// Input for updating a single node's state within an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateChangedInput {
    /// The execution ID whose node state is changing.
    pub execution_id: Uuid,
    /// The node ID whose state changed.
    pub node_id: Uuid,
    /// The new node status.
    pub new_status: NodeStatus,
    /// Output produced by the node (on completion).
    pub output: Option<String>,
    /// Error message (on failure).
    pub error: Option<String>,
    /// Duration of execution in milliseconds (on completion/failure).
    pub duration_ms: Option<u64>,
}

/// Output from updating a single node's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateChangedOutput {
    /// The execution ID.
    pub execution_id: Uuid,
    /// The node ID whose state changed.
    pub node_id: Uuid,
    /// The updated node state.
    pub node_state: NodeState,
    /// ISO 8601 timestamp of the update.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// List Executions DTOs
// ---------------------------------------------------------------------------

/// Input for listing available executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListExecutionsInput {
    /// Maximum number of executions to return.
    /// Default: 50
    pub limit: Option<u32>,
    /// Filter by execution status (optional).
    pub status_filter: Option<ExecutionStatus>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

impl Default for ListExecutionsInput {
    fn default() -> Self {
        Self {
            limit: Some(50),
            status_filter: None,
            offset: Some(0),
        }
    }
}

/// Output from listing available executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListExecutionsOutput {
    /// List of execution summaries.
    pub executions: Vec<ExecutionSummary>,
    /// Total number of executions available (before filtering/pagination).
    pub total_count: u32,
    /// The limit applied to this query.
    pub limit: u32,
    /// The offset applied to this query.
    pub offset: u32,
}

/// Summary of a single execution for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Globally unique execution identifier.
    pub execution_id: Uuid,
    /// Overall execution status.
    pub status: ExecutionStatus,
    /// ISO 8601 timestamp when the execution started.
    pub started_at: DateTime<Utc>,
    /// ISO 8601 timestamp when the execution completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// Total execution duration in milliseconds (if completed).
    pub duration_ms: Option<u64>,
    /// Number of nodes in the execution.
    pub node_count: u32,
    /// Number of completed nodes.
    pub completed_node_count: u32,
    /// Number of failed nodes.
    pub failed_node_count: u32,
    /// Number of skipped nodes.
    pub skipped_node_count: u32,
    /// Whether any nodes are currently in progress.
    pub has_active_nodes: bool,
}

impl From<&ExecutionState> for ExecutionSummary {
    fn from(state: &ExecutionState) -> Self {
        let duration_ms = match (state.started_at, state.completed_at) {
            (start, Some(end)) => Some((end - start).num_milliseconds() as u64),
            _ => None,
        };

        let summary = state.status_summary();

        Self {
            execution_id: state.execution_id,
            status: state.status,
            started_at: state.started_at,
            completed_at: state.completed_at,
            duration_ms,
            node_count: state.node_states.len() as u32,
            completed_node_count: summary.completed,
            failed_node_count: summary.failed,
            skipped_node_count: summary.skipped,
            has_active_nodes: summary.in_progress > 0,
        }
    }
}
