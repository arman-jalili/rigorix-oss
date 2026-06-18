//! Deterministic property-style tests for the DAG engine.
//!
//! Tests invariants across a variety of generated inputs.

#![cfg(test)]

use uuid::Uuid;

use crate::dag_engine::domain::graph::{TaskGraph, TaskNode};

#[test]
fn test_taskgraph_serde_roundtrip_empty() {
    let graph = TaskGraph::new();
    let serialized = serde_json::to_string(&graph).unwrap();
    let deserialized: TaskGraph = serde_json::from_str(&serialized).unwrap();
    assert_eq!(graph.nodes.len(), deserialized.nodes.len());
}

#[test]
fn test_taskgraph_serde_roundtrip_various_sizes() {
    for size in [1, 3, 7, 15] {
        let mut graph = TaskGraph::new();
        for i in 0..size {
            let node = TaskNode::new(
                Uuid::new_v4(),
                format!("node-{}", i),
                "echo",
                vec![],
                "test",
            );
            let _ = graph.add_unchecked(node);
        }
        let serialized = serde_json::to_string(&graph).unwrap();
        let deserialized: TaskGraph = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            graph.nodes.len(),
            deserialized.nodes.len(),
            "Roundtrip should preserve node count for size {}",
            size
        );
    }
}

#[test]
fn test_taskgraph_serialized_size_grows_with_nodes() {
    let sizes: Vec<usize> = (0..10).collect();
    let mut prev_len = 0;
    for &size in &sizes {
        let mut graph = TaskGraph::new();
        for i in 0..size {
            let node = TaskNode::new(Uuid::new_v4(), format!("n{}", i), "cat", vec![], "read");
            let _ = graph.add_unchecked(node);
        }
        let serialized = serde_json::to_string(&graph).unwrap();
        if size > 0 {
            assert!(
                serialized.len() > prev_len,
                "Serialized size should increase with more nodes"
            );
        }
        prev_len = serialized.len();
    }
}
