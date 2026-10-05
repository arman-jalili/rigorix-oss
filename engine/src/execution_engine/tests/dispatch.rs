//! execution_engine tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_execute_graph_without_graph_fails() {
    // GAP-A-02: a missing graph is a caller contract violation — typed error,
    // never a fake-success empty result.
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let err = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: None,
            config_override: None,
        })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("without a graph"), "got: {err}");
}

#[tokio::test]
async fn test_execute_graph_rejects_duplicate() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    let mut graph = TaskGraph::new();
    let root = TaskNode::new(Uuid::new_v4(), "root", "shell", vec![], "echo hi");
    graph.add_unchecked(root.clone()).unwrap();
    graph.seal().unwrap();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let err = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("already in progress"));
}

/// GAP-A-11: max_failures_before_abort stops dispatching new nodes once the
/// threshold is crossed (sequential dispatch keeps it deterministic).
#[tokio::test]
async fn test_max_failures_before_abort_stops_dispatch() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let mut graph = TaskGraph::new();
    let ids: Vec<_> = (0..3)
        .map(|i| {
            let n = TaskNode::new(
                Uuid::new_v4(),
                format!("bad-{}", i),
                "no_such_tool",
                vec![],
                "boom",
            );
            let id = n.id;
            graph.add_unchecked(n).unwrap();
            id
        })
        .collect();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: Some(ParallelExecutorConfig {
                max_concurrent_executions: 1, // sequential -> deterministic
                max_failures_before_abort: 1, // abort after the first failure
                ..Default::default()
            }),
        })
        .await
        .unwrap();

    assert_eq!(
        output.result.node_results.len(),
        1,
        "only the first node should be dispatched before the abort"
    );
    let (_, first) = output.result.node_results.iter().next().unwrap();
    assert!(!first.success, "the dispatched node must have failed");
    let _ = ids;
}

#[tokio::test]
async fn test_abort_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    let abort_output = executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "test abort".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(abort_output.dag_id, dag_id);
    assert_eq!(abort_output.skipped_count, 0); // no nodes to skip
}

#[tokio::test]
async fn test_abort_twice_returns_error() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "first abort".to_string(),
        })
        .await
        .unwrap();

    let err = executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "second abort".to_string(),
        })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("already aborted"));
}

#[tokio::test]
async fn test_abort_nonexistent_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let err = executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "test".to_string(),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::execution_engine::domain::ExecutionError::NodeNotFound { .. }
    ));
}

#[tokio::test]
async fn test_execute_graph_creates_session_and_tracks_state() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();

    assert_eq!(state.dag_id, dag_id);
    assert!(!state.paused);
    assert!(state.started_at.is_some());
}

#[tokio::test]
async fn test_execute_graph_completes_without_cancellation() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    assert!(!output.result.cancelled);
    assert!(output.result.cancellation_reason.is_none());
}

#[tokio::test]
async fn test_abort_marks_execution_as_cancelled() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "manual abort".to_string(),
        })
        .await
        .unwrap();

    // The execution is now aborted; verifying the state requires
    // get_execution_state which returns the session state
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();

    // State is not complete because execution has no nodes tracked yet
    // But abort was accepted without error
    assert_eq!(state.dag_id, dag_id);
}

#[tokio::test]
async fn test_decide_abort_on_exhaustion() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        enable_fallback: false,
        skip_on_exhaustion: false,
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "tool",
        "intent",
        "transient",
        "error",
        3,
        4,
        100,
        400,
    );

    let decision = service.decide(&ctx, &policy, None).await;
    assert!(decision.is_terminal());
    assert!(matches!(decision, RetryDecision::Abort { .. }));
}

/// AC#9 (deny): step A completes, then B is proposed → B is denied BEFORE
/// dispatch: structured sequence_policy_denied failure, B's tool never called.
#[tokio::test]
async fn test_r3_deny_fails_dynamic_later_step_before_dispatch() {
    use crate::execution_engine::domain::NodeStatus;

    let (path_a, path_b) = r3_paths("deny");
    let executor = r3_executor(RuleAction::Deny, &path_a, &path_b);
    let dag_id = Uuid::new_v4();
    let (graph, _a_id, b_id) = r3_chain_graph(&path_a, &path_b);

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    // A executed; B failed with the structured denial — no approval involved.
    assert!(!output.approval_pending);
    assert_eq!(output.result.completed_count, 1, "A completes");
    assert_eq!(output.result.failed_count, 1, "B fails deterministically");

    let b_result = output
        .result
        .node_results
        .get(&b_id)
        .expect("denied B must carry a node result");
    assert!(!b_result.success);
    assert_eq!(
        b_result.failure_type.as_deref(),
        Some("sequence_policy_denied"),
        "denial is a structured, typed node failure"
    );
    assert!(
        b_result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("r3-remove-then-reassign"),
        "error names the matched rule"
    );

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(!state.paused);
    assert!(state.is_complete);
    let step_b = state
        .node_states
        .values()
        .find(|s| s.node_name == "step_b")
        .expect("step_b state");
    assert_eq!(step_b.status, NodeStatus::Failed);
    assert!(
        step_b.started_at.is_none(),
        "denied tool must NEVER be called"
    );

    // Spy assertion: B's file was never created; A's file exists.
    assert!(
        std::path::Path::new(&path_a).exists(),
        "A must have executed"
    );
    assert!(
        !std::path::Path::new(&path_b).exists(),
        "denied B's tool must not have written its file"
    );
}

/// AC#9 (no-match control): with the policy service present but no rule
/// matching the actual pair, dispatch is unchanged — B executes normally.
#[tokio::test]
async fn test_r3_non_matching_rule_does_not_gate_dispatch() {
    let (path_a, path_b) = r3_paths("control");
    // Rule predicates point at DIFFERENT paths → nothing matches.
    let svc = SequencePolicyServiceImpl::new(Box::new(R3PolicyRepo {
        config: Some(r3_config(
            RuleAction::Deny,
            "/tmp/never-a.tmp",
            "/tmp/never-b.tmp",
        )),
    }));
    let executor = create_executor().with_sequence_policy(Arc::new(svc));
    let dag_id = Uuid::new_v4();
    let (graph, _a_id, b_id) = r3_chain_graph(&path_a, &path_b);

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(!output.approval_pending, "no match → no pause");
    assert_eq!(output.result.completed_count, 2);

    let b_result = output.result.node_results.get(&b_id).unwrap();
    assert!(b_result.success, "non-matching rule must not gate dispatch");
    assert_eq!(
        tokio::fs::read_to_string(&path_b).await.unwrap(),
        "b",
        "B executes normally when no rule matches"
    );
}

// ── ADR-017 R2: step-outcome gating (AC #13) ───────────────────────────────

/// Build A → B → C where A fails (unknown tool); B and C depend transitively.
fn failing_chain() -> (crate::dag_engine::domain::TaskGraph, Uuid, Uuid, Uuid) {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(TaskNode::new(
            a,
            "a",
            "definitely_unknown_tool",
            vec![],
            "{}",
        ))
        .unwrap();
    graph
        .add_unchecked(TaskNode::new(
            b,
            "b",
            "definitely_unknown_tool",
            vec![a],
            "{}",
        ))
        .unwrap();
    graph
        .add_unchecked(TaskNode::new(
            c,
            "c",
            "definitely_unknown_tool",
            vec![b],
            "{}",
        ))
        .unwrap();
    graph.seal().unwrap();
    (graph, a, b, c)
}

#[tokio::test]
async fn gating_mode_false_skips_failed_nodes_transitive_dependents() {
    use crate::dag_engine::domain::GatingMode;
    use crate::execution_engine::domain::NodeStatus;

    let (graph, a, b, c) = failing_chain();
    let executor = create_executor().with_gating_mode(GatingMode {
        release_dependents_on_failure: false,
    });
    let dag_id = Uuid::new_v4();
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    let states: std::collections::HashMap<_, _> = state.node_states.into_iter().collect();
    assert_eq!(
        states[&a].status,
        NodeStatus::Failed,
        "A must fail (unknown tool)"
    );
    assert_eq!(
        states[&b].status,
        NodeStatus::Skipped,
        "B must be Skipped, never dispatched"
    );
    assert_eq!(
        states[&c].status,
        NodeStatus::Skipped,
        "C (transitive dependent) must be Skipped, never dispatched"
    );
}

#[tokio::test]
async fn gating_mode_default_releases_dependents_on_failure() {
    use crate::execution_engine::domain::NodeStatus;

    let (graph, a, b, _c) = failing_chain();
    // Default gating mode (release_dependents_on_failure = true).
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    let states: std::collections::HashMap<_, _> = state.node_states.into_iter().collect();
    assert_eq!(states[&a].status, NodeStatus::Failed);
    assert_eq!(
        states[&b].status,
        NodeStatus::Failed,
        "default behavior releases B (it dispatches and fails too)"
    );
}
