//! dag_engine tests (split by concern, #917).

use super::*;

#[test]
fn test_new_graph_is_empty() {
    let graph = TaskGraph::new();
    assert!(graph.is_empty());
    assert_eq!(graph.node_count(), 0);
    assert!(!graph.sealed);
    assert!(graph.topological_order().is_none());
}

#[test]
fn test_seal_successful_with_single_node() {
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(make_node(Uuid::new_v4(), "solo", vec![]))
        .unwrap();
    assert!(graph.seal().is_ok());
    assert!(graph.sealed);
    assert!(graph.topological_order().is_some());
    assert_eq!(graph.topological_order().unwrap().len(), 1);
}

#[test]
fn test_cycle_detected_self_loop() {
    let mut graph = TaskGraph::new();
    let id = Uuid::new_v4();
    let node = TaskNode::new(id, "self", "tool", vec![id], "self-loop");
    graph.add_unchecked(node).unwrap();
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, DagError::CycleDetected { found, total } if found < total));
}

#[test]
fn test_cycle_detected_two_node_cycle() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let node_a = TaskNode::new(a, "a", "tool", vec![b], "depends on b");
    let node_b = TaskNode::new(b, "b", "tool", vec![a], "depends on a");
    graph.add_unchecked(node_a).unwrap();
    graph.add_unchecked(node_b).unwrap();
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, DagError::CycleDetected { .. }));
}

#[test]
fn test_cycle_detected_three_node_circular() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    graph
        .add_unchecked(TaskNode::new(a, "a", "tool", vec![c], "depends on c"))
        .unwrap();
    graph
        .add_unchecked(TaskNode::new(b, "b", "tool", vec![a], "depends on a"))
        .unwrap();
    graph
        .add_unchecked(TaskNode::new(c, "c", "tool", vec![b], "depends on b"))
        .unwrap();
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, DagError::CycleDetected { found, total } if found < total));
    assert!(err.to_string().contains("Cycle detected"));
}

// ---------------------------------------------------------------------------
// Topological Sort
// ---------------------------------------------------------------------------

#[test]
fn test_topological_sort_linear_chain() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    graph
        .add_unchecked(make_node(a, "compile", vec![]))
        .unwrap();
    graph.add_unchecked(make_node(b, "test", vec![a])).unwrap();
    graph
        .add_unchecked(make_node(c, "deploy", vec![b]))
        .unwrap();
    graph.seal().unwrap();
    let order = graph.topological_order().unwrap().to_vec();
    // a must come before b, b before c
    let pos_a = order.iter().position(|id| *id == a).unwrap();
    let pos_b = order.iter().position(|id| *id == b).unwrap();
    let pos_c = order.iter().position(|id| *id == c).unwrap();
    assert!(pos_a < pos_b, "compile must come before test");
    assert!(pos_b < pos_c, "test must come before deploy");
}

#[test]
fn test_topological_sort_diamond() {
    let mut graph = TaskGraph::new();
    let root = Uuid::new_v4();
    let left = Uuid::new_v4();
    let right = Uuid::new_v4();
    let merge = Uuid::new_v4();
    graph
        .add_unchecked(make_node(root, "root", vec![]))
        .unwrap();
    graph
        .add_unchecked(make_node(left, "left", vec![root]))
        .unwrap();
    graph
        .add_unchecked(make_node(right, "right", vec![root]))
        .unwrap();
    graph
        .add_unchecked(make_node(merge, "merge", vec![left, right]))
        .unwrap();
    graph.seal().unwrap();
    let order = graph.topological_order().unwrap();
    let pos_root = order.iter().position(|id| *id == root).unwrap();
    let pos_left = order.iter().position(|id| *id == left).unwrap();
    let pos_right = order.iter().position(|id| *id == right).unwrap();
    let pos_merge = order.iter().position(|id| *id == merge).unwrap();
    assert!(pos_root < pos_left);
    assert!(pos_root < pos_right);
    assert!(pos_left < pos_merge);
    assert!(pos_right < pos_merge);
}

#[test]
fn test_topological_sort_independent_nodes() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    graph.add_unchecked(make_node(a, "a", vec![])).unwrap();
    graph.add_unchecked(make_node(b, "b", vec![])).unwrap();
    graph.add_unchecked(make_node(c, "c", vec![])).unwrap();
    graph.seal().unwrap();
    // All should be in the order (any order is valid since they're independent)
    assert_eq!(graph.topological_order().unwrap().len(), 3);
}

#[test]
fn test_topological_sort_all_nodes_present() {
    let mut graph = TaskGraph::new();
    let nodes: Vec<_> = (0..10).map(|_| Uuid::new_v4()).collect();
    for (i, &id) in nodes.iter().enumerate() {
        let deps = if i > 0 { vec![nodes[i - 1]] } else { vec![] };
        graph
            .add_unchecked(make_node(id, &format!("node-{}", i), deps))
            .unwrap();
    }
    graph.seal().unwrap();
    assert_eq!(graph.topological_order().unwrap().len(), 10);
}

// ---------------------------------------------------------------------------
// Ready Queue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_service_cycle_detection() {
    let service = DagGraphServiceImpl::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let construct = service
        .construct_graph(ConstructGraphInput {
            nodes: vec![
                TaskNode::new(a, "a", "tool", vec![b], "depends on b"),
                TaskNode::new(b, "b", "tool", vec![a], "depends on a"),
            ],
        })
        .await
        .unwrap();
    let dag_id = construct.dag_id;

    let err = service
        .seal_graph(SealGraphInput { dag_id })
        .await
        .unwrap_err();
    assert!(matches!(err, DagError::CycleDetected { .. }));
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------
