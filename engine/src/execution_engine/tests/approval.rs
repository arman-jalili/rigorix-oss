//! execution_engine tests (split by concern, #917).

use super::*;

/// H-08 regression: hydrating a session must preserve the persisted
/// `started_at` so a resumed run reports an undistorted duration.
#[tokio::test]
async fn test_hydrate_preserves_persisted_started_at() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::execution_engine::application::dto::HydrateExecutionInput;
    use chrono::{Duration, Utc};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(Uuid::new_v4(), "step", "echo hi", vec![], "say hi");
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node).unwrap();
    graph.seal().unwrap();

    let persisted_started_at = Utc::now() - Duration::minutes(10);
    let hydrate = executor
        .hydrate_execution(HydrateExecutionInput {
            dag_id,
            graph,
            node_states: Default::default(),
            approved: Default::default(),
            started_at: persisted_started_at,
        })
        .await
        .unwrap();
    assert!(hydrate.created, "session must be created by hydration");

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert_eq!(
        state.started_at,
        Some(persisted_started_at),
        "hydrated session must keep the persisted start time"
    );
    // While paused, reported duration is wall-clock since the preserved start
    // — a fresh Utc::now() baseline would under-report by the pause length.
    assert!(
        state.total_duration_ms >= 10 * 60 * 1000,
        "duration must include the 10-minute pause, got {:?}",
        state.total_duration_ms
    );
}

#[tokio::test]
async fn test_approval_gate_pauses_until_human_signoff() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    // Graph: [safe] and [risky] are independent; risky requires approval.
    let safe = TaskNode::new(
        Uuid::new_v4(),
        "safe",
        "run_command",
        vec![],
        r#"{"command": "echo safe"}"#,
    );
    let risky = TaskNode::new(
        Uuid::new_v4(),
        "risky",
        "run_command",
        vec![],
        r#"{"command": "echo risky"}"#,
    )
    .with_requires_approval(true);
    let mut graph = TaskGraph::new();
    graph.add_unchecked(safe).unwrap();
    graph.add_unchecked(risky).unwrap();
    graph.seal().unwrap();

    // 1. Execute → pauses at the approval boundary; the approval-required
    // step is NOT dispatched and execution is paused for human sign-off.
    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    assert!(output.approval_pending, "expected approval-pending output");
    assert_eq!(output.pending_approval_steps, vec!["risky".to_string()]);

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(state.paused, "execution should be paused");
    assert!(!state.is_complete, "execution should not be terminal");

    // 2. Approve the risky step (human sign-off).
    let approve = executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["risky".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(approve.approved, vec!["risky".to_string()]);
    assert!(approve.still_pending.is_empty());

    // 3. Resume → the remaining node runs and the execution completes.
    let resume = executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    assert_eq!(resume.dag_id, dag_id);

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(!state.paused, "execution should be resumed");
    assert_eq!(state.completed_count, 2, "both nodes should complete");
    assert!(state.is_complete, "execution should be complete");
}

#[tokio::test]
async fn test_cross_process_resume_hydrates_session_and_continues() {
    // GAP-3: a run paused for approval in process A must be resumable from
    // process B. Process B has NO live session — it hydrates from the
    // persisted ExecutionState (sealed graph + node states + approved set),
    // then approves + resumes the DAG exactly where dispatch paused.
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::execution_engine::application::dto::HydrateExecutionInput;

    // ── Process A: build a gated graph, run it, pause at approval, and
    //    capture the session state (graph + node states) as process B would
    //    receive it via the persisted ExecutionState. ──
    let executor_a = create_executor();
    let dag_id = Uuid::new_v4();

    let backup = TaskNode::new(
        Uuid::new_v4(),
        "backup",
        "run_command",
        vec![],
        r#"{"command": "echo backup"}"#,
    );
    let migrate = TaskNode::new(
        Uuid::new_v4(),
        "migrate",
        "run_command",
        vec![backup.id],
        r#"{"command": "echo migrate"}"#,
    )
    .with_requires_approval(true);
    let verify = TaskNode::new(
        Uuid::new_v4(),
        "verify",
        "run_command",
        vec![migrate.id],
        r#"{"command": "echo verify"}"#,
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(backup).unwrap();
    graph.add_unchecked(migrate).unwrap();
    graph.add_unchecked(verify).unwrap();
    graph.seal().unwrap();

    let output_a = executor_a
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph.clone()),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(
        output_a.approval_pending,
        "process A must pause at approval"
    );
    assert_eq!(output_a.pending_approval_steps, vec!["migrate".to_string()]);

    // The live node states from A's run (backup completed, migrate awaiting,
    // verify pending) — what A would persist into ExecutionState.
    let state_a = executor_a
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    let node_states: std::collections::HashMap<_, _> =
        state_a.node_states.clone().into_iter().collect();
    assert_eq!(state_a.completed_count, 1, "backup completed before pause");

    // ── Process B: a FRESH executor has no session for dag_id. Approve first
    //    fails with NodeNotFound, exactly as observed in the GAP-3 bug. ──
    //
    // Crucially, the graph arrives SERIALIZED (as persisted in the state
    // file): TaskGraph serializes `sealed: true` but `execution_state` is
    // #[serde(skip)], so the deserialized graph has an EMPTY ready queue.
    // Hydration must rebuild execution state from the nodes, not reuse it.
    let graph_serialized: serde_json::Value = serde_json::to_value(&graph).unwrap();
    let graph: TaskGraph = serde_json::from_value(graph_serialized).unwrap();
    assert!(graph.sealed, "persisted graph must deserialize as sealed");
    assert!(
        graph.ready_nodes().is_empty(),
        "deserialized execution_state is empty — this is the GAP-3 trap"
    );

    let executor_b = create_executor();
    let err = executor_b
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["migrate".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            crate::execution_engine::domain::ExecutionError::NodeNotFound { .. }
        ),
        "fresh process must not find the session (NodeNotFound)"
    );

    // B hydrates from the persisted state, then approves + resumes.
    let hydrate = executor_b
        .hydrate_execution(HydrateExecutionInput {
            dag_id,
            graph,
            node_states,
            approved: Default::default(),
            // H-08: the persisted start must survive hydration (the resume
            // duration is computed from this timestamp).
            started_at: state_a.started_at.unwrap_or_else(chrono::Utc::now),
        })
        .await
        .unwrap();
    assert!(hydrate.created, "session must be created by hydration");
    assert_eq!(hydrate.node_count, 3);

    let approve = executor_b
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["migrate".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(approve.approved, vec!["migrate".to_string()]);
    assert!(approve.still_pending.is_empty());

    let resume = executor_b
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    assert_eq!(resume.dag_id, dag_id);

    let state_b = executor_b
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(!state_b.paused, "execution should be resumed");
    assert!(
        state_b.is_complete,
        "hydrated run should complete: backup (done in A) + migrate + verify"
    );
}

/// GAP-H-07: approving a node that never requested approval must be denied.
#[tokio::test]
async fn test_approve_node_rejects_ungated_node() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    // NOT approval-gated.
    let node = TaskNode::new(
        Uuid::new_v4(),
        "plain",
        "run_command",
        vec![],
        r#"{"command": "echo hi"}"#,
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node).unwrap();
    graph.seal().unwrap();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let approve = executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["plain".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert!(
        approve.approved.is_empty(),
        "ungated node must not be approvable"
    );
    assert_eq!(approve.denied, vec!["plain".to_string()]);
}

/// GAP-H-07: approving a node that is not currently AwaitingApproval
/// (already approved / executed) must be denied.
#[tokio::test]
async fn test_approve_node_rejects_non_awaiting_node() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(
        Uuid::new_v4(),
        "gated",
        "run_command",
        vec![],
        r#"{"command": "echo hi"}"#,
    )
    .with_requires_approval(true);
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node.clone()).unwrap();
    graph.seal().unwrap();

    // First execution pauses at the gate.
    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(output.approval_pending);

    // First approval succeeds.
    let first = executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["gated".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(first.approved, vec!["gated".to_string()]);
    assert!(first.denied.is_empty());

    // Second approval — the node is no longer AwaitingApproval — is denied.
    let second = executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["gated".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert!(second.approved.is_empty(), "double approval must be denied");
    assert_eq!(second.denied, vec!["gated".to_string()]);
}

// ---------------------------------------------------------------------------
// Permission-mode gating through the factory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_approval_gate_rejects_unknown_step_name() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let risky = TaskNode::new(Uuid::new_v4(), "risky", "echo risky", vec![], "run risky")
        .with_requires_approval(true);
    let mut graph = TaskGraph::new();
    graph.add_unchecked(risky).unwrap();
    graph.seal().unwrap();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let approve = executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["nope".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert!(approve.approved.is_empty());
    assert_eq!(approve.not_found, vec!["nope".to_string()]);
    assert_eq!(approve.still_pending, vec!["risky".to_string()]);
    assert!(approve.denied.is_empty());

    // Not approved → resume still leaves the node blocked, so execution
    // remains paused.
    let _ = executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(state.paused, "execution stays paused until real approval");
}

#[tokio::test]
async fn test_pause_and_resume_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    // Start execution
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    // Pause
    let pause_output = executor
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap();
    assert_eq!(pause_output.dag_id, dag_id);

    // Verify paused state
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(state.paused);

    // Resume
    let resume_output = executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    assert_eq!(resume_output.dag_id, dag_id);

    // Verify resumed state
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(!state.paused);
}

#[tokio::test]
async fn test_pause_already_paused_returns_error() {
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
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap();

    let err = executor
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("already paused"));
}

#[tokio::test]
async fn test_resume_not_paused_returns_error() {
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

    let err = executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("not paused"));
}

#[tokio::test]
async fn test_pause_nonexistent_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let err = executor
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::execution_engine::domain::ExecutionError::NodeNotFound { .. }
    ));
}

/// AC#9 (promote): step A completes, then B is proposed → B is promoted into
/// the existing approval pause; approve → resume → B dispatches and executes
/// (its side effect lands on disk).
#[tokio::test]
async fn test_r3_promote_pauses_dynamic_later_step_and_approve_executes_it() {
    use crate::execution_engine::domain::NodeStatus;

    let (path_a, path_b) = r3_paths("promote");
    let executor = r3_executor(RuleAction::Promote, &path_a, &path_b);
    let dag_id = Uuid::new_v4();
    let (graph, _a_id, _b_id) = r3_chain_graph(&path_a, &path_b);

    // 1. Execute: A (already ready) dispatches and completes; B is promoted
    // at the dispatch boundary BEFORE its tool is called — run pauses.
    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(
        output.approval_pending,
        "promoted dynamic step must pause the run"
    );
    assert_eq!(output.pending_approval_steps, vec!["step_b".to_string()]);

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(state.paused, "execution should be paused at the R3 gate");
    assert!(!state.is_complete, "not terminal while gated");

    let step_a = state
        .node_states
        .values()
        .find(|s| s.node_name == "step_a")
        .expect("step_a state");
    assert_eq!(
        step_a.status,
        NodeStatus::Completed,
        "completed prefix step A must have executed before the gate"
    );
    let step_b = state
        .node_states
        .values()
        .find(|s| s.node_name == "step_b")
        .expect("step_b state");
    assert_eq!(
        step_b.status,
        NodeStatus::AwaitingApproval,
        "promote rule must gate the later dynamic step"
    );
    assert!(
        step_b.started_at.is_none(),
        "promoted step must not dispatch pre-approval"
    );
    assert!(
        !std::path::Path::new(&path_b).exists(),
        "B's tool must not have been called before approval"
    );
    assert!(
        std::path::Path::new(&path_a).exists(),
        "A (the completed prefix step) must have executed"
    );

    // 2. Human approval → the promoted node becomes dispatchable again.
    let approve = executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["step_b".to_string()],
            approver_id: None,
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(approve.approved, vec!["step_b".to_string()]);

    // 3. Resume → B dispatches through the SAME approval machinery and its
    // tool executes (side effect observed on disk); run completes.
    executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(!state.paused, "execution should resume after approval");
    assert!(state.is_complete, "approved run must complete");
    assert_eq!(state.completed_count, 2);

    let content = tokio::fs::read_to_string(&path_b).await.unwrap();
    assert_eq!(content, "b", "approved B tool must have executed");
}
