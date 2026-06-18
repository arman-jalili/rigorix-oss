//! Service interfaces (use cases) for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md
//! Implements: Contract Freeze — CodeGraphService, CodeGraphFormatter,
//!   CodeGraphAnalyzer service traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for code graph
//! construction, analysis, persistence, and formatting:
//! - `CodeGraphService`: Graph construction, node/edge management, sealing
//! - `CodeGraphAnalyzer`: Dependency analysis, cycle detection, impact analysis
//! - `CodeGraphFormatter`: Graph output formatting for display and export
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;
use uuid::Uuid;

use crate::code_graph::domain::{CodeGraphError, GraphMetadata, ModuleEdge, ModuleNode};

use super::dto::{
    AddEdgeInput, AddEdgeOutput, AddNodeInput, AddNodeOutput, AnalyzeDependenciesInput,
    AnalyzeDependenciesOutput, ConstructGraphInput, ConstructGraphOutput, FormatGraphInput,
    FormatGraphOutput, GetGraphInput, GetGraphOutput, GetNodeInput, GetNodeOutput,
    ImpactAnalysisInput, ImpactAnalysisOutput, ListGraphsInput, ListGraphsOutput,
    PersistGraphInput, PersistGraphOutput, SealGraphInput, SealGraphOutput,
};

/// Core code graph service for construction, management, and lifecycle.
///
/// The CodeGraphService sits between code analysis tools (which produce
/// module lists) and graph consumers (formatters, dependency analyzers).
/// It handles:
///
/// 1. Graph construction with metadata (name, source, description)
/// 2. Node and edge management (add, query, list)
/// 3. Graph sealing (freeze for analysis)
/// 4. Graph persistence (save/load)
///
/// # Lifecycle
///
/// 1. `construct_graph` — Create a new CodeGraph with metadata
/// 2. `add_node` — Add module nodes (one at a time or in batches)
/// 3. `add_edge` — Add relationship edges between nodes
/// 4. `seal_graph` — Seal the graph for analysis
/// 5. `get_graph` — Retrieve the current graph state
/// 6. `persist_graph` / `load_graph` — Persistence lifecycle
///
/// # Contract (Frozen)
/// - Graph is open for modification until sealed
/// - After sealing, only query operations are permitted
/// - Persistence is handled through configurable backends
#[async_trait]
pub trait CodeGraphService: Send + Sync {
    /// Construct a new CodeGraph with metadata.
    ///
    /// Creates a new CodeGraph in open state. Nodes and edges can be
    /// added until `seal_graph` is called.
    async fn construct_graph(
        &self,
        input: ConstructGraphInput,
    ) -> Result<ConstructGraphOutput, CodeGraphError>;

    /// Add a module node to an existing open graph.
    ///
    /// Appends a ModuleNode to the graph. Returns an error if the graph
    /// has already been sealed or if a node with the same ID exists.
    async fn add_node(&self, input: AddNodeInput) -> Result<AddNodeOutput, CodeGraphError>;

    /// Add a directed edge between two module nodes.
    ///
    /// Creates a relationship from source_id to target_id. Both endpoints
    /// must exist as nodes in the graph.
    async fn add_edge(&self, input: AddEdgeInput) -> Result<AddEdgeOutput, CodeGraphError>;

    /// Seal a graph, freezing it for analysis and querying.
    ///
    /// After sealing, no more nodes or edges can be added. The graph
    /// is ready for dependency analysis, impact analysis, and formatting.
    async fn seal_graph(&self, input: SealGraphInput) -> Result<SealGraphOutput, CodeGraphError>;

    /// Retrieve the current state of a CodeGraph.
    async fn get_graph(&self, input: GetGraphInput) -> Result<GetGraphOutput, CodeGraphError>;

    /// Get detailed information about a specific node.
    async fn get_node(&self, input: GetNodeInput) -> Result<GetNodeOutput, CodeGraphError>;

    /// List all available graphs with summary information.
    async fn list_graphs(
        &self,
        input: ListGraphsInput,
    ) -> Result<ListGraphsOutput, CodeGraphError>;

    /// Persist a CodeGraph to storage.
    ///
    /// Saves the graph to the configured storage backend (filesystem,
    /// database, etc.).
    async fn persist_graph(
        &self,
        input: PersistGraphInput,
    ) -> Result<PersistGraphOutput, CodeGraphError>;

    /// Load a CodeGraph from storage.
    async fn load_graph(&self, input: GetGraphInput) -> Result<GetGraphOutput, CodeGraphError>;

    /// Delete a CodeGraph from storage.
    ///
    /// Idempotent — returns Ok even if the graph does not exist.
    async fn delete_graph(&self, graph_id: Uuid) -> Result<(), CodeGraphError>;
}

/// Code graph analysis service for dependency and impact analysis.
///
/// The CodeGraphAnalyzer provides structural analysis of the code
/// dependency graph, including:
///
/// 1. Dependency analysis — root/leaf detection, cycle detection
/// 2. Impact analysis — what breaks if a module changes
/// 3. Transitive dependency resolution
///
/// # Contract (Frozen)
/// - All analysis requires a sealed graph
/// - Cycle detection reports all cycles, not just first
/// - Impact analysis respects max_depth for performance
#[async_trait]
pub trait CodeGraphAnalyzer: Send + Sync {
    /// Analyze dependencies in a sealed graph.
    ///
    /// Performs full dependency analysis including:
    /// - Root node detection (no dependencies)
    /// - Leaf node detection (no dependents)
    /// - Cycle detection with path reporting
    /// - Optional scope to a single module
    async fn analyze_dependencies(
        &self,
        input: AnalyzeDependenciesInput,
    ) -> Result<AnalyzeDependenciesOutput, CodeGraphError>;

    /// Analyze the impact of changing a specific module.
    ///
    /// Traces all transitive dependents up to max_depth to determine
    /// what would break if the target module changes.
    async fn analyze_impact(
        &self,
        input: ImpactAnalysisInput,
    ) -> Result<ImpactAnalysisOutput, CodeGraphError>;

    /// Detect cycles in a sealed graph.
    ///
    /// Returns all distinct cycles found in the dependency graph.
    /// Each cycle is represented as a path of module names.
    async fn detect_cycles(
        &self,
        graph_id: Uuid,
    ) -> Result<Vec<Vec<String>>, CodeGraphError>;

    /// Check if a specific module has circular dependencies.
    async fn has_circular_dependencies(
        &self,
        graph_id: Uuid,
        node_id: Uuid,
    ) -> Result<bool, CodeGraphError>;
}

/// Code graph formatter for rendering graphs in various output formats.
///
/// The CodeGraphFormatter converts a sealed CodeGraph into human-readable
/// or machine-parseable output formats for display, documentation, and
/// export.
///
/// # Contract (Frozen)
/// - All formats are generated from a sealed graph
/// - Each format produces self-contained output
/// - Formatting is stateless (no side effects)
/// - Output is deterministic (same graph → same output)
#[async_trait]
pub trait CodeGraphFormatter: Send + Sync {
    /// Format a graph into the specified output format.
    async fn format(&self, input: FormatGraphInput) -> Result<FormatGraphOutput, CodeGraphError>;

    /// Format a graph as a Mermaid.js flowchart.
    async fn format_mermaid(&self, graph_id: Uuid) -> Result<String, CodeGraphError>;

    /// Format a graph as Graphviz DOT.
    async fn format_dot(&self, graph_id: Uuid) -> Result<String, CodeGraphError>;

    /// Format a graph as an indented text tree.
    async fn format_tree(&self, graph_id: Uuid) -> Result<String, CodeGraphError>;

    /// Format a graph as a JSON object.
    async fn format_json(&self, graph_id: Uuid) -> Result<String, CodeGraphError>;

    /// Format a graph as an adjacency list.
    async fn format_list(&self, graph_id: Uuid) -> Result<String, CodeGraphError>;
}

/// Batch import service for populating graphs from analysis tools.
///
/// The CodeGraphImporter provides batch operations for populating a
/// CodeGraph from external analysis tools (e.g., cargo-deps, ts-metrics,
/// custom parsers).
///
/// # Contract (Frozen)
/// - Import creates a new graph or adds to an existing open graph
/// - Import is atomic (all-or-nothing within a single call)
/// - Import validates all nodes before adding any
#[async_trait]
pub trait CodeGraphImporter: Send + Sync {
    /// Import a collection of nodes and edges into a graph.
    ///
    /// If no graph_id is provided, creates a new graph. If a graph_id
    /// is provided, appends to the existing open graph.
    async fn import(
        &self,
        input: ImportInput,
    ) -> Result<ImportOutput, CodeGraphError>;
}

/// Input for importing nodes and edges.
#[derive(Debug, Clone)]
pub struct ImportInput {
    /// Optional graph ID to import into (creates new if None).
    pub graph_id: Option<Uuid>,
    /// The nodes to import.
    pub nodes: Vec<ModuleNode>,
    /// The edges to import.
    pub edges: Vec<ModuleEdge>,
    /// Graph metadata (used only when creating a new graph).
    pub metadata: Option<GraphMetadata>,
}

/// Output from importing nodes and edges.
#[derive(Debug, Clone)]
pub struct ImportOutput {
    /// The graph ID that was imported into.
    pub graph_id: Uuid,
    /// Number of nodes imported.
    pub nodes_imported: u32,
    /// Number of edges imported.
    pub edges_imported: u32,
    /// Total node count after import.
    pub total_nodes: u32,
    /// Total edge count after import.
    pub total_edges: u32,
}
