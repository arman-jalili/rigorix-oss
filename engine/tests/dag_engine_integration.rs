//! Integration test: DAG engine lifecycle.
//!
//! Verifies the TaskGraph can be constructed, sealed, and executed
//! through the full node lifecycle. Exercises O(1) ready queue,
//! topological sort, cycle detection, and completion tracking.

use std::collections::HashMap;
use uuid::Uuid;

use rigorix_engine::dag_engine::domain::{
    DagError, ExecutionPolicy, TaskGraph, TaskNode, ValidationRule,
};

/// Helper: create a simple node with no dependencies.
fn node(name: &str, tool: &str) -> TaskNode {
    TaskNode::new(Uuid::new_v4(), name, tool, vec![], format!("Run {tool}"))
}

/// Helper: create a node that depends on other nodes.
fn node_with_deps(name: &str, tool: &str, deps: Vec<Uuid>) -> TaskNode {
    TaskNode::new(
        Uuid::new_v4(),
        name,
        tool,
        deps,
        format!("Run {tool} after deps"),
    )
}

// ---------------------------------------------------------------------------
// Construction and sealing
// ---------------------------------------------------------------------------

#[test]
fn test_empty_graph_seal_fails() {
    let mut graph = TaskGraph::new();
    let result = graph.seal();
    assert!(matches!(result, Err(DagError::InvalidGraph { .. })));
}

#[test]
fn test_single_node_seal_succeeds() {
    let mut graph = TaskGraph::new();
    let n = node("build", "cargo build");
    graph.add_unchecked(n).unwrap();
    graph.seal().unwrap();
    assert!(graph.sealed);
    assert_eq!(graph.node_count(), 1);
    assert!(graph.topological_order().is_some());
}

// ---------------------------------------------------------------------------
// Topological sort
// ---------------------------------------------------------------------------

#[test]
fn test_linear_chain_topological_order() {
    let mut graph = TaskGraph::new();
    let a = node("a", "echo a");
    let a_id = a.id;
    let b = node_with_deps("b", "echo b", vec![a_id]);
    let b_id = b.id;
    let c = node_with_deps("c", "echo c", vec![b_id]);
    let c_id = c.id;

    graph.add_unchecked(a).unwrap();
    graph.add_unchecked(b).unwrap();
    graph.add_unchecked(c).unwrap();
    graph.seal().unwrap();

    let order = graph.topological_order().unwrap();
    assert_eq!(order.len(), 3);
    // a (no deps) must come before b (depends on a) which must come before c
    let pos: HashMap<_, _> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    assert!(pos[&a_id] < pos[&b_id]);
    assert!(pos[&b_id] < pos[&c_id]);
}

#[test]
fn test_diamond_dependency_topological_order() {
    let mut graph = TaskGraph::new();
    let root = node("root", "setup");
    let root_id = root.id;
    let left = node_with_deps("left", "build-lib", vec![root_id]);
    let left_id = left.id;
    let right = node_with_deps("right", "build-bin", vec![root_id]);
    let right_id = right.id;
    let merge = node_with_deps("merge", "link", vec![left_id, right_id]);
    let merge_id = merge.id;

    graph.add_unchecked(root).unwrap();
    graph.add_unchecked(left).unwrap();
    graph.add_unchecked(right).unwrap();
    graph.add_unchecked(merge).unwrap();
    graph.seal().unwrap();

    let order = graph.topological_order().unwrap();
    assert_eq!(order.len(), 4);
    let pos: HashMap<_, _> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    assert!(pos[&root_id] < pos[&left_id]);
    assert!(pos[&root_id] < pos[&right_id]);
    assert!(pos[&left_id] < pos[&merge_id]);
    assert!(pos[&right_id] < pos[&merge_id]);
}

// ---------------------------------------------------------------------------
// Cycle detection
// ---------------------------------------------------------------------------

#[test]
fn test_cycle_detection() {
    let mut graph = TaskGraph::new();
    let a = node("a", "echo a");
    let a_id = a.id;
    let b_id = Uuid::new_v4();
    // b depends on a, a depends on b → cycle
    let b = TaskNode::new(b_id, "b", "echo b", vec![a_id], "run b");
    let a_cyclic = TaskNode::new(a_id, "a", "echo a", vec![b_id], "run a");

    graph.add_unchecked(b).unwrap();
    graph.add_unchecked(a_cyclic).unwrap();
    let result = graph.seal();
    assert!(matches!(result, Err(DagError::CycleDetected { .. })));
}

// ---------------------------------------------------------------------------
// Ready queue and completion
// ---------------------------------------------------------------------------

#[test]
fn test_ready_queue_and_completion() {
    let mut graph = TaskGraph::new();
    let a = node("a", "echo a");
    let a_id = a.id;
    let b = node_with_deps("b", "echo b", vec![a_id]);
    let b_id = b.id;

    graph.add_unchecked(a).unwrap();
    graph.add_unchecked(b).unwrap();
    graph.seal().unwrap();

    // a should be ready immediately (no deps), b should not
    let ready = graph.ready_nodes();
    assert!(ready.contains(&a_id));
    assert!(!ready.contains(&b_id));

    // Complete a → b should become ready
    graph.mark_completed(a_id).unwrap();
    let ready = graph.ready_nodes();
    assert!(ready.contains(&b_id));

    // Complete b → execution done
    graph.mark_completed(b_id).unwrap();
    assert!(graph.is_execution_complete());
}

// ---------------------------------------------------------------------------
// O(1) node lookup
// ---------------------------------------------------------------------------

#[test]
fn test_get_node_is_constant_time_indexed() {
    let mut graph = TaskGraph::new();
    let n = node("test", "echo test");
    let id = n.id;
    graph.add_unchecked(n).unwrap();

    // Verify lookup works
    let found = graph.get_node(id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "test");

    // Verify lookup fails for unknown id
    assert!(graph.get_node(Uuid::new_v4()).is_none());
}

// ---------------------------------------------------------------------------
// ExecutionPolicy defaults
// ---------------------------------------------------------------------------

#[test]
fn test_execution_policy_defaults() {
    let policy = ExecutionPolicy::default();
    assert_eq!(policy.max_retries, 3);
    assert_eq!(policy.backoff_ms, 100);
    assert!(policy.fallback_node.is_none());
}

// ---------------------------------------------------------------------------
// ValidationRule
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rule_as_str() {
    assert_eq!(ValidationRule::TypeCheck.as_str(), "type_check");
    assert_eq!(ValidationRule::TestPass.as_str(), "test_pass");
    assert_eq!(ValidationRule::LintPass.as_str(), "lint_pass");
    assert_eq!(ValidationRule::Custom("my-check".into()).as_str(), "custom");
}

// ---------------------------------------------------------------------------
// Serialization roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_graph_serialization_roundtrip() {
    let mut graph = TaskGraph::new();
    let a = node("a", "build");
    let a_id = a.id;
    let b = node_with_deps("b", "test", vec![a_id]);
    graph.add_unchecked(a).unwrap();
    graph.add_unchecked(b).unwrap();

    // Serialize BEFORE sealing (sealed graphs can't be re-sealed)
    let json = serde_json::to_string(&graph).unwrap();
    let mut deserialized: TaskGraph = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.node_count(), 2);
    assert!(!deserialized.sealed);

    // After deserialization, seal rebuilds the index and sorts
    deserialized.seal().unwrap();
    assert!(deserialized.sealed);
    assert!(deserialized.topological_order().is_some());
    // Verify we can still look up nodes via O(1) index
    assert!(deserialized.get_node(a_id).is_some());
}
