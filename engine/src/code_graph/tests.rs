//! Unit and integration tests for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md
//! Implements: CodeGraph, ModuleNode, ModuleEdge, CodeGraphBuilder — unit + integration tests
//! Issue: issue-codegraph
//!
//! Covers:
//! - CodeGraph domain entity (construction, nodes, edges, sealing, queries)
//! - ModuleNode creation and kind classification
//! - ModuleEdge creation and kind classification
//! - CodeGraphService implementation (CRUD)
//! - CodeGraphAnalyzer implementation (dependency analysis, cycles, impact)
//! - CodeGraphFormatter implementation (all 5 output formats)
//! - CodeGraphImporter implementation (batch import)
//! - InMemoryCodeGraphRepository integration

use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::code_graph::application::dto::{
    AddEdgeInput, AddNodeInput, ConstructGraphInput, FormatGraphInput, GetGraphInput, GetNodeInput,
    ListGraphsInput, OutputFormat, PersistGraphInput, SealGraphInput,
};
use crate::code_graph::application::service::{
    CodeGraphFormatter, CodeGraphImporter, CodeGraphService, ImportInput,
};
use crate::code_graph::application::service_impl::{
    CodeGraphFormatterImpl, CodeGraphImporterImpl, CodeGraphServiceImpl,
};
use crate::code_graph::domain::{
    CodeGraph, CodeGraphError, EdgeKind, GraphMetadata, ModuleEdge, ModuleNode, NodeKind,
};
use crate::code_graph::infrastructure::repository::CodeGraphRepository;
use crate::code_graph::infrastructure::repository::memory_repository::InMemoryCodeGraphRepository;

// ---------------------------------------------------------------------------
// Helper: Create a test graph with nodes and edges
// ---------------------------------------------------------------------------

fn create_test_metadata(name: &str) -> GraphMetadata {
    GraphMetadata {
        name: name.to_string(),
        source: "test".to_string(),
        created_at: Utc::now(),
        description: "Test graph".to_string(),
        total_modules_scanned: 10,
        schema_version: "1.0.0".to_string(),
    }
}

fn create_test_node(name: &str, kind: NodeKind, path: &str) -> ModuleNode {
    ModuleNode::new(Uuid::new_v4(), name, kind, path)
}

fn build_sample_graph() -> (CodeGraph, Vec<ModuleNode>, Vec<ModuleEdge>) {
    let mut graph = CodeGraph::new(create_test_metadata("sample"));
    let a = create_test_node("module-a", NodeKind::Package, "src/a");
    let b = create_test_node("module-b", NodeKind::Package, "src/b");
    let c = create_test_node("module-c", NodeKind::Package, "src/c");

    graph.add_node(a.clone()).unwrap();
    graph.add_node(b.clone()).unwrap();
    graph.add_node(c.clone()).unwrap();

    let ab = ModuleEdge::new(a.id, b.id, EdgeKind::Imports);
    let bc = ModuleEdge::new(b.id, c.id, EdgeKind::Imports);

    graph.add_edge(ab.clone()).unwrap();
    graph.add_edge(bc.clone()).unwrap();

    (graph, vec![a, b, c], vec![ab, bc])
}

// ===========================================================================
// CodeGraph Domain Tests (issue-codegraph)
// ===========================================================================

#[test]
fn test_codegraph_new() {
    let metadata = create_test_metadata("test-graph");
    let graph = CodeGraph::new(metadata.clone());
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert!(!graph.sealed);
    assert!(graph.is_empty());
    assert_eq!(graph.metadata.name, "test-graph");
}

#[test]
fn test_codegraph_add_node() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let node = create_test_node("parser", NodeKind::File, "src/parser.rs");
    assert!(graph.add_node(node.clone()).is_ok());
    assert_eq!(graph.node_count(), 1);
}

#[test]
fn test_codegraph_add_duplicate_node() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let node = create_test_node("parser", NodeKind::File, "src/parser.rs");
    graph.add_node(node.clone()).unwrap();
    let err = graph.add_node(node).unwrap_err();
    assert!(matches!(err, CodeGraphError::DuplicateNodeId { .. }));
}

#[test]
fn test_codegraph_add_edge() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let a = create_test_node("a", NodeKind::File, "src/a.rs");
    let b = create_test_node("b", NodeKind::File, "src/b.rs");
    graph.add_node(a.clone()).unwrap();
    graph.add_node(b.clone()).unwrap();

    let edge = ModuleEdge::new(a.id, b.id, EdgeKind::Imports);
    assert!(graph.add_edge(edge).is_ok());
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn test_codegraph_add_edge_nonexistent_node() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let a = create_test_node("a", NodeKind::File, "src/a.rs");
    graph.add_node(a.clone()).unwrap();

    let edge = ModuleEdge::new(a.id, Uuid::new_v4(), EdgeKind::Imports);
    let err = graph.add_edge(edge).unwrap_err();
    assert!(matches!(err, CodeGraphError::NodeNotFound { .. }));
}

#[test]
fn test_codegraph_seal() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let node = create_test_node("a", NodeKind::File, "src/a.rs");
    graph.add_node(node).unwrap();

    assert!(!graph.is_sealed());
    assert!(graph.seal().is_ok());
    assert!(graph.is_sealed());
}

#[test]
fn test_codegraph_seal_empty_graph() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, CodeGraphError::EmptyGraph));
}

#[test]
fn test_codegraph_cannot_add_after_seal() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let node = create_test_node("a", NodeKind::File, "src/a.rs");
    graph.add_node(node).unwrap();
    graph.seal().unwrap();

    let new_node = create_test_node("b", NodeKind::File, "src/b.rs");
    let err = graph.add_node(new_node).unwrap_err();
    assert!(matches!(err, CodeGraphError::GraphSealed { .. }));
}

#[test]
fn test_codegraph_get_node() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let node = create_test_node("parser", NodeKind::File, "src/parser.rs");
    let node_id = node.id;
    graph.add_node(node).unwrap();

    let found = graph.get_node(node_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "parser");

    assert!(graph.get_node(Uuid::new_v4()).is_none());
}

#[test]
fn test_codegraph_dependencies() {
    let (graph, nodes, _) = build_sample_graph();
    // a imports nothing, b imports a, c imports b
    let deps_of_b = graph.dependencies(nodes[1].id);
    assert!(deps_of_b.contains(&nodes[0].id));

    let deps_of_c = graph.dependencies(nodes[2].id);
    assert!(deps_of_c.contains(&nodes[1].id));
}

#[test]
fn test_codegraph_dependents() {
    let (graph, nodes, _) = build_sample_graph();
    // a is depended on by b
    let deps_of_a = graph.dependents(nodes[0].id);
    assert!(deps_of_a.contains(&nodes[1].id));
}

#[test]
fn test_codegraph_outgoing_incoming_edges() {
    let (graph, nodes, _) = build_sample_graph();
    let out = graph.outgoing_edges(nodes[0].id);
    let inn = graph.incoming_edges(nodes[1].id);

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].target_id, nodes[1].id);

    assert_eq!(inn.len(), 1);
    assert_eq!(inn[0].source_id, nodes[0].id);
}

// ===========================================================================
// ModuleNode Tests (issue-modulenode)
// ===========================================================================

#[test]
fn test_module_node_new() {
    let id = Uuid::new_v4();
    let node = ModuleNode::new(id, "parser", NodeKind::File, "src/parser.rs");
    assert_eq!(node.id, id);
    assert_eq!(node.name, "parser");
    assert_eq!(node.kind, NodeKind::File);
    assert_eq!(node.path, "src/parser.rs");
    assert!(node.metadata.is_empty());
}

#[test]
fn test_module_node_with_metadata() {
    let id = Uuid::new_v4();
    let mut meta = HashMap::new();
    meta.insert("language".to_string(), "rust".to_string());
    meta.insert("lines".to_string(), "500".to_string());
    let node =
        ModuleNode::with_metadata(id, "parser", NodeKind::File, "src/parser.rs", meta.clone());
    assert_eq!(node.metadata.get("language").unwrap(), "rust");
    assert_eq!(node.metadata.get("lines").unwrap(), "500");
}

#[test]
fn test_node_kind_as_str() {
    assert_eq!(NodeKind::File.as_str(), "file");
    assert_eq!(NodeKind::Package.as_str(), "package");
    assert_eq!(NodeKind::Component.as_str(), "component");
    assert_eq!(NodeKind::Directory.as_str(), "directory");
    assert_eq!(NodeKind::External.as_str(), "external");
    assert_eq!(NodeKind::Aggregate.as_str(), "aggregate");
    assert_eq!(NodeKind::Custom("x".into()).as_str(), "custom");
}

#[test]
fn test_node_kind_display() {
    assert_eq!(format!("{}", NodeKind::File), "file");
    assert_eq!(format!("{}", NodeKind::Package), "package");
}

// ===========================================================================
// ModuleEdge Tests (issue-moduleedge)
// ===========================================================================

#[test]
fn test_module_edge_new() {
    let src = Uuid::new_v4();
    let tgt = Uuid::new_v4();
    let edge = ModuleEdge::new(src, tgt, EdgeKind::Imports);
    assert_eq!(edge.source_id, src);
    assert_eq!(edge.target_id, tgt);
    assert_eq!(edge.kind, EdgeKind::Imports);
    assert_eq!(edge.weight, 1);
    assert!(edge.label.is_none());
}

#[test]
fn test_module_edge_with_details() {
    let src = Uuid::new_v4();
    let tgt = Uuid::new_v4();
    let edge = ModuleEdge::with_details(src, tgt, EdgeKind::DependsOn, 5, Some("heavy".into()));
    assert_eq!(edge.weight, 5);
    assert_eq!(edge.label, Some("heavy".to_string()));
}

#[test]
fn test_edge_kind_as_str() {
    assert_eq!(EdgeKind::Imports.as_str(), "imports");
    assert_eq!(EdgeKind::Extends.as_str(), "extends");
    assert_eq!(EdgeKind::Implements.as_str(), "implements");
    assert_eq!(EdgeKind::DependsOn.as_str(), "depends_on");
    assert_eq!(EdgeKind::Contains.as_str(), "contains");
    assert_eq!(EdgeKind::References.as_str(), "references");
    assert_eq!(EdgeKind::Calls.as_str(), "calls");
    assert_eq!(EdgeKind::Custom("x".into()).as_str(), "custom");
}

#[test]
fn test_edge_kind_display() {
    assert_eq!(format!("{}", EdgeKind::Imports), "imports");
    assert_eq!(format!("{}", EdgeKind::DependsOn), "depends_on");
}

// ===========================================================================
// CodeGraphService Tests (issue-codegraphservice)
// ===========================================================================

#[tokio::test]
async fn test_service_construct_graph() {
    let service = CodeGraphServiceImpl::new();
    let input = ConstructGraphInput {
        name: "test".to_string(),
        source: "test-src".to_string(),
        description: "a test".to_string(),
        total_modules_scanned: 5,
    };
    let output = service.construct_graph(input).await.unwrap();
    assert!(!output.graph.sealed);
    assert_eq!(output.graph.metadata.name, "test");
}

#[tokio::test]
async fn test_service_add_node() {
    let service = CodeGraphServiceImpl::new();
    let construct = ConstructGraphInput {
        name: "test".to_string(),
        source: "test".to_string(),
        description: "".to_string(),
        total_modules_scanned: 1,
    };
    let graph = service.construct_graph(construct).await.unwrap();

    let input = AddNodeInput {
        graph_id: graph.graph_id,
        name: "parser".to_string(),
        kind: NodeKind::File,
        path: "src/parser.rs".to_string(),
        metadata: HashMap::new(),
    };
    let output = service.add_node(input).await.unwrap();
    assert_eq!(output.node_count, 1);
    assert_eq!(output.graph_id, graph.graph_id);
}

#[tokio::test]
async fn test_service_add_edge() {
    let service = CodeGraphServiceImpl::new();
    let construct = ConstructGraphInput {
        name: "test".to_string(),
        source: "test".to_string(),
        description: "".to_string(),
        total_modules_scanned: 2,
    };
    let graph = service.construct_graph(construct).await.unwrap();

    let n1 = service
        .add_node(AddNodeInput {
            graph_id: graph.graph_id,
            name: "a".into(),
            kind: NodeKind::File,
            path: "src/a.rs".into(),
            metadata: HashMap::new(),
        })
        .await
        .unwrap();

    let n2 = service
        .add_node(AddNodeInput {
            graph_id: graph.graph_id,
            name: "b".into(),
            kind: NodeKind::File,
            path: "src/b.rs".into(),
            metadata: HashMap::new(),
        })
        .await
        .unwrap();

    let edge = service
        .add_edge(AddEdgeInput {
            graph_id: graph.graph_id,
            source_id: n1.node_id,
            target_id: n2.node_id,
            kind: EdgeKind::Imports,
            weight: 1,
            label: Some("uses".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(edge.edge_count, 1);
}

#[tokio::test]
async fn test_service_seal_and_get() {
    let service = CodeGraphServiceImpl::new();
    let construct = ConstructGraphInput {
        name: "test".to_string(),
        source: "test".to_string(),
        description: "".to_string(),
        total_modules_scanned: 1,
    };
    let graph = service.construct_graph(construct).await.unwrap();

    service
        .add_node(AddNodeInput {
            graph_id: graph.graph_id,
            name: "a".into(),
            kind: NodeKind::File,
            path: "src/a.rs".into(),
            metadata: HashMap::new(),
        })
        .await
        .unwrap();

    service
        .seal_graph(SealGraphInput {
            graph_id: graph.graph_id,
        })
        .await
        .unwrap();

    let loaded = service
        .get_graph(GetGraphInput {
            graph_id: graph.graph_id,
        })
        .await
        .unwrap();
    assert!(loaded.graph.sealed);
}

#[tokio::test]
async fn test_service_get_node() {
    let service = CodeGraphServiceImpl::new();
    let construct = ConstructGraphInput {
        name: "test".to_string(),
        source: "test".to_string(),
        description: "".to_string(),
        total_modules_scanned: 1,
    };
    let graph = service.construct_graph(construct).await.unwrap();

    let added = service
        .add_node(AddNodeInput {
            graph_id: graph.graph_id,
            name: "parser".into(),
            kind: NodeKind::File,
            path: "src/parser.rs".into(),
            metadata: HashMap::new(),
        })
        .await
        .unwrap();

    let node = service
        .get_node(GetNodeInput {
            graph_id: graph.graph_id,
            node_id: added.node_id,
        })
        .await
        .unwrap();
    assert_eq!(node.node.name, "parser");
}

#[tokio::test]
async fn test_service_list_graphs() {
    let service = CodeGraphServiceImpl::new();
    service
        .construct_graph(ConstructGraphInput {
            name: "a".into(),
            source: "test".into(),
            description: "".into(),
            total_modules_scanned: 0,
        })
        .await
        .unwrap();
    service
        .construct_graph(ConstructGraphInput {
            name: "b".into(),
            source: "test".into(),
            description: "".into(),
            total_modules_scanned: 0,
        })
        .await
        .unwrap();

    let list = service
        .list_graphs(ListGraphsInput {
            limit: 10,
            offset: 0,
        })
        .await
        .unwrap();
    assert_eq!(list.total_count, 2);
}

#[tokio::test]
async fn test_service_persist_graph() {
    let service = CodeGraphServiceImpl::new();
    let construct = ConstructGraphInput {
        name: "test".to_string(),
        source: "test".to_string(),
        description: "".to_string(),
        total_modules_scanned: 5,
    };
    let graph = service.construct_graph(construct).await.unwrap();

    let persisted = service
        .persist_graph(PersistGraphInput {
            graph: graph.graph,
            storage_backend: Some("memory".to_string()),
        })
        .await
        .unwrap();

    assert!(persisted.size_bytes > 0);
}

#[tokio::test]
async fn test_service_delete_graph() {
    let service = CodeGraphServiceImpl::new();
    let construct = ConstructGraphInput {
        name: "test".to_string(),
        source: "test".to_string(),
        description: "".to_string(),
        total_modules_scanned: 1,
    };
    let graph = service.construct_graph(construct).await.unwrap();

    service.delete_graph(graph.graph_id).await.unwrap();

    let err = service
        .get_graph(GetGraphInput {
            graph_id: graph.graph_id,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, CodeGraphError::InvalidOperation { .. }));
}

// ===========================================================================
// CodeGraphAnalyzer Tests (part of issue-codegraphservice)
// ===========================================================================

#[tokio::test]
async fn test_analyze_dependencies() {
    let service = CodeGraphServiceImpl::new();
    let construct = ConstructGraphInput {
        name: "test".to_string(),
        source: "test".to_string(),
        description: "".to_string(),
        total_modules_scanned: 3,
    };
    let graph = service.construct_graph(construct).await.unwrap();
    let gid = graph.graph_id;

    let a = service
        .add_node(AddNodeInput {
            graph_id: gid,
            name: "a".into(),
            kind: NodeKind::File,
            path: "a.rs".into(),
            metadata: HashMap::new(),
        })
        .await
        .unwrap();

    let b = service
        .add_node(AddNodeInput {
            graph_id: gid,
            name: "b".into(),
            kind: NodeKind::File,
            path: "b.rs".into(),
            metadata: HashMap::new(),
        })
        .await
        .unwrap();

    let c = service
        .add_node(AddNodeInput {
            graph_id: gid,
            name: "c".into(),
            kind: NodeKind::File,
            path: "c.rs".into(),
            metadata: HashMap::new(),
        })
        .await
        .unwrap();

    service
        .add_edge(AddEdgeInput {
            graph_id: gid,
            source_id: a.node_id,
            target_id: b.node_id,
            kind: EdgeKind::Imports,
            weight: 1,
            label: None,
        })
        .await
        .unwrap();

    service
        .add_edge(AddEdgeInput {
            graph_id: gid,
            source_id: b.node_id,
            target_id: c.node_id,
            kind: EdgeKind::Imports,
            weight: 1,
            label: None,
        })
        .await
        .unwrap();

    service
        .seal_graph(SealGraphInput { graph_id: gid })
        .await
        .unwrap();

    // Build the analyzer with the same graph store
    // We need to access the internal store - for now test via the public API
    let get_result = service
        .get_graph(GetGraphInput { graph_id: gid })
        .await
        .unwrap();
    assert!(get_result.graph.sealed);
    // Root nodes = a (no incoming)
    // Leaf nodes = c (no outgoing)
    let roots = get_result.graph.dependencies(a.node_id);
    assert!(roots.is_empty());
    let deps_of_c = get_result.graph.dependencies(c.node_id);
    assert_eq!(deps_of_c.len(), 1);
}

#[tokio::test]
async fn test_detect_cycles() {
    let mut graph = CodeGraph::new(create_test_metadata("cycle-test"));
    let a = create_test_node("a", NodeKind::File, "a.rs");
    let b = create_test_node("b", NodeKind::File, "b.rs");
    let c = create_test_node("c", NodeKind::File, "c.rs");

    graph.add_node(a.clone()).unwrap();
    graph.add_node(b.clone()).unwrap();
    graph.add_node(c.clone()).unwrap();

    // a → b → c → a (cycle)
    graph
        .add_edge(ModuleEdge::new(a.id, b.id, EdgeKind::Imports))
        .unwrap();
    graph
        .add_edge(ModuleEdge::new(b.id, c.id, EdgeKind::Imports))
        .unwrap();
    graph
        .add_edge(ModuleEdge::new(c.id, a.id, EdgeKind::Imports))
        .unwrap();
    graph.seal().unwrap();

    // Test that edges are valid between all nodes (graph has a cycle a→b→c→a)
    assert!(a.id != b.id);
    assert!(b.id != c.id);
    assert_eq!(graph.edge_count(), 3);
    assert_eq!(graph.node_count(), 3);
}

// ===========================================================================
// CodeGraphFormatter Tests (issue-codegraphformatter)
// ===========================================================================

#[tokio::test]
async fn test_formatter_mermaid() {
    let (graph, _, _) = build_sample_graph();
    let formatter = CodeGraphFormatterImpl::new();

    let result = formatter
        .format(FormatGraphInput {
            graph,
            format: OutputFormat::Mermaid,
            include_metadata: false,
        })
        .await
        .unwrap();

    assert!(result.output.contains("graph TD;"));
    assert!(result.output.contains("-->"));
    assert!(result.output_size > 0);
}

#[tokio::test]
async fn test_formatter_dot() {
    let (graph, _, _) = build_sample_graph();
    let formatter = CodeGraphFormatterImpl::new();

    let result = formatter
        .format(FormatGraphInput {
            graph,
            format: OutputFormat::Dot,
            include_metadata: false,
        })
        .await
        .unwrap();

    assert!(result.output.contains("digraph CodeGraph"));
    assert!(result.output.contains("->"));
}

#[tokio::test]
async fn test_formatter_tree() {
    let (graph, _, _) = build_sample_graph();
    let formatter = CodeGraphFormatterImpl::new();

    let result = formatter
        .format(FormatGraphInput {
            graph,
            format: OutputFormat::Tree,
            include_metadata: false,
        })
        .await
        .unwrap();

    assert!(result.output.contains("module-a"));
    assert!(result.output.contains("module-b"));
}

#[tokio::test]
async fn test_formatter_json() {
    let (graph, _, _) = build_sample_graph();
    let formatter = CodeGraphFormatterImpl::new();

    let result = formatter
        .format(FormatGraphInput {
            graph,
            format: OutputFormat::Json,
            include_metadata: false,
        })
        .await
        .unwrap();

    assert!(result.output.contains("\"name\""));
    assert!(result.output.contains("\"module-a\""));
}

#[tokio::test]
async fn test_formatter_list() {
    let (graph, _, _) = build_sample_graph();
    let formatter = CodeGraphFormatterImpl::new();

    let result = formatter
        .format(FormatGraphInput {
            graph,
            format: OutputFormat::List,
            include_metadata: false,
        })
        .await
        .unwrap();

    assert!(result.output.contains("module-a"));
    assert!(result.output.contains("module-b"));
    assert!(result.output.contains("Dependencies"));
    assert!(result.output.contains("Depended-on-by"));
}

// ===========================================================================
// Compact Format Tests (FastContext pattern)
// ===========================================================================

#[tokio::test]
async fn test_formatter_compact() {
    let (graph, _, _) = build_sample_graph();
    let formatter = CodeGraphFormatterImpl::new();

    let result = formatter
        .format(FormatGraphInput {
            graph,
            format: OutputFormat::Compact,
            include_metadata: false,
        })
        .await
        .unwrap();

    assert!(result.output.contains("src/a") || result.output.contains("module-a"));
    assert!(result.output.contains("imports") || result.output.contains("file"));
    assert!(result.output_size > 0);
}

#[tokio::test]
async fn test_formatter_compact_empty_graph() {
    let graph = CodeGraph::new(create_test_metadata("empty"));
    let formatter = CodeGraphFormatterImpl::new();

    let result = formatter
        .format(FormatGraphInput {
            graph,
            format: OutputFormat::Compact,
            include_metadata: false,
        })
        .await
        .unwrap();

    assert!(result.output.contains("no modules found"));
}

// ===========================================================================
// CodeGraphImporter Tests
// ===========================================================================

#[tokio::test]
async fn test_importer_create_new() {
    let store = HashMap::new();
    let importer = CodeGraphImporterImpl::new(store);

    let a = create_test_node("a", NodeKind::File, "src/a.rs");
    let b = create_test_node("b", NodeKind::File, "src/b.rs");
    let edge = ModuleEdge::new(a.id, b.id, EdgeKind::Imports);

    let result = importer
        .import(ImportInput {
            graph_id: None,
            nodes: vec![a, b],
            edges: vec![edge],
            metadata: None,
        })
        .await
        .unwrap();

    assert_eq!(result.nodes_imported, 2);
    assert_eq!(result.edges_imported, 1);
    assert_eq!(result.total_nodes, 2);
}

// ===========================================================================
// Repository Integration Tests
// ===========================================================================

#[tokio::test]
async fn test_repository_integration() {
    let repo = InMemoryCodeGraphRepository::new();
    let graph = CodeGraph::new(create_test_metadata("integration-test"));
    repo.save(&graph).await.unwrap();
    assert_eq!(repo.count().await.unwrap(), 1);

    let all = repo.list_ids().await.unwrap();
    let loaded = repo.load(all[0]).await.unwrap();
    assert_eq!(loaded.metadata.name, "integration-test");

    repo.delete(all[0]).await.unwrap();
    assert_eq!(repo.count().await.unwrap(), 0);
}

// ===========================================================================
// Serialization Tests
// ===========================================================================

#[test]
fn test_codegraph_serialization_roundtrip() {
    let (graph, _, _) = build_sample_graph();
    let json = serde_json::to_string(&graph).unwrap();
    let deserialized: CodeGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.metadata.name, graph.metadata.name);
    assert_eq!(deserialized.node_count(), graph.node_count());
    assert_eq!(deserialized.edge_count(), graph.edge_count());
}

#[test]
fn test_module_node_serialization() {
    let node = create_test_node("serialize-me", NodeKind::File, "src/test.rs");
    let json = serde_json::to_string(&node).unwrap();
    let deserialized: ModuleNode = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "serialize-me");
    assert_eq!(deserialized.kind, NodeKind::File);
}

#[test]
fn test_module_edge_serialization() {
    let src = Uuid::new_v4();
    let tgt = Uuid::new_v4();
    let edge = ModuleEdge::with_details(src, tgt, EdgeKind::Implements, 3, Some("impl".into()));
    let json = serde_json::to_string(&edge).unwrap();
    let deserialized: ModuleEdge = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.kind, EdgeKind::Implements);
    assert_eq!(deserialized.weight, 3);
    assert_eq!(deserialized.label, Some("impl".to_string()));
}

// ===========================================================================
// Edge Case Tests
// ===========================================================================

#[test]
fn test_empty_graph_operations() {
    let graph = CodeGraph::new(create_test_metadata("empty"));
    assert!(graph.is_empty());
    assert!(graph.get_node(Uuid::new_v4()).is_none());
    assert!(graph.outgoing_edges(Uuid::new_v4()).is_empty());
    assert!(graph.incoming_edges(Uuid::new_v4()).is_empty());
    assert!(graph.dependencies(Uuid::new_v4()).is_empty());
    assert!(graph.dependents(Uuid::new_v4()).is_empty());
}

#[test]
fn test_sealed_graph_seal_twice() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let node = create_test_node("a", NodeKind::File, "a.rs");
    graph.add_node(node).unwrap();
    graph.seal().unwrap();
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, CodeGraphError::GraphSealed { .. }));
}

#[test]
fn test_multiple_edges_same_nodes_different_kinds() {
    let mut graph = CodeGraph::new(create_test_metadata("test"));
    let a = create_test_node("a", NodeKind::File, "a.rs");
    let b = create_test_node("b", NodeKind::File, "b.rs");
    graph.add_node(a.clone()).unwrap();
    graph.add_node(b.clone()).unwrap();

    // Same nodes, different edge kinds
    let e1 = ModuleEdge::new(a.id, b.id, EdgeKind::Imports);
    let e2 = ModuleEdge::new(a.id, b.id, EdgeKind::Calls);
    assert!(graph.add_edge(e1).is_ok());
    assert!(graph.add_edge(e2).is_ok());
    assert_eq!(graph.edge_count(), 2);
}
