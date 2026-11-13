//! The graph builder translates a resolved dependency set into `Uuid` edges (#1526).
//!
//! `step_dependencies` decides *which* edges exist; this module pins the translation, because that
//! is where the frozen sequential contract lives and where a name becomes an id. It is an in-crate
//! test for a mechanical reason: `build_graph_from_steps` is `pub(super)`, so the integration-level
//! cases next to it could not call it — and the existing chaining test lives here too
//! (`tests/plan.rs`), which is where the contract it protects is checked.

use super::*;
use crate::orchestrator::application::dto::TemplateStepDef;

fn step(name: &str, depends_on: &[&str]) -> TemplateStepDef {
    TemplateStepDef {
        name: name.into(),
        tool: "bash".into(),
        description: name.into(),
        parameters: serde_json::json!({ "step": name }),
        requires_approval: false,
        require_identity: false,
        timeout_secs: None,
        evaluate_score: false,
        depends_on: depends_on.iter().map(|d| (*d).to_string()).collect(),
    }
}

/// The graph's edge set, with ids replaced by position so two runs are comparable: the ids are
/// `Uuid::new_v4()`, and a test that compared them would be comparing randomness.
fn edges_by_position(graph: &crate::dag_engine::domain::TaskGraph) -> Vec<(String, Vec<usize>)> {
    let ids: Vec<uuid::Uuid> = graph.nodes().map(|node| node.id).collect();
    graph
        .nodes()
        .map(|node| {
            let deps: Vec<usize> = node
                .dependencies
                .iter()
                .map(|id| {
                    ids.iter()
                        .position(|candidate| candidate == id)
                        .expect("a dependency is a node in this graph")
                })
                .collect();
            (node.name.clone(), deps)
        })
        .collect()
}

#[test]
fn test_build_graph_from_steps_edges_equal_the_sequential_chain() {
    // Criterion: a plan with no `depends_on` produces the same edge set as the base commit — a
    // chain. Asserted by position, and the expectation is owned here rather than read from the
    // implementation.
    let orch = OrchestratorServiceImpl::default_test();
    let steps = [step("a", &[]), step("b", &[]), step("c", &[])];
    let graph = orch.build_graph_from_steps(&steps).unwrap();
    assert_eq!(
        edges_by_position(&graph),
        vec![
            ("a".to_string(), vec![]),
            ("b".to_string(), vec![0]),
            ("c".to_string(), vec![1]),
        ]
    );
}

#[test]
fn test_build_graph_from_steps_diamond_has_no_edge_between_the_branches() {
    // a; b←a; c←a; d←b,c. The absence of a b→c or c→b edge is the whole feature: it is what makes
    // the frontier representable, and a chain would add one.
    let orch = OrchestratorServiceImpl::default_test();
    let steps = [
        step("a", &[]),
        step("b", &["a"]),
        step("c", &["a"]),
        step("d", &["b", "c"]),
    ];
    let graph = orch.build_graph_from_steps(&steps).unwrap();
    assert_eq!(
        edges_by_position(&graph),
        vec![
            ("a".to_string(), vec![]),
            ("b".to_string(), vec![0]),
            ("c".to_string(), vec![0]),
            ("d".to_string(), vec![1, 2]),
        ]
    );
}

#[test]
fn test_build_graph_from_steps_refuses_an_unknown_dependency_and_names_it() {
    let orch = OrchestratorServiceImpl::default_test();
    let steps = [step("a", &[]), step("b", &["nope"])];
    let err = orch.build_graph_from_steps(&steps).unwrap_err();
    match err {
        OrchestratorError::InvalidStepDependencies { kind, detail } => {
            assert_eq!(kind, "unknown_dependency");
            assert!(detail.contains("'b'"), "{detail}");
            assert!(detail.contains("'nope'"), "{detail}");
        }
        other => panic!("expected InvalidStepDependencies, got {other:?}"),
    }
}

#[test]
fn test_build_graph_from_steps_refuses_a_cycle_and_names_the_steps() {
    let orch = OrchestratorServiceImpl::default_test();
    let steps = [step("a", &["b"]), step("b", &["a"])];
    let err = orch.build_graph_from_steps(&steps).unwrap_err();
    match err {
        OrchestratorError::InvalidStepDependencies { kind, detail } => {
            assert_eq!(kind, "cycle");
            assert!(detail.contains('a') && detail.contains('b'), "{detail}");
        }
        other => panic!("expected InvalidStepDependencies, got {other:?}"),
    }
}

#[test]
fn test_build_graph_from_steps_refuses_a_self_edge() {
    let orch = OrchestratorServiceImpl::default_test();
    let err = orch
        .build_graph_from_steps(&[step("a", &["a"])])
        .unwrap_err();
    assert!(matches!(
        err,
        OrchestratorError::InvalidStepDependencies {
            kind: "self_edge",
            ..
        }
    ));
}

#[test]
fn test_build_graph_from_steps_refuses_a_duplicate_name() {
    let orch = OrchestratorServiceImpl::default_test();
    let err = orch
        .build_graph_from_steps(&[step("a", &[]), step("a", &[])])
        .unwrap_err();
    assert!(matches!(
        err,
        OrchestratorError::InvalidStepDependencies {
            kind: "duplicate_step_name",
            ..
        }
    ));
}

#[test]
fn test_build_graph_carries_requires_approval_onto_the_join_step() {
    // A pause on a join step is what the frontier buys: `d` waits for both branches *and* for a
    // human. The node carries the flag; the executor's pause is checked where it lives.
    let orch = OrchestratorServiceImpl::default_test();
    let mut join = step("d", &["b", "c"]);
    join.requires_approval = true;
    let steps = [step("a", &[]), step("b", &["a"]), step("c", &["a"]), join];
    let graph = orch.build_graph_from_steps(&steps).unwrap();
    let flag = graph
        .nodes()
        .find(|n| n.name == "d")
        .map(|n| n.requires_approval);
    assert_eq!(flag, Some(true));
    // …and it has two dependencies, so the pause happens after both branches, not after one.
    let d = graph.nodes().find(|n| n.name == "d").unwrap();
    assert_eq!(d.dependencies.len(), 2);
}
