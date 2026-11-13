//! A caller-supplied plan may carry a dependency graph (#1526).
//!
//! The shape is not new — `TemplateNode { depends_on }` and `DagNode` already express it. What is
//! new is that a *plan* names its steps, so the names must be resolved to the ids a fresh graph
//! allocates, and the four ways a dependency set can be wrong must be refused instead of becoming
//! a graph the executor cannot order.
//!
//! These cases exercise `orchestrator::domain::step_graph`, which is the whole decision: which
//! edges exist, in which order. The graph builder is a translation of that answer into `Uuid`s,
//! and it is pinned separately, in-crate, where it is reachable.

use rigorix_engine::orchestrator::domain::step_graph::{
    StepDependencyError, resolve_step_dependencies,
};

fn plan(steps: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    steps
        .iter()
        .map(|(name, deps)| {
            (
                (*name).to_string(),
                deps.iter().map(|d| (*d).to_string()).collect(),
            )
        })
        .collect()
}

#[test]
fn orchestrator_step_dependencies_absent_keeps_the_sequential_contract() {
    // No step declares a dependency: each depends on its predecessor, which is the frozen
    // contract every existing caller relies on. If this changed, a template would stop executing
    // as a runbook and start executing as a batch.
    let resolved = resolve_step_dependencies(&plan(&[("a", &[]), ("b", &[]), ("c", &[])])).unwrap();
    assert!(!resolved.explicit);
    assert_eq!(resolved.order, vec![0, 1, 2]);
    assert_eq!(resolved.dependencies, vec![vec![], vec![0], vec![1]]);
}

#[test]
fn orchestrator_step_dependencies_diamond_is_a_frontier_not_a_line() {
    // a; b←a; c←a; d←b,c. The point of the issue: b and c have no edge between them, so they are
    // a frontier the executor may run concurrently.
    let resolved = resolve_step_dependencies(&plan(&[
        ("a", &[]),
        ("b", &["a"]),
        ("c", &["a"]),
        ("d", &["b", "c"]),
    ]))
    .unwrap();
    assert!(resolved.explicit);
    assert_eq!(resolved.dependencies[0], Vec::<usize>::new());
    assert_eq!(resolved.dependencies[1], vec![0]);
    assert_eq!(resolved.dependencies[2], vec![0]);
    assert_eq!(resolved.dependencies[3], vec![1, 2]);
    // Order is stable and respects the edges: a before b before d, a before c before d.
    let position = |i: usize| resolved.order.iter().position(|&x| x == i).unwrap();
    assert!(position(0) < position(1));
    assert!(position(0) < position(2));
    assert!(position(1) < position(3));
    assert!(position(2) < position(3));
}

#[test]
fn orchestrator_step_dependencies_order_follows_list_position_when_ready() {
    // Two independent roots: the tie-break is the caller's list position, so the plan-time gate
    // iterates the same order every run rather than whatever a hash map yields.
    let resolved =
        resolve_step_dependencies(&plan(&[("a", &[]), ("b", &[]), ("c", &["a"])])).unwrap();
    assert_eq!(resolved.order, vec![0, 1, 2]);
}

#[test]
fn orchestrator_step_dependencies_unknown_name_names_the_step_and_the_known_ones() {
    let err = resolve_step_dependencies(&plan(&[("a", &[]), ("b", &["typo"])])).unwrap_err();
    assert!(matches!(err, StepDependencyError::UnknownDependency { .. }));
    let message = err.to_string();
    assert!(message.contains("'b'"), "{message}");
    assert!(message.contains("'typo'"), "{message}");
    assert!(message.contains("known steps: a, b"), "{message}");
}

#[test]
fn orchestrator_step_dependencies_self_edge_is_refused() {
    let err = resolve_step_dependencies(&plan(&[("a", &["a"])])).unwrap_err();
    assert_eq!(err.kind(), "self_edge");
    assert!(err.to_string().contains("'a' depends on itself"));
}

#[test]
fn orchestrator_step_dependencies_duplicate_name_is_refused() {
    // A dependency on a name that appears twice does not identify one node, so the plan cannot be
    // resolved unambiguously — and the refusal says which name, because the caller can only fix
    // it if it knows.
    let err = resolve_step_dependencies(&plan(&[("a", &[]), ("a", &[])])).unwrap_err();
    assert_eq!(err.kind(), "duplicate_step_name");
    assert!(err.to_string().contains("'a'"), "{err}");
}

#[test]
fn orchestrator_step_dependencies_cycle_names_the_steps_that_cannot_start() {
    let err = resolve_step_dependencies(&plan(&[("a", &["c"]), ("b", &["a"]), ("c", &["b"])]))
        .unwrap_err();
    assert_eq!(err.kind(), "cycle");
    let message = err.to_string();
    for step in ["a", "b", "c"] {
        assert!(message.contains(step), "{message}");
    }
}

#[test]
fn orchestrator_step_dependencies_a_step_behind_a_cycle_is_named_too() {
    // `d` depends on nothing cyclic, but it is behind `a`; the message lists what remains, so a
    // reader can see the cycle *and* what it is holding up.
    let err = resolve_step_dependencies(&plan(&[("a", &["b"]), ("b", &["a"]), ("d", &["a"])]))
        .unwrap_err();
    assert_eq!(err.kind(), "cycle");
    let message = err.to_string();
    assert!(
        message.contains("a") && message.contains("b") && message.contains("d"),
        "{message}"
    );
}
