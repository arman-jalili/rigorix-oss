//! Data Transfer Objects for the Code Graph module.
//!
//! @canonical .pi/architecture/modules/code-graph.md
//! Implements: Contract Freeze — DTO schemas for code graph operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::code_graph::domain::{CodeGraph, EdgeKind, ModuleEdge, ModuleNode, NodeKind};

// ---------------------------------------------------------------------------
// Construct Graph DTOs
// ---------------------------------------------------------------------------

/// Input for constructing a new CodeGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructGraphInput {
    /// Name for the graph (e.g., "cargo-deps", "ts-imports").
    pub name: String,
    /// The tool or process that produced this graph.
    pub source: String,
    /// Human-readable description of what this graph represents.
    pub description: String,
    /// Total number of modules scanned (may differ from nodes added).
    pub total_modules_scanned: u64,
}

/// Output from constructing a new CodeGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructGraphOutput {
    /// The ID assigned to this graph.
    pub graph_id: Uuid,
    /// The constructed (unsealed) CodeGraph.
    pub graph: CodeGraph,
    /// ISO 8601 timestamp of construction.
    pub constructed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Add Node DTOs
// ---------------------------------------------------------------------------

/// Input for adding a module node to a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeInput {
    /// The ID of the graph to add the node to.
    pub graph_id: Uuid,
    /// Human-readable name of the module.
    pub name: String,
    /// The kind of module this node represents.
    pub kind: NodeKind,
    /// Canonical path to the module.
    pub path: String,
    /// Optional metadata key-value pairs.
    pub metadata: std::collections::HashMap<String, String>,
}

/// Output from adding a node to a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeOutput {
    /// The ID of the graph the node was added to.
    pub graph_id: Uuid,
    /// The ID of the added node.
    pub node_id: Uuid,
    /// Updated node count in the graph.
    pub node_count: u32,
    /// ISO 8601 timestamp of the addition.
    pub added_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Add Edge DTOs
// ---------------------------------------------------------------------------

/// Input for adding an edge between two module nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEdgeInput {
    /// The ID of the graph to add the edge to.
    pub graph_id: Uuid,
    /// The source node ID (provider/dependency).
    pub source_id: Uuid,
    /// The target node ID (consumer/dependent).
    pub target_id: Uuid,
    /// The kind of relationship.
    pub kind: EdgeKind,
    /// Optional weight or count of the relationship.
    pub weight: u64,
    /// Optional label for display.
    pub label: Option<String>,
}

/// Output from adding an edge to a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEdgeOutput {
    /// The ID of the graph the edge was added to.
    pub graph_id: Uuid,
    /// The source node ID.
    pub source_id: Uuid,
    /// The target node ID.
    pub target_id: Uuid,
    /// Updated edge count in the graph.
    pub edge_count: u32,
    /// ISO 8601 timestamp of the addition.
    pub added_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Seal Graph DTOs
// ---------------------------------------------------------------------------

/// Input for sealing a graph (freezing it for analysis).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealGraphInput {
    /// The ID of the graph to seal.
    pub graph_id: Uuid,
}

/// Output from sealing a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealGraphOutput {
    /// The sealed CodeGraph.
    pub graph: CodeGraph,
    /// Number of nodes in the sealed graph.
    pub node_count: u32,
    /// Number of edges in the sealed graph.
    pub edge_count: u32,
    /// ISO 8601 timestamp of sealing.
    pub sealed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Get/Query Graph DTOs
// ---------------------------------------------------------------------------

/// Input for retrieving a CodeGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGraphInput {
    /// The ID of the graph to retrieve.
    pub graph_id: Uuid,
}

/// Output from retrieving a CodeGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGraphOutput {
    /// The ID of the graph.
    pub graph_id: Uuid,
    /// The retrieved CodeGraph.
    pub graph: CodeGraph,
    /// ISO 8601 timestamp of retrieval.
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// List Graphs DTOs
// ---------------------------------------------------------------------------

/// Input for listing available graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGraphsInput {
    /// Maximum number of graphs to return.
    pub limit: u32,
    /// Pagination offset.
    pub offset: u32,
}

/// Output from listing available graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGraphsOutput {
    /// Summary of each graph.
    pub graphs: Vec<GraphSummary>,
    /// Total number of available graphs.
    pub total_count: u32,
    /// The limit used for pagination.
    pub limit: u32,
    /// The offset used for pagination.
    pub offset: u32,
}

// ---------------------------------------------------------------------------
// Node Query DTOs
// ---------------------------------------------------------------------------

/// Input for querying a specific node in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeInput {
    /// The ID of the graph containing the node.
    pub graph_id: Uuid,
    /// The ID of the node to retrieve.
    pub node_id: Uuid,
}

/// Output from querying a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeOutput {
    /// The retrieved node.
    pub node: ModuleNode,
    /// Incoming edges (dependencies of this node).
    pub incoming_edges: Vec<ModuleEdge>,
    /// Outgoing edges (dependents of this node).
    pub outgoing_edges: Vec<ModuleEdge>,
    /// ISO 8601 timestamp of retrieval.
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Dependency Analysis DTOs
// ---------------------------------------------------------------------------

/// Input for dependency analysis on a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeDependenciesInput {
    /// The ID of the graph to analyze.
    pub graph_id: Uuid,
    /// Optional node ID to scope analysis to a specific module.
    pub scope_node_id: Option<Uuid>,
    /// Whether to include transitive dependencies.
    pub include_transitive: bool,
}

/// Output from dependency analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeDependenciesOutput {
    /// The graph ID analyzed.
    pub graph_id: Uuid,
    /// Total number of nodes in the graph.
    pub total_nodes: u32,
    /// Total number of edges in the graph.
    pub total_edges: u32,
    /// Number of cycles detected (0 if no cycles).
    pub cycle_count: u32,
    /// List of module paths involved in cycles (if any).
    pub cycle_paths: Vec<Vec<String>>,
    /// Nodes with no incoming edges (root modules).
    pub root_nodes: Vec<ModuleNode>,
    /// Nodes with no outgoing edges (leaf modules).
    pub leaf_nodes: Vec<ModuleNode>,
    /// ISO 8601 timestamp of analysis.
    pub analyzed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Impact Analysis DTOs
// ---------------------------------------------------------------------------

/// Input for impact analysis (what breaks if a module changes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysisInput {
    /// The ID of the graph to analyze.
    pub graph_id: Uuid,
    /// The node ID whose change impact to analyze.
    pub node_id: Uuid,
    /// Maximum depth of transitive impact to follow.
    pub max_depth: u32,
}

/// Output from impact analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysisOutput {
    /// The node that was analyzed.
    pub target_node: ModuleNode,
    /// Direct dependents (depth 1).
    pub direct_impact_count: u32,
    /// Total transitive dependents up to max_depth.
    pub total_impact_count: u32,
    /// The dependency chain for each affected module.
    pub impact_chains: Vec<ImpactChain>,
    /// ISO 8601 timestamp of analysis.
    pub analyzed_at: DateTime<Utc>,
}

/// A single impact chain from target to affected module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactChain {
    /// The ultimately affected node.
    pub affected_node: ModuleNode,
    /// The depth of the impact (1 = direct dependency).
    pub depth: u32,
    /// The path of module names from target to affected node.
    pub path: Vec<String>,
}

// ---------------------------------------------------------------------------
// Persist DTOs
// ---------------------------------------------------------------------------

/// Input for persisting a CodeGraph to storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistGraphInput {
    /// The graph to persist.
    pub graph: CodeGraph,
    /// Optional storage backend override (default: filesystem).
    pub storage_backend: Option<String>,
}

/// Output from persisting a CodeGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistGraphOutput {
    /// The ID of the persisted graph.
    pub graph_id: Uuid,
    /// The storage backend used.
    pub storage_backend: String,
    /// Size in bytes of the persisted data.
    pub size_bytes: u64,
    /// ISO 8601 timestamp of persistence.
    pub persisted_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Format DTOs
// ---------------------------------------------------------------------------

/// Input for formatting a graph for output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatGraphInput {
    /// The graph to format.
    pub graph: CodeGraph,
    /// The output format.
    pub format: OutputFormat,
    /// Whether to include node metadata in the output.
    pub include_metadata: bool,
}

/// Output from formatting a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatGraphOutput {
    /// The formatted output string.
    pub output: String,
    /// The format used.
    pub format: OutputFormat,
    /// Size of the output in characters.
    pub output_size: u64,
    /// ISO 8601 timestamp of formatting.
    pub formatted_at: DateTime<Utc>,
}

/// Supported output formats for graph rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Mermaid.js flowchart syntax.
    Mermaid,
    /// Graphviz DOT format.
    Dot,
    /// Indented text tree representation.
    Tree,
    /// JSON serialization.
    Json,
    /// Plain text adjacency list.
    List,
    /// Compact citation format: file → [dependency1, dependency2]
    /// Mirrors FastContext <final_answer> output — file:line references only.
    Compact,
}

impl OutputFormat {
    /// Returns the canonical name of this format.
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Mermaid => "mermaid",
            OutputFormat::Dot => "dot",
            OutputFormat::Tree => "tree",
            OutputFormat::Json => "json",
            OutputFormat::List => "list",
            OutputFormat::Compact => "compact",
        }
    }
}

// ---------------------------------------------------------------------------
// Graph Summary DTO
// ---------------------------------------------------------------------------

/// Summary of a CodeGraph for display and listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSummary {
    /// The graph ID.
    pub graph_id: Uuid,
    /// Name of the graph.
    pub name: String,
    /// The tool/process that produced this graph.
    pub source: String,
    /// Number of nodes in the graph.
    pub node_count: u32,
    /// Number of edges in the graph.
    pub edge_count: u32,
    /// Whether the graph is sealed.
    pub is_sealed: bool,
    /// ISO 8601 timestamp when the graph was created.
    pub created_at: DateTime<Utc>,
    /// ISO 8601 timestamp when the graph was sealed (if applicable).
    pub sealed_at: Option<DateTime<Utc>>,
}

impl GraphSummary {
    /// Create a GraphSummary from a CodeGraph and its associated ID.
    pub fn from_graph(graph: &CodeGraph, graph_id: Uuid) -> Self {
        Self {
            graph_id,
            name: graph.metadata.name.clone(),
            source: graph.metadata.source.clone(),
            node_count: graph.nodes.len() as u32,
            edge_count: graph.edges.len() as u32,
            is_sealed: graph.sealed,
            created_at: graph.metadata.created_at,
            sealed_at: None,
        }
    }
}
