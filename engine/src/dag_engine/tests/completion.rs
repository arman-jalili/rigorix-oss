//! dag_engine tests (split by concern, #917).

use super::*;

#[test]
fn test_mark_completed_unsealed_graph_fails() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    graph.add_unchecked(make_node(a, "a", vec![])).unwrap();
    let err = graph.mark_completed(a).unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));
}

#[test]
fn test_mark_completed_nonexistent_node_fails() {
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(make_node(Uuid::new_v4(), "a", vec![]))
        .unwrap();
    graph.seal().unwrap();
    let err = graph.mark_completed(Uuid::new_v4()).unwrap_err();
    assert!(matches!(err, DagError::TaskNotFound { .. }));
}

// ---------------------------------------------------------------------------
// Execution Completion
// ---------------------------------------------------------------------------

#[test]
fn test_execution_complete_all_nodes_done() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    graph.add_unchecked(make_node(a, "a", vec![])).unwrap();
    graph.add_unchecked(make_node(b, "b", vec![a])).unwrap();
    graph.seal().unwrap();

    assert!(!graph.is_execution_complete());
    graph.mark_completed(a).unwrap();
    assert!(!graph.is_execution_complete());
    graph.mark_completed(b).unwrap();
    assert!(graph.is_execution_complete());
}

// ---------------------------------------------------------------------------
// TaskNode
// ---------------------------------------------------------------------------
