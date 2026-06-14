//! Data Transfer Objects for the DAG Engine module.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: Contract Freeze — DTO schemas for DAG operations
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

use crate::dag_engine::domain::{PlanDiff, TaskGraph, TaskNode};

// ---------------------------------------------------------------------------
// Construct Graph DTOs
// ---------------------------------------------------------------------------

/// Input for constructing a new TaskGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructGraphInput {
    /// The nodes to add to the graph.
    pub nodes: Vec<TaskNode>,
}

/// Output from constructing a new TaskGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructGraphOutput {
    /// The constructed (unsealed) TaskGraph.
    pub graph: TaskGraph,
    /// Number of nodes in the graph.
    pub node_count: u32,
    /// ISO 8601 timestamp of construction.
    pub constructed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Add Node DTOs
// ---------------------------------------------------------------------------

/// Input for adding a single node to an unsealed graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeInput {
    /// The ID of the graph to add the node to.
    pub dag_id: Uuid,
    /// The node to add.
    pub node: TaskNode,
}

/// Output from adding a node to a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeOutput {
    /// The ID of the graph the node was added to.
    pub dag_id: Uuid,
    /// The ID of the added node.
    pub node_id: Uuid,
    /// Updated node count in the graph.
    pub node_count: u32,
    /// ISO 8601 timestamp of the addition.
    pub added_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Seal Graph DTOs
// ---------------------------------------------------------------------------

/// Input for sealing a graph and running topological sort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealGraphInput {
    /// The ID of the graph to seal.
    pub dag_id: Uuid,
}

/// Output from sealing a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealGraphOutput {
    /// The sealed TaskGraph.
    pub graph: TaskGraph,
    /// The topological ordering of node IDs.
    pub topological_order: Vec<Uuid>,
    /// Number of nodes successfully processed.
    pub processed_count: u32,
    /// Total number of nodes in the graph.
    pub total_nodes: u32,
    /// ISO 8601 timestamp of sealing.
    pub sealed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Get Graph DTOs
// ---------------------------------------------------------------------------

/// Input for retrieving a TaskGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGraphInput {
    /// The ID of the graph to retrieve.
    pub dag_id: Uuid,
}

/// Output from retrieving a TaskGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGraphOutput {
    /// The retrieved TaskGraph.
    pub graph: TaskGraph,
    /// ISO 8601 timestamp of retrieval.
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Get Node DTOs
// ---------------------------------------------------------------------------

/// Input for retrieving a specific node from a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeInput {
    /// The ID of the graph containing the node.
    pub dag_id: Uuid,
    /// The ID of the node to retrieve.
    pub node_id: Uuid,
}

/// Output from retrieving a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeOutput {
    /// The retrieved node.
    pub node: TaskNode,
    /// ISO 8601 timestamp of retrieval.
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// List Nodes DTOs
// ---------------------------------------------------------------------------

/// Input for listing nodes in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNodesInput {
    /// The ID of the graph whose nodes to list.
    pub dag_id: Uuid,
}

/// Output from listing nodes in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNodesOutput {
    /// The list of nodes.
    pub nodes: Vec<TaskNode>,
    /// Total number of nodes.
    pub total_count: u32,
}

// ---------------------------------------------------------------------------
// Compare Plans DTOs
// ---------------------------------------------------------------------------

/// Input for comparing two execution plans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparePlansInput {
    /// The old plan's nodes.
    pub old_nodes: Vec<TaskNode>,
    /// The new plan's nodes.
    pub new_nodes: Vec<TaskNode>,
}

/// Output from comparing two execution plans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparePlansOutput {
    /// The structured diff between plans.
    pub diff: PlanDiff,
    /// ISO 8601 timestamp of comparison.
    pub compared_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Plan Summary DTO
// ---------------------------------------------------------------------------

/// Summary of a plan for display and listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummary {
    /// The plan ID (typically the dag_id).
    pub plan_id: Uuid,
    /// Number of nodes in the plan.
    pub node_count: u32,
    /// ISO 8601 timestamp when the plan was created.
    pub created_at: DateTime<Utc>,
    /// ISO 8601 timestamp when the plan was sealed.
    pub sealed_at: Option<DateTime<Utc>>,
    /// ISO 8601 timestamp when execution completed (if applicable).
    pub completed_at: Option<DateTime<Utc>>,
    /// Number of completed nodes.
    pub completed_node_count: u32,
    /// Number of failed nodes.
    pub failed_node_count: u32,
    /// Number of skipped nodes.
    pub skipped_node_count: u32,
    /// Whether the graph is sealed.
    pub is_sealed: bool,
}

impl PlanSummary {
    /// Create a PlanSummary from a TaskGraph and its associated ID.
    ///
    /// The graph ID is provided separately since TaskGraph doesn't
    /// carry its own identity in the domain layer.
    pub fn from_graph(graph: &TaskGraph, plan_id: Uuid) -> Self {
        Self {
            plan_id,
            node_count: graph.nodes.len() as u32,
            created_at: Utc::now(),
            sealed_at: None,
            completed_at: None,
            completed_node_count: 0,
            failed_node_count: 0,
            skipped_node_count: 0,
            is_sealed: graph.sealed,
        }
    }
}
