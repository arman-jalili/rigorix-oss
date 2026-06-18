//! Core code graph data structures: CodeGraph, ModuleNode, ModuleEdge.
//!
//! @canonical .pi/architecture/modules/code-graph.md#graph
//! Implements: Contract Freeze — CodeGraph, ModuleNode, ModuleEdge
//! Issue: issue-contract-freeze
//!
//! Defines the core data structures for code dependency analysis:
//! - `CodeGraph`: A directed graph of code modules with typed edges
//! - `ModuleNode`: A single code module (file, package, or component)
//! - `ModuleEdge`: A typed relationship between two modules
//! - `NodeKind`: Classification of what a module node represents
//! - `EdgeKind`: Classification of relationship type between modules
//! - `GraphMetadata`: Metadata about the graph itself
//!
//! # Contract (Frozen)
//! - CodeGraph supports two-phase construction (add_node → seal)
//! - ModuleNode carries identity, kind, path, and metadata
//! - ModuleEdge is directional with a typed relationship
//! - GraphMetadata captures source, timestamp, and tool info
//! - All types are serializable for persistence and API responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use super::error::CodeGraphError;

// ---------------------------------------------------------------------------
// CodeGraph — Core Code Dependency Graph Structure
// ---------------------------------------------------------------------------

/// A directed graph of code modules and their dependency relationships.
///
/// CodeGraph represents the dependency structure of a codebase. It supports
/// two-phase construction:
/// 1. **Phase 1 (Open):** Nodes and edges are added via `add_node` / `add_edge`.
///    The graph can accept new elements but cannot be queried for traversal.
/// 2. **Phase 2 (Sealed):** `seal()` finalizes the graph, making it ready for
///    traversal, analysis, and formatting. After sealing, no new nodes or edges
///    can be added.
///
/// # Contract (Frozen)
/// - Two-phase construction: add_node/add_edge → seal → query/analyze
/// - Duplicate node detection during construction
/// - Edge validation (both endpoints must exist in the graph)
/// - Serializable for persistence and API responses
/// - Metadata captures graph provenance (source, tool, timestamp)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraph {
    /// The nodes (modules) in this graph.
    pub nodes: Vec<ModuleNode>,

    /// The edges (relationships) between nodes.
    pub edges: Vec<ModuleEdge>,

    /// Node ID → position index for O(1) lookups.
    #[serde(skip)]
    node_index: HashMap<Uuid, usize>,

    /// Whether the graph has been sealed (frozen for analysis).
    pub sealed: bool,

    /// Metadata about this graph and its provenance.
    pub metadata: GraphMetadata,
}

/// Metadata about a CodeGraph instance.
///
/// Captures provenance information including source, tool, and timestamp
/// for audit and traceability purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    /// Human-readable name for this graph (e.g., "cargo-deps", "ts-imports").
    pub name: String,

    /// The tool or process that produced this graph (e.g., "cargo-deps", "ts-metrics").
    pub source: String,

    /// ISO 8601 timestamp when the graph was created.
    pub created_at: DateTime<Utc>,

    /// Human-readable description of what this graph represents.
    pub description: String,

    /// Total number of modules analyzed to produce this graph.
    /// May differ from `nodes.len()` if filtering was applied.
    pub total_modules_scanned: u64,

    /// Version of the graph schema.
    pub schema_version: String,
}

impl CodeGraph {
    /// Create a new empty CodeGraph in open state.
    ///
    /// Nodes and edges can be added until `seal()` is called.
    pub fn new(metadata: GraphMetadata) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            node_index: HashMap::new(),
            sealed: false,
            metadata,
        }
    }

    /// Add a module node to the graph.
    ///
    /// Returns an error if:
    /// - The graph is already sealed
    /// - A node with the same ID already exists
    pub fn add_node(&mut self, node: ModuleNode) -> Result<(), CodeGraphError> {
        if self.sealed {
            return Err(CodeGraphError::GraphSealed {
                operation: "add_node".to_string(),
            });
        }
        if self.node_index.contains_key(&node.id) {
            return Err(CodeGraphError::DuplicateNodeId { id: node.id });
        }
        let idx = self.nodes.len();
        self.node_index.insert(node.id, idx);
        self.nodes.push(node);
        Ok(())
    }

    /// Add a directed edge between two module nodes.
    ///
    /// Returns an error if:
    /// - The graph is already sealed
    /// - Either endpoint does not exist as a node in this graph
    /// - The edge would create a duplicate
    pub fn add_edge(&mut self, edge: ModuleEdge) -> Result<(), CodeGraphError> {
        if self.sealed {
            return Err(CodeGraphError::GraphSealed {
                operation: "add_edge".to_string(),
            });
        }
        if !self.node_index.contains_key(&edge.source_id) {
            return Err(CodeGraphError::NodeNotFound {
                node_id: edge.source_id,
            });
        }
        if !self.node_index.contains_key(&edge.target_id) {
            return Err(CodeGraphError::NodeNotFound {
                node_id: edge.target_id,
            });
        }
        // Check for duplicate edges
        if self.edges.iter().any(|e| {
            e.source_id == edge.source_id && e.target_id == edge.target_id && e.kind == edge.kind
        }) {
            return Err(CodeGraphError::DuplicateEdge {
                source_id: edge.source_id,
                target_id: edge.target_id,
                kind: edge.kind.clone(),
            });
        }
        self.edges.push(edge);
        Ok(())
    }

    /// Seal the graph, making it ready for querying and analysis.
    ///
    /// After sealing, no more nodes or edges can be added. The graph
    /// is ready for traversal, formatting, and analysis operations.
    ///
    /// # Errors
    /// - `CodeGraphError::GraphSealed` if already sealed
    /// - `CodeGraphError::EmptyGraph` if no nodes have been added
    pub fn seal(&mut self) -> Result<(), CodeGraphError> {
        if self.sealed {
            return Err(CodeGraphError::GraphSealed {
                operation: "seal".to_string(),
            });
        }
        if self.nodes.is_empty() {
            return Err(CodeGraphError::EmptyGraph);
        }
        self.sealed = true;
        Ok(())
    }

    /// Rebuild the node index from the node list.
    ///
    /// Called internally to restore the index after deserialization.
    #[allow(dead_code)]
    fn rebuild_index(&mut self) {
        self.node_index.clear();
        for (i, node) in self.nodes.iter().enumerate() {
            self.node_index.insert(node.id, i);
        }
    }

    /// Get a node by its ID.
    pub fn get_node(&self, node_id: Uuid) -> Option<&ModuleNode> {
        self.node_index
            .get(&node_id)
            .and_then(|&idx| self.nodes.get(idx))
    }

    /// Get a mutable reference to a node by its ID.
    pub fn get_node_mut(&mut self, node_id: Uuid) -> Option<&mut ModuleNode> {
        let idx = *self.node_index.get(&node_id)?;
        self.nodes.get_mut(idx)
    }

    /// Get all edges where the given node is the source.
    pub fn outgoing_edges(&self, node_id: Uuid) -> Vec<&ModuleEdge> {
        self.edges
            .iter()
            .filter(|e| e.source_id == node_id)
            .collect()
    }

    /// Get all edges where the given node is the target.
    pub fn incoming_edges(&self, node_id: Uuid) -> Vec<&ModuleEdge> {
        self.edges
            .iter()
            .filter(|e| e.target_id == node_id)
            .collect()
    }

    /// Get the set of direct dependencies (nodes this node depends on).
    pub fn dependencies(&self, node_id: Uuid) -> Vec<Uuid> {
        self.incoming_edges(node_id)
            .iter()
            .map(|e| e.source_id)
            .collect()
    }

    /// Get the set of direct dependents (nodes that depend on this node).
    pub fn dependents(&self, node_id: Uuid) -> Vec<Uuid> {
        self.outgoing_edges(node_id)
            .iter()
            .map(|e| e.target_id)
            .collect()
    }

    /// Return an iterator over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &ModuleNode> {
        self.nodes.iter()
    }

    /// Return an iterator over all edges.
    pub fn edges(&self) -> impl Iterator<Item = &ModuleEdge> {
        self.edges.iter()
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Whether the graph is empty (no nodes).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Whether the graph is sealed.
    pub fn is_sealed(&self) -> bool {
        self.sealed
    }
}

// ---------------------------------------------------------------------------
// ModuleNode — A Single Code Module
// ---------------------------------------------------------------------------

/// A single code module in the dependency graph.
///
/// Represents any type of code module — a file, a package, a crate, a
/// component, or a logical grouping. Each module carries:
/// - A unique identifier and human-readable name
/// - A kind classification (what type of module this is)
/// - A file system path or logical location
/// - Optional metadata for additional context
///
/// # Contract (Frozen)
/// - Every node has a unique UUID
/// - `kind` classifies what the node represents (file, package, etc.)
/// - `path` is the canonical location (file path or module path)
/// - `metadata` is an extensible key-value map for additional context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleNode {
    /// Globally unique identifier for this module node.
    pub id: Uuid,

    /// Human-readable name (e.g., "parser", "lexer", "core").
    pub name: String,

    /// The kind of module this node represents.
    pub kind: NodeKind,

    /// Canonical path to the module (file path, module path, or package name).
    pub path: String,

    /// Optional metadata key-value pairs for extensibility.
    pub metadata: HashMap<String, String>,
}

impl ModuleNode {
    /// Create a new ModuleNode.
    pub fn new(id: Uuid, name: impl Into<String>, kind: NodeKind, path: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            path: path.into(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new ModuleNode with metadata.
    pub fn with_metadata(
        id: Uuid,
        name: impl Into<String>,
        kind: NodeKind,
        path: impl Into<String>,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            path: path.into(),
            metadata,
        }
    }
}

// ---------------------------------------------------------------------------
// NodeKind — Classification of Module Nodes
// ---------------------------------------------------------------------------

/// Classification of what a module node represents.
///
/// # Contract (Frozen)
/// - `File`: A single source file (e.g., parser.rs, utils.ts)
/// - `Package`: A package or crate (e.g., Cargo.toml, package.json)
/// - `Component`: A logical component or bounded context
/// - `Directory`: A directory containing modules
/// - `External`: An external dependency (not part of the codebase)
/// - `Aggregate`: An aggregation of multiple modules for analysis
/// - `Custom(String)`: User-defined node kind
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// A single source file.
    File,
    /// A package, crate, or module (e.g., Cargo.toml, package.json).
    Package,
    /// A logical component or bounded context.
    Component,
    /// A directory containing modules.
    Directory,
    /// An external dependency (not part of the codebase).
    External,
    /// An aggregation of multiple modules for analysis.
    Aggregate,
    /// User-defined node kind.
    Custom(String),
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl NodeKind {
    /// Returns the canonical snake_case name of this node kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::File => "file",
            NodeKind::Package => "package",
            NodeKind::Component => "component",
            NodeKind::Directory => "directory",
            NodeKind::External => "external",
            NodeKind::Aggregate => "aggregate",
            NodeKind::Custom(_) => "custom",
        }
    }
}

// ---------------------------------------------------------------------------
// ModuleEdge — Typed Relationship Between Two Modules
// ---------------------------------------------------------------------------

/// A typed, directed relationship between two module nodes.
///
/// Each edge represents a dependency or relationship from `source_id` to
/// `target_id`. The direction follows dependency convention: if module A
/// depends on module B, the edge goes from B (source) to A (target).
///
/// # Contract (Frozen)
/// - Every edge is directional (source → target)
/// - `kind` classifies the relationship type
/// - `weight` expresses the strength or count of the relationship
/// - Both endpoints must exist as nodes in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleEdge {
    /// The UUID of the source module (the provider/dependency).
    pub source_id: Uuid,

    /// The UUID of the target module (the consumer/dependent).
    pub target_id: Uuid,

    /// The kind of relationship between source and target.
    pub kind: EdgeKind,

    /// Optional weight or count of the relationship.
    /// Higher weight = stronger dependency relationship.
    pub weight: u64,

    /// Optional label for display purposes.
    pub label: Option<String>,
}

impl ModuleEdge {
    /// Create a new ModuleEdge.
    pub fn new(source_id: Uuid, target_id: Uuid, kind: EdgeKind) -> Self {
        Self {
            source_id,
            target_id,
            kind,
            weight: 1,
            label: None,
        }
    }

    /// Create a new ModuleEdge with weight and label.
    pub fn with_details(
        source_id: Uuid,
        target_id: Uuid,
        kind: EdgeKind,
        weight: u64,
        label: Option<String>,
    ) -> Self {
        Self {
            source_id,
            target_id,
            kind,
            weight,
            label,
        }
    }
}

// ---------------------------------------------------------------------------
// EdgeKind — Classification of Module Relationships
// ---------------------------------------------------------------------------

/// Classification of the relationship type between two modules.
///
/// # Contract (Frozen)
/// - `Imports`: A direct import/use relationship
/// - `Extends`: Inheritance or extension relationship
/// - `Implements`: Implementation of an interface or trait
/// - `DependsOn`: Generic dependency relationship
/// - `Contains`: Containment relationship (parent → child)
/// - `References`: A reference to a symbol in another module
/// - `Calls`: Function or method call across module boundaries
/// - `Custom(String)`: User-defined relationship kind
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    /// A direct import or use relationship (e.g., `use`, `import`).
    Imports,
    /// Inheritance or extension relationship (e.g., `extends`, `inherits`).
    Extends,
    /// Implementation of an interface, trait, or contract.
    Implements,
    /// Generic dependency relationship.
    DependsOn,
    /// Containment relationship (parent contains child module).
    Contains,
    /// A reference to a symbol in another module.
    References,
    /// Function or method call across module boundaries.
    Calls,
    /// User-defined relationship kind.
    Custom(String),
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl EdgeKind {
    /// Returns the canonical snake_case name of this edge kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Imports => "imports",
            EdgeKind::Extends => "extends",
            EdgeKind::Implements => "implements",
            EdgeKind::DependsOn => "depends_on",
            EdgeKind::Contains => "contains",
            EdgeKind::References => "references",
            EdgeKind::Calls => "calls",
            EdgeKind::Custom(_) => "custom",
        }
    }
}
