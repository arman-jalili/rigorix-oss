//! Service implementations for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md
//! Implements: CodeGraph — CodeGraphServiceImpl, CodeGraphAnalyzerImpl,
//!   CodeGraphFormatterImpl, CodeGraphImporterImpl
//! Issue: issue-codegraph
//!
//! Concrete implementations of CodeGraphService, CodeGraphAnalyzer,
//! CodeGraphFormatter, and CodeGraphImporter that operate directly on
//! CodeGraph domain objects in memory.
//!
//! # Design Decisions
//! - In-memory storage for graph construction phase
//! - Graph state is passed by graph_id which maps to an in-memory CodeGraph
//! - Analyzer uses DFS for cycle detection and impact analysis
//! - Formatter produces deterministic output (same graph → same output)

use async_trait::async_trait;
use chrono::Utc;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use uuid::Uuid;

use crate::code_graph::domain::{
    CodeGraph, CodeGraphError, GraphMetadata, ModuleEdge, ModuleNode, NodeKind,
};

use super::dto::{
    AddEdgeInput, AddEdgeOutput, AddNodeInput, AddNodeOutput, AnalyzeDependenciesInput,
    AnalyzeDependenciesOutput, ConstructGraphInput, ConstructGraphOutput, FormatGraphInput,
    FormatGraphOutput, GetGraphInput, GetGraphOutput, GetNodeInput, GetNodeOutput,
    GraphSummary, ImpactAnalysisInput, ImpactAnalysisOutput, ImpactChain, ListGraphsInput,
    ListGraphsOutput, OutputFormat, PersistGraphInput, PersistGraphOutput, SealGraphInput,
    SealGraphOutput,
};
use super::service::{
    CodeGraphAnalyzer, CodeGraphFormatter, CodeGraphImporter, CodeGraphService, ImportInput,
    ImportOutput,
};

// ---------------------------------------------------------------------------
// CodeGraphServiceImpl
// ---------------------------------------------------------------------------

/// In-memory implementation of CodeGraphService.
///
/// Stores CodeGraph instances in a HashMap keyed by UUID.
pub struct CodeGraphServiceImpl {
    /// In-memory graph store.
    graphs: Mutex<HashMap<Uuid, CodeGraph>>,
}

impl CodeGraphServiceImpl {
    /// Create a new empty CodeGraphServiceImpl.
    pub fn new() -> Self {
        Self {
            graphs: Mutex::new(HashMap::new()),
        }
    }

    /// Get a graph by ID from the internal store.
    fn get_graph(&self, graph_id: Uuid) -> Result<CodeGraph, CodeGraphError> {
        let graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.get(&graph_id).cloned().ok_or_else(|| {
            CodeGraphError::InvalidOperation {
                reason: format!("Graph not found: {}", graph_id),
            }
        })
    }

    /// Get a mutable graph handle for modification.
    fn get_graph_mut(&self, graph_id: Uuid) -> Result<CodeGraph, CodeGraphError> {
        let graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.get(&graph_id).cloned().ok_or_else(|| {
            CodeGraphError::InvalidOperation {
                reason: format!("Graph not found: {}", graph_id),
            }
        })
    }
}

impl Default for CodeGraphServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CodeGraphService for CodeGraphServiceImpl {
    async fn construct_graph(
        &self,
        input: ConstructGraphInput,
    ) -> Result<ConstructGraphOutput, CodeGraphError> {
        let graph_id = Uuid::new_v4();
        let now = Utc::now();

        let metadata = GraphMetadata {
            name: input.name,
            source: input.source,
            created_at: now,
            description: input.description,
            total_modules_scanned: input.total_modules_scanned,
            schema_version: "1.0.0".to_string(),
        };

        let graph = CodeGraph::new(metadata);

        let mut graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.insert(graph_id, graph.clone());

        Ok(ConstructGraphOutput {
            graph_id,
            graph,
            constructed_at: now,
        })
    }

    async fn add_node(&self, input: AddNodeInput) -> Result<AddNodeOutput, CodeGraphError> {
        let mut graph = self.get_graph_mut(input.graph_id)?;
        let node_id = Uuid::new_v4();

        let node = ModuleNode::with_metadata(
            node_id,
            input.name,
            input.kind,
            input.path,
            input.metadata,
        );

        graph.add_node(node)?;
        let node_count = graph.node_count() as u32;

        let mut graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.insert(input.graph_id, graph);

        Ok(AddNodeOutput {
            graph_id: input.graph_id,
            node_id,
            node_count,
            added_at: Utc::now(),
        })
    }

    async fn add_edge(&self, input: AddEdgeInput) -> Result<AddEdgeOutput, CodeGraphError> {
        let mut graph = self.get_graph_mut(input.graph_id)?;

        let edge = ModuleEdge::with_details(
            input.source_id,
            input.target_id,
            input.kind,
            input.weight,
            input.label,
        );

        graph.add_edge(edge)?;
        let edge_count = graph.edge_count() as u32;

        let mut graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.insert(input.graph_id, graph);

        Ok(AddEdgeOutput {
            graph_id: input.graph_id,
            source_id: input.source_id,
            target_id: input.target_id,
            edge_count,
            added_at: Utc::now(),
        })
    }

    async fn seal_graph(&self, input: SealGraphInput) -> Result<SealGraphOutput, CodeGraphError> {
        let mut graph = self.get_graph_mut(input.graph_id)?;
        graph.seal()?;
        let (node_count, edge_count) = (graph.node_count() as u32, graph.edge_count() as u32);

        let mut graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        let graph_clone = graph.clone();
        graphs.insert(input.graph_id, graph);

        Ok(SealGraphOutput {
            graph: graph_clone,
            node_count,
            edge_count,
            sealed_at: Utc::now(),
        })
    }

    async fn get_graph(&self, input: GetGraphInput) -> Result<GetGraphOutput, CodeGraphError> {
        let graph = self.get_graph(input.graph_id)?;
        Ok(GetGraphOutput {
            graph_id: input.graph_id,
            graph,
            retrieved_at: Utc::now(),
        })
    }

    async fn get_node(&self, input: GetNodeInput) -> Result<GetNodeOutput, CodeGraphError> {
        let graph = self.get_graph(input.graph_id)?;

        let node = graph
            .get_node(input.node_id)
            .cloned()
            .ok_or(CodeGraphError::NodeNotFound {
                node_id: input.node_id,
            })?;

        let incoming_edges = graph.incoming_edges(input.node_id).into_iter().cloned().collect();
        let outgoing_edges = graph.outgoing_edges(input.node_id).into_iter().cloned().collect();

        Ok(GetNodeOutput {
            node,
            incoming_edges,
            outgoing_edges,
            retrieved_at: Utc::now(),
        })
    }

    async fn list_graphs(
        &self,
        input: ListGraphsInput,
    ) -> Result<ListGraphsOutput, CodeGraphError> {
        let graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let mut summaries: Vec<GraphSummary> = graphs
            .iter()
            .map(|(id, g)| GraphSummary::from_graph(g, *id))
            .collect();

        let total_count = summaries.len() as u32;
        let offset = input.offset as usize;
        let limit = input.limit as usize;

        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        summaries = summaries.into_iter().skip(offset).take(limit).collect();

        Ok(ListGraphsOutput {
            graphs: summaries,
            total_count,
            limit: input.limit,
            offset: input.offset,
        })
    }

    async fn persist_graph(
        &self,
        input: PersistGraphInput,
    ) -> Result<PersistGraphOutput, CodeGraphError> {
        let graph_id = Uuid::new_v4();
        let serialized =
            serde_json::to_string(&input.graph).map_err(|e| CodeGraphError::SerializationError {
                detail: format!("JSON serialization failed: {}", e),
            })?;

        let size_bytes = serialized.len() as u64;

        let mut graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.insert(graph_id, input.graph);

        Ok(PersistGraphOutput {
            graph_id,
            storage_backend: input.storage_backend.unwrap_or_else(|| "memory".to_string()),
            size_bytes,
            persisted_at: Utc::now(),
        })
    }

    async fn load_graph(&self, input: GetGraphInput) -> Result<GetGraphOutput, CodeGraphError> {
        let graph = self.get_graph(input.graph_id)?;
        Ok(GetGraphOutput {
            graph_id: input.graph_id,
            graph,
            retrieved_at: Utc::now(),
        })
    }

    async fn delete_graph(&self, graph_id: Uuid) -> Result<(), CodeGraphError> {
        let mut graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.remove(&graph_id);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CodeGraphAnalyzerImpl
// ---------------------------------------------------------------------------

/// In-memory implementation of CodeGraphAnalyzer.
///
/// Uses DFS traversal for cycle detection, root/leaf identification,
/// and transitive dependency analysis. All analysis requires a sealed graph.
pub struct CodeGraphAnalyzerImpl {
    /// Reference to the graph store for accessing graphs by ID.
    graphs: Mutex<HashMap<Uuid, CodeGraph>>,
}

impl CodeGraphAnalyzerImpl {
    /// Create a new CodeGraphAnalyzerImpl sharing the same graph store.
    pub fn new(graphs: HashMap<Uuid, CodeGraph>) -> Self {
        Self {
            graphs: Mutex::new(graphs),
        }
    }
}

#[async_trait]
impl CodeGraphAnalyzer for CodeGraphAnalyzerImpl {
    async fn analyze_dependencies(
        &self,
        input: AnalyzeDependenciesInput,
    ) -> Result<AnalyzeDependenciesOutput, CodeGraphError> {
        let graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&input.graph_id).ok_or_else(|| {
            CodeGraphError::InvalidOperation {
                reason: format!("Graph not found: {}", input.graph_id),
            }
        })?;

        if !graph.sealed {
            return Err(CodeGraphError::InvalidOperation {
                reason: "Graph must be sealed before analysis".to_string(),
            });
        }

        // Build adjacency maps
        let mut in_degree: HashMap<Uuid, usize> = HashMap::new();
        let mut out_degree: HashMap<Uuid, usize> = HashMap::new();
        for node in &graph.nodes {
            in_degree.entry(node.id).or_insert(0);
            out_degree.entry(node.id).or_insert(0);
        }
        for edge in &graph.edges {
            *in_degree.entry(edge.target_id).or_insert(0) += 1;
            *out_degree.entry(edge.source_id).or_insert(0) += 1;
        }

        // Root nodes = no incoming edges (no dependencies)
        let root_nodes: Vec<ModuleNode> = graph
            .nodes
            .iter()
            .filter(|n| *in_degree.get(&n.id).unwrap_or(&0) == 0)
            .cloned()
            .collect();

        // Leaf nodes = no outgoing edges (no dependents)
        let leaf_nodes: Vec<ModuleNode> = graph
            .nodes
            .iter()
            .filter(|n| *out_degree.get(&n.id).unwrap_or(&0) == 0)
            .cloned()
            .collect();

        // Cycle detection via DFS
        let cycles = self.detect_cycles_in_graph(graph)?;

        Ok(AnalyzeDependenciesOutput {
            graph_id: input.graph_id,
            total_nodes: graph.node_count() as u32,
            total_edges: graph.edge_count() as u32,
            cycle_count: cycles.len() as u32,
            cycle_paths: cycles,
            root_nodes,
            leaf_nodes,
            analyzed_at: Utc::now(),
        })
    }

    async fn analyze_impact(
        &self,
        input: ImpactAnalysisInput,
    ) -> Result<ImpactAnalysisOutput, CodeGraphError> {
        let graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&input.graph_id).ok_or_else(|| {
            CodeGraphError::InvalidOperation {
                reason: format!("Graph not found: {}", input.graph_id),
            }
        })?;

        if !graph.sealed {
            return Err(CodeGraphError::InvalidOperation {
                reason: "Graph must be sealed before impact analysis".to_string(),
            });
        }

        let target_node = graph
            .get_node(input.node_id)
            .cloned()
            .ok_or(CodeGraphError::NodeNotFound {
                node_id: input.node_id,
            })?;

        // BFS to find all transitive dependents
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut impact_chains: Vec<ImpactChain> = Vec::new();

        // Build forward adjacency (source → targets)
        let mut forward: HashMap<Uuid, Vec<(Uuid, String)>> = HashMap::new();
        for edge in &graph.edges {
            // Follow target direction: if A imports B, changing B impacts A
            // So from source (provider) → target (consumer)
            let name = graph
                .get_node(edge.target_id)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            forward
                .entry(edge.source_id)
                .or_default()
                .push((edge.target_id, name));
        }

        // Wait, the dependency direction: if A depends on B, edge.source = B, edge.target = A
        // Changing B impacts A, so we follow edge.target from edge.source
        // Actually we need: for each edge where source = node, target depends on source
        // So the impact flows from source_id → target_id

        let mut impact_adj: HashMap<Uuid, Vec<(Uuid, String)>> = HashMap::new();
        for edge in &graph.edges {
            let name = graph
                .get_node(edge.target_id)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            impact_adj
                .entry(edge.source_id)
                .or_default()
                .push((edge.target_id, name));
        }

        queue.push_back((input.node_id, 0u32, vec![target_node.name.clone()]));
        visited.insert(input.node_id);

        while let Some((current, depth, path)) = queue.pop_front() {
            if depth > 0 {
                let affected_node = graph.get_node(current).cloned().ok_or_else(|| {
                    CodeGraphError::NodeNotFound { node_id: current }
                })?;
                impact_chains.push(ImpactChain {
                    affected_node,
                    depth,
                    path: path.clone(),
                });
            }

            if depth >= input.max_depth {
                continue;
            }

            if let Some(dependents) = impact_adj.get(&current) {
                for (next_id, next_name) in dependents {
                    if !visited.contains(next_id) {
                        visited.insert(*next_id);
                        let mut new_path = path.clone();
                        new_path.push(next_name.clone());
                        queue.push_back((*next_id, depth + 1, new_path));
                    }
                }
            }
        }

        let direct_impact = impact_chains.iter().filter(|c| c.depth == 1).count() as u32;

        Ok(ImpactAnalysisOutput {
            target_node,
            direct_impact_count: direct_impact,
            total_impact_count: impact_chains.len() as u32,
            impact_chains,
            analyzed_at: Utc::now(),
        })
    }

    async fn detect_cycles(
        &self,
        graph_id: Uuid,
    ) -> Result<Vec<Vec<String>>, CodeGraphError> {
        let graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&graph_id).ok_or_else(|| {
            CodeGraphError::InvalidOperation {
                reason: format!("Graph not found: {}", graph_id),
            }
        })?;

        self.detect_cycles_in_graph(graph)
    }

    async fn has_circular_dependencies(
        &self,
        graph_id: Uuid,
        node_id: Uuid,
    ) -> Result<bool, CodeGraphError> {
        let cycles = self.detect_cycles(graph_id).await?;
        Ok(cycles.iter().any(|cycle| cycle.contains(&node_id.to_string())))
    }
}

impl CodeGraphAnalyzerImpl {
    /// Detect all cycles in a graph using DFS with coloring.
    fn detect_cycles_in_graph(
        &self,
        graph: &CodeGraph,
    ) -> Result<Vec<Vec<String>>, CodeGraphError> {
        let mut cycles: Vec<Vec<String>> = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path: Vec<Uuid> = Vec::new();

        // Build adjacency: for each edge (source → target), target depends on source
        // So from target, we can find what depends on it (dependents go to targets)
        // For cycles we want: if A depends on B and B depends on A
        // Following edges from source (provider) → target (consumer)
        let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for edge in &graph.edges {
            adj.entry(edge.source_id).or_default().push(edge.target_id);
        }

        for node in &graph.nodes {
            if !visited.contains(&node.id) {
                self.dfs_cycle_detect(
                    node.id,
                    &adj,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    graph,
                    &mut cycles,
                );
            }
        }

        Ok(cycles)
    }

    #[allow(clippy::too_many_arguments)]
    fn dfs_cycle_detect(
        &self,
        node_id: Uuid,
        adj: &HashMap<Uuid, Vec<Uuid>>,
        visited: &mut HashSet<Uuid>,
        rec_stack: &mut HashSet<Uuid>,
        path: &mut Vec<Uuid>,
        graph: &CodeGraph,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node_id);
        rec_stack.insert(node_id);
        path.push(node_id);

        if let Some(neighbors) = adj.get(&node_id) {
            for &neighbor in neighbors {
                // Skip self-loops
                if neighbor == node_id {
                    continue;
                }
                if !visited.contains(&neighbor) {
                    self.dfs_cycle_detect(neighbor, adj, visited, rec_stack, path, graph, cycles);
                } else if rec_stack.contains(&neighbor) {
                    // Found a cycle — reconstruct path
                    let cycle_start = path.iter().position(|&id| id == neighbor).unwrap_or(0);
                    let cycle_path: Vec<String> = path[cycle_start..]
                        .iter()
                        .map(|id| {
                            graph
                                .get_node(*id)
                                .map(|n| n.name.clone())
                                .unwrap_or_else(|| id.to_string())
                        })
                        .collect();
                    if !cycles.contains(&cycle_path) {
                        cycles.push(cycle_path);
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(&node_id);
    }
}

// ---------------------------------------------------------------------------
// CodeGraphFormatterImpl
// ---------------------------------------------------------------------------

/// Implementation of CodeGraphFormatter that produces output in various
/// text-based formats (Mermaid, DOT, Tree, JSON, List).
///
/// All formatting is stateless and deterministic.
pub struct CodeGraphFormatterImpl;

impl CodeGraphFormatterImpl {
    /// Create a new CodeGraphFormatterImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodeGraphFormatterImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CodeGraphFormatter for CodeGraphFormatterImpl {
    async fn format(&self, input: FormatGraphInput) -> Result<FormatGraphOutput, CodeGraphError> {
        let output = match input.format {
            OutputFormat::Mermaid => self.format_mermaid_inner(&input.graph)?,
            OutputFormat::Dot => self.format_dot_inner(&input.graph)?,
            OutputFormat::Tree => self.format_tree_inner(&input.graph)?,
            OutputFormat::Json => self.format_json_inner(&input.graph)?,
            OutputFormat::List => self.format_list_inner(&input.graph)?,
        };

        let output_size = output.len() as u64;

        Ok(FormatGraphOutput {
            output,
            format: input.format,
            output_size,
            formatted_at: Utc::now(),
        })
    }

    async fn format_mermaid(&self, _graph_id: Uuid) -> Result<String, CodeGraphError> {
        Err(CodeGraphError::InvalidOperation {
            reason: "format_mermaid requires a graph instance, use format() instead".to_string(),
        })
    }

    async fn format_dot(&self, _graph_id: Uuid) -> Result<String, CodeGraphError> {
        Err(CodeGraphError::InvalidOperation {
            reason: "format_dot requires a graph instance, use format() instead".to_string(),
        })
    }

    async fn format_tree(&self, _graph_id: Uuid) -> Result<String, CodeGraphError> {
        Err(CodeGraphError::InvalidOperation {
            reason: "format_tree requires a graph instance, use format() instead".to_string(),
        })
    }

    async fn format_json(&self, _graph_id: Uuid) -> Result<String, CodeGraphError> {
        Err(CodeGraphError::InvalidOperation {
            reason: "format_json requires a graph instance, use format() instead".to_string(),
        })
    }

    async fn format_list(&self, _graph_id: Uuid) -> Result<String, CodeGraphError> {
        Err(CodeGraphError::InvalidOperation {
            reason: "format_list requires a graph instance, use format() instead".to_string(),
        })
    }
}

impl CodeGraphFormatterImpl {
    /// Format as Mermaid.js flowchart.
    fn format_mermaid_inner(&self, graph: &CodeGraph) -> Result<String, CodeGraphError> {
        let mut output = String::from("graph TD;\n");

        // Add node declarations
        for node in &graph.nodes {
            let safe_name = node.name.replace(['\"', '(', ')', '[', ']', '{', '}'], "_");
            match node.kind {
                NodeKind::File => {
                    output.push_str(&format!("    {}[\"{}\"];\n", node.id, safe_name));
                }
                NodeKind::Package => {
                    output.push_str(&format!("    {}[\"{}\"];\n", node.id, safe_name));
                }
                NodeKind::Component => {
                    output.push_str(&format!("    {}[\"{}\"];\n", node.id, safe_name));
                }
                NodeKind::External => {
                    output.push_str(&format!("    {}[\"{}\"];\n", node.id, safe_name));
                }
                _ => {
                    output.push_str(&format!("    {}[\"{}\"];\n", node.id, safe_name));
                }
            }
        }

        // Add edge declarations
        for edge in &graph.edges {
            let label = edge
                .label
                .as_deref()
                .unwrap_or_else(|| edge.kind.as_str());
            output.push_str(&format!(
                "    {} -->|\"{}\"| {};\n",
                edge.source_id, label, edge.target_id
            ));
        }

        Ok(output)
    }

    /// Format as Graphviz DOT.
    fn format_dot_inner(&self, graph: &CodeGraph) -> Result<String, CodeGraphError> {
        let mut output = String::from("digraph CodeGraph {\n");
        output.push_str("    rankdir=LR;\n");
        output.push_str(&format!(
            "    label=\"{}\";\n",
            graph.metadata.name
        ));
        output.push_str("    node [shape=box, style=rounded];\n\n");

        for node in &graph.nodes {
            let safe_name = node.name.replace('\"', "\\\"");
            let label = if node.metadata.is_empty() {
                format!("{}\n[{}]", safe_name, node.kind.as_str())
            } else {
                format!("{}\n[{}]", safe_name, node.kind.as_str())
            };
            output.push_str(&format!("    {} [label=\"{}\"];\n", node.id, label));
        }

        output.push('\n');

        for edge in &graph.edges {
            let label = edge
                .label
                .as_deref()
                .unwrap_or_else(|| edge.kind.as_str());
            output.push_str(&format!(
                "    {} -> {} [label=\"{}\"];\n",
                edge.source_id, edge.target_id, label
            ));
        }

        output.push_str("}\n");
        Ok(output)
    }

    /// Format as indented text tree.
    fn format_tree_inner(&self, graph: &CodeGraph) -> Result<String, CodeGraphError> {
        let mut output = String::new();

        // Find root nodes (no incoming edges)
        let has_incoming: HashSet<Uuid> =
            graph.edges.iter().map(|e| e.target_id).collect();
        let root_ids: Vec<&ModuleNode> = graph
            .nodes
            .iter()
            .filter(|n| !has_incoming.contains(&n.id))
            .collect();

        // Build adjacency from provider to consumer
        let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for edge in &graph.edges {
            // Consumer is edge.target_id, depends on edge.source_id
            // Tree direction: parent (source) → children (targets)
            children
                .entry(edge.source_id)
                .or_default()
                .push(edge.target_id);
        }

        let mut visited = HashSet::new();

        for root in &root_ids {
            self.write_tree_node(graph, root.id, &children, 0, &mut visited, &mut output);
        }

        Ok(output)
    }

    fn write_tree_node(
        &self,
        graph: &CodeGraph,
        node_id: Uuid,
        children: &HashMap<Uuid, Vec<Uuid>>,
        depth: usize,
        visited: &mut HashSet<Uuid>,
        output: &mut String,
    ) {
        if visited.contains(&node_id) {
            return;
        }
        visited.insert(node_id);

        let indent = "  ".repeat(depth);
        if let Some(node) = graph.get_node(node_id) {
            output.push_str(&format!(
                "{}- {} [{}]\n",
                indent,
                node.name,
                node.kind.as_str()
            ));
        }

        if let Some(child_ids) = children.get(&node_id) {
            for child_id in child_ids {
                self.write_tree_node(graph, *child_id, children, depth + 1, visited, output);
            }
        }
    }

    /// Format as JSON.
    fn format_json_inner(&self, graph: &CodeGraph) -> Result<String, CodeGraphError> {
        serde_json::to_string_pretty(graph).map_err(|e| CodeGraphError::SerializationError {
            detail: format!("JSON formatting failed: {}", e),
        })
    }

    /// Format as adjacency list.
    fn format_list_inner(&self, graph: &CodeGraph) -> Result<String, CodeGraphError> {
        let mut output = String::new();

        // Build adjacency: module → its dependencies (incoming edges' sources)
        let mut deps_map: HashMap<Uuid, Vec<(Uuid, String)>> = HashMap::new();
        for edge in &graph.edges {
            // module at edge.target_id depends on edge.source_id
            let source_name = graph
                .get_node(edge.source_id)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            deps_map
                .entry(edge.target_id)
                .or_default()
                .push((edge.source_id, source_name));
        }

        // Also build dependents: module → what depends on it
        let mut dep_of_map: HashMap<Uuid, Vec<(Uuid, String)>> = HashMap::new();
        for edge in &graph.edges {
            let target_name = graph
                .get_node(edge.target_id)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            dep_of_map
                .entry(edge.source_id)
                .or_default()
                .push((edge.target_id, target_name));
        }

        for node in &graph.nodes {
            output.push_str(&format!("[{}] {}\n", node.kind.as_str(), node.name));
            output.push_str(&format!("  Path: {}\n", node.path));

            output.push_str("  Dependencies:\n");
            if let Some(deps) = deps_map.get(&node.id) {
                for (_, name) in deps {
                    output.push_str(&format!("    - {}\n", name));
                }
            } else {
                output.push_str("    (none)\n");
            }

            output.push_str("  Depended-on-by:\n");
            if let Some(dependents) = dep_of_map.get(&node.id) {
                for (_, name) in dependents {
                    output.push_str(&format!("    - {}\n", name));
                }
            } else {
                output.push_str("    (none)\n");
            }

            output.push('\n');
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// CodeGraphImporterImpl
// ---------------------------------------------------------------------------

/// In-memory implementation of CodeGraphImporter.
///
/// Imports nodes and edges into the shared graph store, creating a new
/// graph if no graph_id is provided.
pub struct CodeGraphImporterImpl {
    graphs: Mutex<HashMap<Uuid, CodeGraph>>,
}

impl CodeGraphImporterImpl {
    /// Create a new CodeGraphImporterImpl.
    pub fn new(graphs: HashMap<Uuid, CodeGraph>) -> Self {
        Self {
            graphs: Mutex::new(graphs),
        }
    }
}

#[async_trait]
impl CodeGraphImporter for CodeGraphImporterImpl {
    async fn import(
        &self,
        input: ImportInput,
    ) -> Result<ImportOutput, CodeGraphError> {
        let mut graphs = self.graphs.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph_id = match input.graph_id {
            Some(id) => {
                // Validate graph exists and is not sealed
                let graph = graphs.get(&id).ok_or_else(|| {
                    CodeGraphError::InvalidOperation {
                        reason: format!("Graph not found: {}", id),
                    }
                })?;
                if graph.sealed {
                    return Err(CodeGraphError::GraphSealed {
                        operation: "import".to_string(),
                    });
                }
                id
            }
            None => {
                // Create new graph
                let now = Utc::now();
                let metadata = input.metadata.unwrap_or(GraphMetadata {
                    name: "imported".to_string(),
                    source: "importer".to_string(),
                    created_at: now,
                    description: "Imported graph".to_string(),
                    total_modules_scanned: input.nodes.len() as u64,
                    schema_version: "1.0.0".to_string(),
                });
                let id = Uuid::new_v4();
                graphs.insert(id, CodeGraph::new(metadata));
                id
            }
        };

        let graph = graphs.get_mut(&graph_id).ok_or_else(|| {
            CodeGraphError::InvalidOperation {
                reason: format!("Graph not found: {}", graph_id),
            }
        })?;

        let nodes_imported = input.nodes.len() as u32;
        let edges_imported = input.edges.len() as u32;

        // Pre-validate all edges' endpoints exist
        let node_ids: HashSet<Uuid> = input.nodes.iter().map(|n| n.id).collect();
        for edge in &input.edges {
            if !node_ids.contains(&edge.source_id) && graph.get_node(edge.source_id).is_none() {
                return Err(CodeGraphError::NodeNotFound {
                    node_id: edge.source_id,
                });
            }
            if !node_ids.contains(&edge.target_id) && graph.get_node(edge.target_id).is_none() {
                return Err(CodeGraphError::NodeNotFound {
                    node_id: edge.target_id,
                });
            }
        }

        for node in input.nodes {
            graph.add_node(node)?;
        }

        for edge in input.edges {
            graph.add_edge(edge)?;
        }

        let total_nodes = graph.node_count() as u32;
        let total_edges = graph.edge_count() as u32;

        Ok(ImportOutput {
            graph_id,
            nodes_imported,
            edges_imported,
            total_nodes,
            total_edges,
        })
    }
}
