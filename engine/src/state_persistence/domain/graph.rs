//! ExecutionGraph and ExecutionGraphNode domain entities.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#graph
//! Implements: Contract Freeze — ExecutionGraph structure for TUI history view
//! Issue: issue-contract-freeze
//!
//! Defines the graph structure that is persisted for TUI "view past execution"
//! mode. Captures the DAG structure, node-level results, and execution metadata
//! so the TUI can reconstruct and display completed executions.
//!
//! # Contract (Frozen)
//! - `ExecutionGraph` is the persistent record of a completed DAG execution
//! - `ExecutionGraphNode` captures per-node results and timing
//! - `GraphManager` is the service interface for CRUD operations
//! - All fields are public for direct serialisation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::state::{ExecutionStatus, NodeStatus};

/// A persisted execution graph for TUI history view.
///
/// Captures the complete structure and results of a DAG execution so the
/// TUI can display past executions with full node-level detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionGraph {
    /// Unique identifier for this graph record.
    pub graph_id: Uuid,

    /// The execution ID this graph corresponds to.
    pub execution_id: Uuid,

    /// The human-readable name or description of this execution.
    pub name: String,

    /// Overall execution status.
    pub status: ExecutionStatus,

    /// ISO 8601 timestamp when the execution started.
    pub started_at: DateTime<Utc>,

    /// ISO 8601 timestamp when the execution completed.
    pub completed_at: Option<DateTime<Utc>>,

    /// Nodes in this execution graph, keyed by node ID.
    pub nodes: Vec<ExecutionGraphNode>,

    /// Total number of nodes in the original DAG.
    pub total_node_count: u32,

    /// Number of nodes that completed successfully.
    pub completed_node_count: u32,

    /// Number of nodes that failed.
    pub failed_node_count: u32,

    /// Number of nodes that were skipped.
    pub skipped_node_count: u32,

    /// Total execution duration in milliseconds.
    pub total_duration_ms: u64,
}

impl ExecutionGraph {
    /// Create a new ExecutionGraph from execution results.
    pub fn new(
        execution_id: Uuid,
        name: String,
        status: ExecutionStatus,
        started_at: DateTime<Utc>,
        completed_at: Option<DateTime<Utc>>,
        nodes: Vec<ExecutionGraphNode>,
        total_duration_ms: u64,
    ) -> Self {
        let total_node_count = nodes.len() as u32;
        let completed_node_count = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Completed)
            .count() as u32;
        let failed_node_count = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Failed)
            .count() as u32;
        let skipped_node_count = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Skipped)
            .count() as u32;

        Self {
            graph_id: Uuid::new_v4(),
            execution_id,
            name,
            status,
            started_at,
            completed_at,
            nodes,
            total_node_count,
            completed_node_count,
            failed_node_count,
            skipped_node_count,
            total_duration_ms,
        }
    }
}

/// A single node within a persisted execution graph.
///
/// Captures the node's results, timing, and metadata for TUI display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionGraphNode {
    /// Unique identifier for this node within the DAG.
    pub node_id: Uuid,

    /// Human-readable name of this node.
    pub name: String,

    /// Terminal status of this node.
    pub status: NodeStatus,

    /// Output produced by this node (truncated for storage).
    pub output_summary: Option<String>,

    /// Error message if this node failed.
    pub error: Option<String>,

    /// Number of retry attempts.
    pub retries: u8,

    /// Duration of execution in milliseconds.
    pub duration_ms: Option<u64>,

    /// ISO 8601 timestamp when this node started.
    pub started_at: Option<DateTime<Utc>>,

    /// ISO 8601 timestamp when this node completed.
    pub completed_at: Option<DateTime<Utc>>,

    /// List of upstream dependency node IDs.
    pub dependencies: Vec<Uuid>,

    /// The template action that was executed for this node.
    pub action_type: String,
}

impl ExecutionGraphNode {
    /// Create a new ExecutionGraphNode.
    pub fn new(node_id: Uuid, name: String, action_type: String, dependencies: Vec<Uuid>) -> Self {
        Self {
            node_id,
            name,
            status: NodeStatus::Pending,
            output_summary: None,
            error: None,
            retries: 0,
            duration_ms: None,
            started_at: None,
            completed_at: None,
            dependencies,
            action_type,
        }
    }
}
