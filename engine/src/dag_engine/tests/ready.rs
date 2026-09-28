//! dag_engine tests (split by concern, #917).

use super::*;

#[test]
fn test_seal_double_seal_fails() {
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(make_node(Uuid::new_v4(), "a", vec![]))
        .unwrap();
    graph.seal().unwrap();
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));
    let msg = format!("{}", err);
    assert!(
        msg.contains("already sealed"),
        "Expected 'already sealed': {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// Dependency Validation
// ---------------------------------------------------------------------------

#[test]
fn test_ready_nodes_empty_before_seal() {
    let graph = TaskGraph::new();
    assert!(graph.ready_nodes().is_empty());
}

#[test]
fn test_ready_nodes_after_seal() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    graph
        .add_unchecked(make_node(a, "no-deps", vec![]))
        .unwrap();
    graph
        .add_unchecked(make_node(b, "has-dep", vec![a]))
        .unwrap();
    graph.seal().unwrap();
    // a has no deps, so it should be ready
    let ready = graph.ready_nodes();
    assert!(ready.contains(&a), "Node a (no deps) should be ready");
    assert!(
        !ready.contains(&b),
        "Node b (has dep on a) should not be ready yet"
    );
}

#[test]
fn test_mark_completed_updates_ready_queue() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    graph.add_unchecked(make_node(a, "a", vec![])).unwrap();
    graph.add_unchecked(make_node(b, "b", vec![a])).unwrap();
    graph.seal().unwrap();

    // Initially only a is ready
    assert!(graph.ready_nodes().contains(&a));
    assert!(!graph.ready_nodes().contains(&b));

    // Mark a as completed
    graph.mark_completed(a).unwrap();

    // Now b should be ready
    let ready = graph.ready_nodes();
    assert!(
        ready.contains(&b),
        "Node b should be ready after a completes"
    );
}

#[test]
fn test_mark_completed_idempotent() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    graph.add_unchecked(make_node(a, "a", vec![])).unwrap();
    graph.add_unchecked(make_node(b, "b", vec![a])).unwrap();
    graph.seal().unwrap();

    graph.mark_completed(a).unwrap();
    let ready_before = graph.ready_nodes().len();
    graph.mark_completed(a).unwrap(); // Second call, should be no-op
    let ready_after = graph.ready_nodes().len();
    assert_eq!(
        ready_before, ready_after,
        "Marking completed twice should be idempotent"
    );
}
