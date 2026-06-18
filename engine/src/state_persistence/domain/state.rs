//! ExecutionState, NodeState, ExecutionStatus, and NodeStatus domain entities.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#state
//! Implements: Contract Freeze — ExecutionState aggregate with node_states
//! Issue: issue-contract-freeze
//!
//! Defines the serializable snapshot of an entire execution: overall status,
//! timing, per-node state (status, output, errors, retries, duration), and
//! a symbol graph hash for replay determinism.
//!
//! # Contract (Frozen)
//! - `ExecutionState` is the root aggregate for execution snapshots
//! - `NodeState` tracks individual DAG node lifecycle
//! - `NodeStatus` and `ExecutionStatus` enums define the lifecycle state machine
//! - All fields are public for direct access by application services
//! - Construction happens via constructors or StateManager initialisation

use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::StateError;

// ---------------------------------------------------------------------------
// Execution Status
// ---------------------------------------------------------------------------

/// Overall status of an execution.
///
/// Represents the lifecycle state of the entire execution:
/// - `Pending`: Execution created but not yet started
/// - `Running`: Execution is actively running nodes
/// - `Completed`: All nodes completed successfully
/// - `Failed`: Execution terminated with an unrecoverable error
/// - `Cancelled`: Execution was cancelled (user-initiated or graceful shutdown)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ExecutionStatus {
    /// Execution created but not yet started.
    #[default]
    Pending,
    /// Execution is actively running nodes.
    Running,
    /// All nodes completed successfully.
    Completed,
    /// Execution terminated with an unrecoverable error.
    Failed,
    /// Execution was cancelled (user-initiated or graceful shutdown).
    Cancelled,
}

impl ExecutionStatus {
    /// Returns true if this status represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        )
    }

    /// Returns true if this status represents an error/failure condition.
    pub fn is_error(&self) -> bool {
        matches!(self, ExecutionStatus::Failed)
    }

    /// Returns the canonical snake_case name of this status variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionStatus::Pending => "pending",
            ExecutionStatus::Running => "running",
            ExecutionStatus::Completed => "completed",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::Cancelled => "cancelled",
        }
    }
}

// ---------------------------------------------------------------------------
// Per-Node Status
// ---------------------------------------------------------------------------

/// Status of an individual DAG node during execution.
///
/// Represents the lifecycle state of a single node:
/// - `Pending`: Node created but not yet started
/// - `InProgress`: Node is currently executing
/// - `Completed`: Node finished successfully
/// - `Failed`: Node failed (may be retried depending on policy)
/// - `Skipped`: Node was skipped (dependency failed or policy decision)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NodeStatus {
    /// Node created but not yet started.
    #[default]
    Pending,
    /// Node is currently executing.
    InProgress,
    /// Node finished successfully.
    Completed,
    /// Node failed (may be retried depending on policy).
    Failed,
    /// Node was skipped (dependency failed or policy decision).
    Skipped,
}

impl NodeStatus {
    /// Returns true if this status represents a terminal node state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Skipped
        )
    }

    /// Returns true if this status represents a running/in-progress node.
    pub fn is_active(&self) -> bool {
        *self == NodeStatus::InProgress
    }

    /// Returns the canonical snake_case name of this status variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeStatus::Pending => "pending",
            NodeStatus::InProgress => "in_progress",
            NodeStatus::Completed => "completed",
            NodeStatus::Failed => "failed",
            NodeStatus::Skipped => "skipped",
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionState — Root Aggregate
// ---------------------------------------------------------------------------

/// Serializable snapshot of an entire execution.
///
/// Captures the full state of an execution at a point in time, including
/// overall status, timing, per-node states, and the symbol graph hash
/// for replay determinism.
///
/// Persisted atomically by `StateManager` using write-rename:
/// `{execution_id}.json.tmp` → `{execution_id}.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionState {
    /// Globally unique execution identifier.
    pub execution_id: Uuid,

    /// Overall execution status.
    pub status: ExecutionStatus,

    /// ISO 8601 timestamp when the execution started.
    pub started_at: DateTime<Utc>,

    /// ISO 8601 timestamp when the execution completed, failed, or was cancelled.
    /// `None` while the execution is still running.
    pub completed_at: Option<DateTime<Utc>>,

    /// Per-node states keyed by node UUID.
    ///
    /// Uses `IndexMap` for deterministic serialization order.
    pub node_states: IndexMap<Uuid, NodeState>,

    /// SHA-256 hash of the symbol graph state at execution start.
    ///
    /// Used for replay determinism — two executions with the same
    /// symbol_graph_hash on the same template should produce the same plan.
    pub symbol_graph_hash: String,
}

impl ExecutionState {
    /// Create a new ExecutionState for the given execution.
    ///
    /// Initialises all fields with the execution running.
    /// `node_states` is empty — nodes are added via `init_node_states`.
    pub fn new(execution_id: Uuid, symbol_graph_hash: String) -> Self {
        Self {
            execution_id,
            status: ExecutionStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
            node_states: IndexMap::new(),
            symbol_graph_hash,
        }
    }

    /// Initialise per-node states from a list of node IDs.
    ///
    /// Each node is initialised to `NodeStatus::Pending`.
    /// If a node already exists in the map, it is not overwritten.
    pub fn init_node_states(&mut self, node_ids: &[Uuid]) {
        for &node_id in node_ids {
            self.node_states
                .entry(node_id)
                .or_insert_with(|| NodeState::new(node_id));
        }
    }

    /// Transition this execution to Running status.
    ///
    /// Sets `started_at` to the current time.
    /// Returns an error if the execution is already in a terminal state.
    pub fn start(&mut self) -> Result<(), StateError> {
        if self.status.is_terminal() {
            return Err(StateError::InvalidTransition {
                from: self.status.as_str().to_string(),
                to: "running".to_string(),
                detail: "Cannot start an execution that is already in a terminal state".to_string(),
            });
        }
        self.status = ExecutionStatus::Running;
        self.started_at = Utc::now();
        Ok(())
    }

    /// Transition this execution to Completed status.
    ///
    /// Sets `completed_at` to the current time.
    /// Returns an error if the execution is not currently Running.
    pub fn complete(&mut self) -> Result<(), StateError> {
        if self.status != ExecutionStatus::Running {
            return Err(StateError::InvalidTransition {
                from: self.status.as_str().to_string(),
                to: "completed".to_string(),
                detail: "Only a running execution can be completed".to_string(),
            });
        }
        self.status = ExecutionStatus::Completed;
        self.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Transition this execution to Failed status.
    ///
    /// Sets `completed_at` to the current time.
    /// Returns an error if the execution is not currently Running.
    pub fn fail(&mut self) -> Result<(), StateError> {
        if self.status != ExecutionStatus::Running && self.status != ExecutionStatus::Pending {
            return Err(StateError::InvalidTransition {
                from: self.status.as_str().to_string(),
                to: "failed".to_string(),
                detail: "Only a pending or running execution can fail".to_string(),
            });
        }
        self.status = ExecutionStatus::Failed;
        self.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Transition this execution to Cancelled status.
    ///
    /// Sets `completed_at` to the current time.
    /// Returns an error if the execution is already in a terminal state.
    pub fn cancel(&mut self) -> Result<(), StateError> {
        if self.status.is_terminal() {
            return Err(StateError::InvalidTransition {
                from: self.status.as_str().to_string(),
                to: "cancelled".to_string(),
                detail: "Cannot cancel an execution that is already in a terminal state"
                    .to_string(),
            });
        }
        self.status = ExecutionStatus::Cancelled;
        self.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Mark a node as started (transition to InProgress).
    pub fn node_started(&mut self, node_id: Uuid) -> Result<(), StateError> {
        let node = self
            .node_states
            .get_mut(&node_id)
            .ok_or_else(|| StateError::NodeNotFound {
                node_id: node_id.to_string(),
                execution_id: self.execution_id.to_string(),
            })?;

        if node.status != NodeStatus::Pending {
            return Err(StateError::InvalidNodeTransition {
                node_id: node_id.to_string(),
                from: node.status.as_str().to_string(),
                to: "in_progress".to_string(),
                detail: "Only a pending node can be started".to_string(),
            });
        }

        node.status = NodeStatus::InProgress;
        node.started_at = Some(Utc::now());
        Ok(())
    }

    /// Mark a node as completed with its output and duration.
    pub fn node_completed(
        &mut self,
        node_id: Uuid,
        output: Option<String>,
        duration_ms: u64,
    ) -> Result<(), StateError> {
        let node = self
            .node_states
            .get_mut(&node_id)
            .ok_or_else(|| StateError::NodeNotFound {
                node_id: node_id.to_string(),
                execution_id: self.execution_id.to_string(),
            })?;

        if node.status != NodeStatus::InProgress {
            return Err(StateError::InvalidNodeTransition {
                node_id: node_id.to_string(),
                from: node.status.as_str().to_string(),
                to: "completed".to_string(),
                detail: "Only an in-progress node can be completed".to_string(),
            });
        }

        node.status = NodeStatus::Completed;
        node.output = output;
        node.duration_ms = Some(duration_ms);
        Ok(())
    }

    /// Mark a node as failed with its error.
    pub fn node_failed(&mut self, node_id: Uuid, error: String) -> Result<(), StateError> {
        let node = self
            .node_states
            .get_mut(&node_id)
            .ok_or_else(|| StateError::NodeNotFound {
                node_id: node_id.to_string(),
                execution_id: self.execution_id.to_string(),
            })?;

        if node.status != NodeStatus::InProgress && node.status != NodeStatus::Pending {
            return Err(StateError::InvalidNodeTransition {
                node_id: node_id.to_string(),
                from: node.status.as_str().to_string(),
                to: "failed".to_string(),
                detail: "Only an in-progress or pending node can fail".to_string(),
            });
        }

        node.status = NodeStatus::Failed;
        node.error = Some(error);
        Ok(())
    }

    /// Mark a node as skipped.
    pub fn node_skipped(
        &mut self,
        node_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), StateError> {
        let node = self
            .node_states
            .get_mut(&node_id)
            .ok_or_else(|| StateError::NodeNotFound {
                node_id: node_id.to_string(),
                execution_id: self.execution_id.to_string(),
            })?;

        if node.status != NodeStatus::Pending {
            return Err(StateError::InvalidNodeTransition {
                node_id: node_id.to_string(),
                from: node.status.as_str().to_string(),
                to: "skipped".to_string(),
                detail: "Only a pending node can be skipped".to_string(),
            });
        }

        node.status = NodeStatus::Skipped;
        node.error = reason;
        Ok(())
    }

    /// Increment the retry count for a node.
    ///
    /// Resets the node status back to Pending so it can be re-started.
    /// Returns an error if the node is not currently Failed.
    pub fn increment_retry(&mut self, node_id: Uuid) -> Result<(), StateError> {
        let node = self
            .node_states
            .get_mut(&node_id)
            .ok_or_else(|| StateError::NodeNotFound {
                node_id: node_id.to_string(),
                execution_id: self.execution_id.to_string(),
            })?;

        if node.status != NodeStatus::Failed {
            return Err(StateError::InvalidNodeTransition {
                node_id: node_id.to_string(),
                from: node.status.as_str().to_string(),
                to: "pending".to_string(),
                detail: "Only a failed node can be retried".to_string(),
            });
        }

        let current_retries = node.retries;
        if current_retries == u8::MAX {
            return Err(StateError::RetryLimitExceeded {
                node_id: node_id.to_string(),
                retries: current_retries,
                max_retries: u8::MAX,
            });
        }

        node.retries += 1;
        node.status = NodeStatus::Pending;
        node.error = None; // Clear the error for the retry
        Ok(())
    }

    /// Get the number of nodes in each status category.
    pub fn status_summary(&self) -> NodeStatusSummary {
        let mut summary = NodeStatusSummary::default();
        for node_state in self.node_states.values() {
            match node_state.status {
                NodeStatus::Pending => summary.pending += 1,
                NodeStatus::InProgress => summary.in_progress += 1,
                NodeStatus::Completed => summary.completed += 1,
                NodeStatus::Failed => summary.failed += 1,
                NodeStatus::Skipped => summary.skipped += 1,
            }
        }
        summary
    }
}

/// Summary of node status counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NodeStatusSummary {
    /// Number of nodes still pending.
    pub pending: u32,
    /// Number of nodes currently in progress.
    pub in_progress: u32,
    /// Number of nodes completed successfully.
    pub completed: u32,
    /// Number of nodes that failed.
    pub failed: u32,
    /// Number of nodes that were skipped.
    pub skipped: u32,
}

// ---------------------------------------------------------------------------
// NodeState
// ---------------------------------------------------------------------------

/// Per-node state within an execution.
///
/// Tracks the lifecycle state of a single DAG node: its status, output,
/// error information, retry count, and timing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeState {
    /// Unique identifier for this node within the DAG.
    pub node_id: Uuid,

    /// Current status of this node.
    pub status: NodeStatus,

    /// Output produced by this node on successful completion.
    /// `None` if the node has not completed or failed without output.
    pub output: Option<String>,

    /// Error message if this node failed.
    /// `None` if the node has not failed or is still running.
    pub error: Option<String>,

    /// Number of retry attempts so far (0 = first attempt, never retried).
    pub retries: u8,

    /// Duration of the last execution in milliseconds.
    /// `None` if the node has never been executed.
    pub duration_ms: Option<u64>,

    /// ISO 8601 timestamp when this node started its current/last execution.
    /// `None` if the node has never been started.
    pub started_at: Option<DateTime<Utc>>,

    /// ISO 8601 timestamp when this node last completed, failed, or was skipped.
    /// `None` if the node has never reached a terminal state.
    pub completed_at: Option<DateTime<Utc>>,
}

impl NodeState {
    /// Create a new NodeState in Pending status.
    pub fn new(node_id: Uuid) -> Self {
        Self {
            node_id,
            status: NodeStatus::Pending,
            output: None,
            error: None,
            retries: 0,
            duration_ms: None,
            started_at: None,
            completed_at: None,
        }
    }
}
