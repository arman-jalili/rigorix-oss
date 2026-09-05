//! ParallelExecutor implementation tests.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: ParallelExecutor — ParallelExecutionServiceImpl tests
//! Issue: issue-parallelexecutor
//!
//! Comprehensive tests for the ParallelExecutionServiceImpl and
//! RetryEvaluationServiceImpl implementations.

use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::execution_engine::application::dto::{
    AbortExecutionInput, ApproveNodeInput, EvaluateRetryInput, ExecuteGraphInput, ExecuteNodeInput,
    GetExecutionStateInput, PauseExecutionInput, ResumeExecutionInput,
};
use crate::execution_engine::application::service::{
    ParallelExecutionService, RetryEvaluationService,
};
use crate::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use crate::execution_engine::domain::{
    BackoffStrategy, FailureContext, NodeExecutionState, ParallelExecutorConfig, RetryDecision,
    RetryPolicy, RetryStrategy,
};
use crate::failure_classification::application::failure_classifier_service_impl::FailureClassifierServiceImpl;

// ---------------------------------------------------------------------------
// Helper: create a configured service pair
// ---------------------------------------------------------------------------

fn create_executor() -> ParallelExecutionServiceImpl {
    let config = ParallelExecutorConfig::default();
    let retry = RetryEvaluationServiceImpl::new();
    let event_bus = Arc::new(EventBusServiceImpl::default());
    ParallelExecutionServiceImpl::new(config, Box::new(retry), event_bus)
}

/// A-02: minimal sealed graph for session-setup tests (abort/pause/state).
fn sample_graph() -> crate::dag_engine::domain::TaskGraph {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    let mut graph = TaskGraph::new();
    let root = TaskNode::new(Uuid::new_v4(), "root", "shell", vec![], "echo hi");
    graph.add_unchecked(root).unwrap();
    graph.seal().unwrap();
    graph
}

// ---------------------------------------------------------------------------
// ParallelExecutionServiceImpl Tests
// ---------------------------------------------------------------------------

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

#[tokio::test]
async fn test_execute_graph_with_config_override() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let custom_config = ParallelExecutorConfig {
        max_concurrent_executions: 8,
        ..Default::default()
    };

    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    let mut graph = TaskGraph::new();
    let root = TaskNode::new(Uuid::new_v4(), "root", "shell", vec![], "echo hi");
    graph.add_unchecked(root.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: Some(custom_config),
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

#[tokio::test]
async fn test_execute_node_returns_result() {
    let executor = create_executor();
    let node_id = Uuid::new_v4();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: Some(fast_non_retriable_policy()),
        })
        .await
        .unwrap();

    // A-02: a node that does not exist in any session graph must FAIL, not
    // return the old placeholder success.
    assert_eq!(output.result.node_id, node_id);
    assert!(!output.result.success);
    let err = output.result.error.as_deref().unwrap_or("");
    assert!(
        err.contains("node_not_found") || err.contains("not found"),
        "error should mention node_not_found, got: {err}"
    );
}

#[tokio::test]
async fn test_execute_node_with_retry_policy() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: Some(fast_non_retriable_policy()),
        })
        .await
        .unwrap();

    // A-02: missing node fails regardless of the configured retry policy.
    assert_eq!(output.result.node_id, node_id);
    assert!(!output.result.success);
    assert!(
        output
            .result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("node_not_found"),
        "missing node must fail, not succeed"
    );
}

/// Fast retry policy that treats the failure as non-retriable: a single
/// attempt, immediate backoff, no sleep, so tests stay deterministic.
fn fast_non_retriable_policy() -> RetryPolicy {
    RetryPolicy {
        max_attempts: 1,
        retryable_failures: vec!["__never_matches__".to_string()],
        backoff_strategy: BackoffStrategy::Immediate,
        ..Default::default()
    }
}

/// A-01 (parallel path): an unknown tool name in a graph must produce a
/// node failure, never a silent success.
#[tokio::test]
async fn test_unknown_tool_fails_graph_execution() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "no_such_tool",
        vec![],
        "no such tool",
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let nr = output
        .result
        .node_results
        .get(&node.id)
        .expect("node result present");
    assert!(
        !nr.success,
        "unknown tool must fail the node, not report success"
    );
    assert_eq!(
        nr.failure_type.as_deref(),
        Some("unknown_tool"),
        "failure type must be unknown_tool"
    );
    assert!(
        nr.error.as_deref().unwrap_or("").contains("no_such_tool"),
        "error must name the tool"
    );
}

/// A-01 (single-node path): the same unknown-tool failure must surface via
/// `execute_node` (the `execute_tool` dispatch), not just the parallel loop.
#[tokio::test]
async fn test_unknown_tool_fails_single_node_path() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    // Approval-gated so execute_graph leaves the node in the session graph
    // without dispatching it; then execute_node drives the single-node path.
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "no_such_tool",
        vec![],
        "no such tool",
    )
    .with_requires_approval(true);
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(output.approval_pending, "gate must pause before dispatch");

    let node_out = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id: node.id,
            retry_policy: Some(fast_non_retriable_policy()),
        })
        .await
        .unwrap();
    assert!(
        !node_out.result.success,
        "single-node path must fail on unknown tool"
    );
    let err = node_out.result.error.as_deref().unwrap_or("");
    assert!(
        err.contains("no_such_tool") || err.contains("unknown_tool"),
        "error must reference the unknown tool, got: {err}"
    );
}

/// A-05: PreToolUse hooks must gate the PARALLEL dispatch loop, not just the
/// single-node `execute_tool` path. A hook returning deny blocks the tool.
#[tokio::test]
async fn test_pre_tool_use_hook_blocks_parallel_dispatch() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::hooks::application::hook_runner_impl::HookRunnerImpl;
    use crate::hooks::domain::config::HookConfig;

    let retry =
        crate::execution_engine::application::service_impl::RetryEvaluationServiceImpl::new();
    let event_bus = Arc::new(EventBusServiceImpl::default());
    let runner = Arc::new(HookRunnerImpl::new(HookConfig {
        pre_tool_use: vec![r#"echo '{"decision":"Deny","reason":"blocked by test"}'"#.into()],
        ..Default::default()
    }));
    let executor = ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig::default(),
        Box::new(retry),
        event_bus,
    )
    .with_hook_runner(runner);

    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "run_command",
        vec![],
        r#"{"command": "echo hi"}"#,
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let nr = output
        .result
        .node_results
        .get(&node.id)
        .expect("node result present");
    assert!(
        !nr.success,
        "PreToolUse deny must block the tool in the parallel path"
    );
    assert_eq!(
        nr.failure_type.as_deref(),
        Some("hook_blocked"),
        "failure type must be hook_blocked"
    );
    assert!(
        nr.error
            .as_deref()
            .unwrap_or("")
            .contains("blocked by test"),
        "error must carry the hook reason"
    );
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

/// GAP-A-11: max_total_retries_per_session stops the retry loop once the
/// session-wide budget is consumed.
#[tokio::test]
async fn test_max_total_retries_per_session_stops_retrying() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let retry =
        crate::execution_engine::application::service_impl::RetryEvaluationServiceImpl::new();
    let event_bus = Arc::new(EventBusServiceImpl::default());
    let executor = ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig {
            max_total_retries_per_session: 1,
            ..Default::default()
        },
        Box::new(retry),
        event_bus,
    );
    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(Uuid::new_v4(), "fragile", "no_such_tool", vec![], "boom")
        .with_requires_approval(true);
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(output.approval_pending);

    let node_out = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id: node.id,
            retry_policy: None, // default: all failures retriable, 4 attempts
        })
        .await
        .unwrap();

    assert!(!node_out.result.success);
    assert_eq!(
        node_out.result.failure_type.as_deref(),
        Some("retry_budget_exhausted"),
        "retry loop must stop at the session budget, got {:?}",
        node_out.result.failure_type
    );
}

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
async fn test_get_execution_state_before_execution_returns_error() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let err = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::execution_engine::domain::ExecutionError::NodeNotFound { .. }
    ));
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
async fn test_factory_threads_permission_enforcer_read_only_gates_bash_write() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use crate::permission::application::enforcer_factory_impl::PermissionEnforcerFactoryImpl;
    use crate::permission::application::factory::PermissionEnforcerFactory;
    use crate::permission::domain::mode::PermissionMode;

    // Build a ReadOnly enforcer and thread it through the factory config —
    // this is exactly what the CLI/action/MCP entry points now do.
    let enforcer = PermissionEnforcerFactoryImpl
        .create_with_mode(PermissionMode::ReadOnly)
        .await
        .expect("read-only enforcer construction");

    let service = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            permission_enforcer: Some(Arc::from(enforcer)),
            ..Default::default()
        })
        .await
        .expect("factory create");

    let dag_id = Uuid::new_v4();
    // `run_command` requires WorkspaceWrite → denied in ReadOnly before exec.
    let write_node = TaskNode::new(
        Uuid::new_v4(),
        "write",
        "run_command",
        vec![],
        "touch /tmp/rigorix-perm",
    );
    // `grep_search` is allow-listed at ReadOnly → real file_read completes.
    let read_node = TaskNode::new(
        Uuid::new_v4(),
        "read",
        "file_read",
        vec![],
        r#"{"path": "Cargo.toml"}"#,
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(write_node).unwrap();
    graph.add_unchecked(read_node).unwrap();
    graph.seal().unwrap();

    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(output.result.dag_id, dag_id);

    let state = service
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert_eq!(
        state.failed_count, 1,
        "bash write command must be denied in read_only mode"
    );
    assert_eq!(
        state.completed_count, 1,
        "allow-listed read tool must still complete"
    );
}
/// AC#13 (permission R5): a `workspace_write` agent file-write to
/// `.rigorix/**` is denied by the DEFAULT permission config — the operator's
/// sequence rules are never writable by the agent they judge.
#[tokio::test]
async fn test_default_permission_denies_rigorix_config_write() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use crate::permission::application::enforcer_factory_impl::PermissionEnforcerFactoryImpl;
    use crate::permission::application::factory::PermissionEnforcerFactory;

    // Default posture: PermissionConfig::default() (workspace_write mode).
    let enforcer = PermissionEnforcerFactoryImpl
        .create_default()
        .await
        .expect("default enforcer construction");

    let service = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            permission_enforcer: Some(Arc::from(enforcer)),
            ..Default::default()
        })
        .await
        .expect("factory create");

    let dag_id = Uuid::new_v4();
    // Agent tries to overwrite the operator's sequence-policy rules.
    let rule_write = TaskNode::new(
        Uuid::new_v4(),
        "write_rule",
        "file_write",
        vec![],
        r#"{"path": ".rigorix/sequence-policy.toml", "content": "evil"}"#,
    );
    // Control: an ordinary workspace write under the same mode stays allowed.
    let scratch = std::env::current_dir()
        .expect("cwd")
        .join(format!("rigorix-r5-control-{}.txt", Uuid::new_v4()))
        .display()
        .to_string();
    let scratch_write = TaskNode::new(
        Uuid::new_v4(),
        "write_scratch",
        "file_write",
        vec![],
        format!(r#"{{"path": "{}", "content": "ok"}}"#, scratch),
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(rule_write).unwrap();
    graph.add_unchecked(scratch_write).unwrap();
    graph.seal().unwrap();

    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let rule_node_id = output
        .result
        .execution_states
        .iter()
        .find(|(_, s)| s.node_name == "write_rule")
        .map(|(id, _)| *id)
        .expect("write_rule node");
    let rule_result = output
        .result
        .node_results
        .get(&rule_node_id)
        .expect("rule write result");
    assert!(!rule_result.success, "rule file write must be denied");
    assert_eq!(
        rule_result.failure_type.as_deref(),
        Some("permission_denied"),
        "denial surfaces as the structured permission_denied failure"
    );
    assert!(
        !std::path::Path::new(".rigorix/sequence-policy.toml").exists()
            || std::fs::read_to_string(".rigorix/sequence-policy.toml")
                .map(|c| !c.contains("evil"))
                .unwrap_or(true),
        "the operator rule file must be untouched"
    );

    // Control leg: same-mode ordinary write completed.
    let state = service
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert_eq!(state.completed_count, 1, "scratch write completes");
    let _ = std::fs::remove_file(&scratch);
}

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
async fn test_on_progress_callback() {
    let executor = create_executor();
    let _dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let called_clone = called.clone();

    executor.on_progress(Box::new(move |_progress| {
        called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    }));

    // Create a node state to trigger notification
    let _state = NodeExecutionState::new(node_id, "test-node");

    // Verify callback registered (not triggered since no session)
    // The callback mechanism is trigger-based; in a real execution it fires on completion
    assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn test_execute_graph_with_custom_config_override_respected() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let config = ParallelExecutorConfig {
        max_concurrent_executions: 16,
        enable_fallback: false,
        enable_validation: false,
        ..Default::default()
    };

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: Some(config),
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

// ---------------------------------------------------------------------------
// RetryEvaluationServiceImpl Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_retry_evaluate_retry_on_first_failure() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy::default();

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "network timeout",
        0, // first failure
        4, // max 4 attempts
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(!output.is_terminal);
    assert!(output.decision.is_retry());
}

#[tokio::test]
async fn test_retry_evaluate_retry_on_exhausted() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy::default();

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "still failing",
        3, // attempt 3 = 4th attempt = last
        4, // max 4 attempts
        100,
        400,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    // No fallback configured, skip_on_exhaustion=false → Abort
    match output.decision {
        RetryDecision::Abort { .. } => {} // expected
        ref other => panic!("Expected Abort, got: {:?}", other),
    }
}

// Fix: Compare by variant
#[tokio::test]
async fn test_retry_exhausted_with_skip_on_exhaustion() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        skip_on_exhaustion: true,
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "failed",
        3, // last attempt
        4,
        100,
        400,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    assert!(matches!(output.decision, RetryDecision::Skip { .. }));
}

#[tokio::test]
async fn test_retry_exhausted_with_fallback() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let fallback_id = Uuid::new_v4();
    let policy = RetryPolicy::default();

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "failed too many times",
        3,
        4,
        100,
        400,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: Some(fallback_id),
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    match output.decision {
        RetryDecision::Fallback {
            fallback_node_id, ..
        } => {
            assert_eq!(fallback_node_id, fallback_id);
        }
        other => panic!("Expected Fallback decision, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_non_retriable_failure() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        retryable_failures: vec!["transient".to_string()],
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "compile_error", // not in retryable_failures
        "syntax error",
        0,
        4,
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    assert!(matches!(output.decision, RetryDecision::Skip { .. }));
}

#[tokio::test]
async fn test_retry_non_retriable_with_fallback() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let fallback_id = Uuid::new_v4();
    let policy = RetryPolicy {
        retryable_failures: vec!["transient".to_string()],
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "compile_error",
        "syntax error",
        0,
        4,
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: Some(fallback_id),
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    match output.decision {
        RetryDecision::Fallback {
            fallback_node_id, ..
        } => {
            assert_eq!(fallback_node_id, fallback_id);
        }
        other => panic!("Expected Fallback, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_strategy_escalation() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        retry_strategies: vec![
            RetryStrategy::SameOperation,
            RetryStrategy::ExpandContext,
            RetryStrategy::AlternateApproach,
        ],
        ..Default::default()
    };

    // First failure → SameOperation
    let ctx1 = FailureContext::new(
        node_id,
        "n",
        "tool",
        "intent",
        "transient",
        "err",
        0,
        4,
        100,
        100,
    );
    let output1 = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx1,
            policy: policy.clone(),
            fallback_node_id: None,
        })
        .await
        .unwrap();
    match output1.decision {
        RetryDecision::Retry {
            strategy, attempt, ..
        } => {
            assert_eq!(strategy, RetryStrategy::SameOperation);
            assert_eq!(attempt, 1);
        }
        other => panic!("Expected Retry, got: {:?}", other),
    }

    // Second failure → ExpandContext
    let ctx2 = FailureContext::new(
        node_id,
        "n",
        "tool",
        "intent",
        "transient",
        "err",
        1,
        4,
        100,
        200,
    );
    let output2 = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx2,
            policy: policy.clone(),
            fallback_node_id: None,
        })
        .await
        .unwrap();
    match output2.decision {
        RetryDecision::Retry {
            strategy, attempt, ..
        } => {
            assert_eq!(strategy, RetryStrategy::ExpandContext);
            assert_eq!(attempt, 2);
        }
        other => panic!("Expected Retry, got: {:?}", other),
    }

    // Third failure → AlternateApproach
    let ctx3 = FailureContext::new(
        node_id,
        "n",
        "tool",
        "intent",
        "transient",
        "err",
        2,
        4,
        100,
        300,
    );
    let output3 = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx3,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();
    match output3.decision {
        RetryDecision::Retry {
            strategy, attempt, ..
        } => {
            assert_eq!(strategy, RetryStrategy::AlternateApproach);
            assert_eq!(attempt, 3);
        }
        other => panic!("Expected Retry, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_compute_backoff_exponential() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Exponential {
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 10_000,
        },
        ..Default::default()
    };

    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let backoff = service.compute_backoff(&ctx, &policy).await;
    assert_eq!(backoff, 100); // 100 * 2^0

    let ctx2 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 1, 4, 100, 200);
    let backoff2 = service.compute_backoff(&ctx2, &policy).await;
    assert_eq!(backoff2, 200); // 100 * 2^1
}

#[tokio::test]
async fn test_compute_backoff_fixed() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Fixed { base_delay_ms: 500 },
        ..Default::default()
    };

    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let backoff = service.compute_backoff(&ctx, &policy).await;
    assert_eq!(backoff, 500);

    let ctx2 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 3, 4, 100, 400);
    let backoff2 = service.compute_backoff(&ctx2, &policy).await;
    assert_eq!(backoff2, 500);
}

#[tokio::test]
async fn test_compute_backoff_immediate() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Immediate,
        ..Default::default()
    };

    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let backoff = service.compute_backoff(&ctx, &policy).await;
    assert_eq!(backoff, 0);
}

#[tokio::test]
async fn test_validate_policy_valid() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy::default();

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.is_empty());
}

#[tokio::test]
async fn test_validate_policy_zero_attempts() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        max_attempts: 0,
        ..Default::default()
    };

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.iter().any(|e| e.contains("max_attempts")));
}

#[tokio::test]
async fn test_validate_policy_empty_strategies() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        retry_strategies: vec![],
        ..Default::default()
    };

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.iter().any(|e| e.contains("retry_strategies")));
}

#[tokio::test]
async fn test_validate_policy_bad_multiplier() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Exponential {
            base_delay_ms: 100,
            multiplier: 0.5, // must be >= 1.0
            max_delay_ms: 10_000,
        },
        ..Default::default()
    };

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.iter().any(|e| e.contains("multiplier")));
}

#[tokio::test]
async fn test_retry_decision_driven_by_classification_transient() {
    // GAP-A-19: a confident Transient classification (network/timeout)
    // drives a Retry decision.
    let service = RetryEvaluationServiceImpl::with_classifier(std::sync::Arc::new(
        FailureClassifierServiceImpl,
    ));
    let policy = RetryPolicy {
        // Policy says this failure_code is NOT retriable; the structured
        // classification (Transient) overrides and grants the retry.
        retryable_failures: vec!["compile_error".to_string()],
        max_attempts: 3,
        ..Default::default()
    };
    let ctx = FailureContext::new(
        uuid::Uuid::new_v4(),
        "test-node",
        "run-command",
        "run",
        "command_failed",
        "connection to host timed out after 30s",
        0,
        3,
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(
        output.decision.is_retry(),
        "Transient must retry: {:?}",
        output.decision
    );
}

#[tokio::test]
async fn test_retry_decision_driven_by_classification_build_failure_skips() {
    // GAP-A-19: a confident BuildFailure classification is NOT retryable
    // even though the policy would allow the code.
    let service = RetryEvaluationServiceImpl::with_classifier(std::sync::Arc::new(
        FailureClassifierServiceImpl,
    ));
    let policy = RetryPolicy::default(); // all codes retriable
    let ctx = FailureContext::new(
        uuid::Uuid::new_v4(),
        "test-node",
        "cargo build",
        "build",
        "command_failed",
        "error: build failed: cannot compile",
        0,
        3,
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(
        matches!(output.decision, RetryDecision::Skip { .. }),
        "BuildFailure must skip: {:?}",
        output.decision
    );
}

#[tokio::test]
async fn test_retry_decision_unclassified_defers_to_policy() {
    // GAP-A-19: an unmatched message (documented NonRetryable default) is
    // treated as unclassified and defers to the policy (all retriable here).
    let service = RetryEvaluationServiceImpl::with_classifier(std::sync::Arc::new(
        FailureClassifierServiceImpl,
    ));
    let policy = RetryPolicy::default();
    let ctx = FailureContext::new(
        uuid::Uuid::new_v4(),
        "test-node",
        "tool",
        "op",
        "custom_failure",
        "some completely unusual error text",
        0,
        3,
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(
        output.decision.is_retry(),
        "unclassified must defer to policy: {:?}",
        output.decision
    );
}

#[tokio::test]
async fn test_is_failure_retriable_default_all() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy::default(); // empty retryable_failures = all retriable

    assert!(service.is_failure_retriable(&policy, "transient").await);
    assert!(service.is_failure_retriable(&policy, "compile_error").await);
    assert!(service.is_failure_retriable(&policy, "permanent").await);
}

// ---------------------------------------------------------------------------
// Factory Implementation Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_parallel_execution_factory_creates_service() {
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;

    let factory = ParallelExecutionFactoryImpl::new();
    let config = ParallelExecutionFactoryConfig::default();
    let service = factory.create(config).await.unwrap();

    let dag_id = Uuid::new_v4();
    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

#[tokio::test]
async fn test_retry_evaluation_factory_creates_service() {
    use crate::execution_engine::application::factory::{
        RetryEvaluationFactory, RetryEvaluationFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::RetryEvaluationFactoryImpl;

    let factory = RetryEvaluationFactoryImpl::new();
    let config = RetryEvaluationFactoryConfig::default();
    let service = factory.create(config).await.unwrap();

    let node_id = Uuid::new_v4();
    let policy = RetryPolicy::default();
    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.decision.is_retry());
}

#[tokio::test]
async fn test_factory_with_custom_config() {
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use crate::execution_engine::domain::ParallelExecutorConfig;

    let factory = ParallelExecutionFactoryImpl::new();
    let custom_executor_config = ParallelExecutorConfig {
        max_concurrent_executions: 16,
        enable_fallback: false,
        ..Default::default()
    };
    let config = ParallelExecutionFactoryConfig {
        executor_config: custom_executor_config,
        ..Default::default()
    };

    let service = factory.create(config).await.unwrap();
    let dag_id = Uuid::new_v4();

    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

// ---------------------------------------------------------------------------
// Inline Retry Loop Tests
// ---------------------------------------------------------------------------

/// Create an approval-paused session holding a single `run_command` node, so
/// `execute_node` can drive the single-node dispatch (inline retry loop)
/// against a REAL tool — fake tool names now fail (GAP-A-01).
async fn paused_session_with_node() -> (ParallelExecutionServiceImpl, Uuid, Uuid) {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "run_command",
        vec![],
        r#"{"command": "echo hi"}"#,
    )
    .with_requires_approval(true);
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(output.approval_pending, "gate must pause before dispatch");
    (executor, dag_id, node.id)
}

#[tokio::test]
async fn test_inline_retry_loop_succeeds_on_first_attempt() {
    let (executor, dag_id, node_id) = paused_session_with_node().await;

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: None,
        })
        .await
        .unwrap();

    assert!(
        output.result.success,
        "real tool must succeed on first attempt"
    );
    assert_eq!(output.result.node_id, node_id);
    assert_eq!(output.result.retry_attempts, 0);
    assert!(output.retry_decision.is_none());
}

#[tokio::test]
async fn test_inline_retry_loop_with_retry_policy() {
    let (executor, dag_id, node_id) = paused_session_with_node().await;

    let policy = RetryPolicy {
        max_attempts: 2,
        ..Default::default()
    };

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: Some(policy),
        })
        .await
        .unwrap();

    // A real tool succeeds on the first attempt — no retries consumed.
    assert!(output.result.success);
    assert_eq!(output.result.retry_attempts, 0);
}

#[tokio::test]
async fn test_inline_retry_loop_uses_default_policy_when_none_provided() {
    let (executor, dag_id, node_id) = paused_session_with_node().await;

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: None, // Should use default_retry_policy from config
        })
        .await
        .unwrap();

    assert!(output.result.success);
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
async fn test_is_failure_retriable_filtered() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        retryable_failures: vec!["transient".to_string(), "lsp_conflict".to_string()],
        ..Default::default()
    };

    assert!(service.is_failure_retriable(&policy, "transient").await);
    assert!(service.is_failure_retriable(&policy, "lsp_conflict").await);
    assert!(!service.is_failure_retriable(&policy, "compile_error").await);
    assert!(!service.is_failure_retriable(&policy, "permanent").await);
}

#[tokio::test]
async fn test_decide_skip_on_skip_and_continue_strategy() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        retry_strategies: vec![RetryStrategy::SkipAndContinue],
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "tool",
        "intent",
        "transient",
        "error",
        0, // first attempt → strategy at index 0 = SkipAndContinue
        4,
        100,
        100,
    );

    let decision = service.decide(&ctx, &policy, None).await;
    assert!(decision.is_terminal());
    assert!(matches!(decision, RetryDecision::Skip { .. }));
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

#[tokio::test]
async fn test_decide_skip_on_skip_conditions() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        skip_conditions: Some(vec!["test skip".to_string()]),
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "tool",
        "intent",
        "transient",
        "this is a test skip condition",
        0,
        4,
        100,
        100,
    );

    let decision = service.decide(&ctx, &policy, None).await;
    assert!(decision.is_terminal());
    match decision {
        RetryDecision::Skip { reason } => {
            assert!(reason.contains("test skip"));
        }
        other => panic!("Expected Skip decision, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_progress_callback_fires() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let called = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let called_clone = called.clone();

    executor.on_progress(Box::new(move |progress| {
        assert_eq!(progress.dag_id, dag_id);
        called_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));

    // Trigger a progress notification directly via the internal mechanism
    // This is an internal implementation detail test
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    // GAP-A-02 change: the graph now really executes (sample_graph has one
    // shell node), so the progress callback fires for the completed node.
    // This also guards the notify_progress re-lock deadlock (regression):
    // a registered callback used to self-deadlock on the sessions Mutex.
    let fired = called.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        fired >= 1,
        "progress callback should fire for completed node, got {fired}"
    );
}

#[tokio::test]
async fn test_retry_with_skip_and_continue_strategy_index() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();

    // Strategy 0 = SameOperation, Strategy 1 = SkipAndContinue
    let policy = RetryPolicy {
        retry_strategies: vec![RetryStrategy::SameOperation, RetryStrategy::SkipAndContinue],
        ..Default::default()
    };

    // First failure: attempt 0 → strategy[0] = SameOperation (not skip)
    let ctx1 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let decision1 = service.decide(&ctx1, &policy, None).await;
    assert!(decision1.is_retry());

    // Second failure: attempt 1 → strategy[1] = SkipAndContinue (skip)
    let ctx2 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 1, 4, 100, 200);
    let decision2 = service.decide(&ctx2, &policy, None).await;
    assert!(decision2.is_terminal());
    assert!(matches!(decision2, RetryDecision::Skip { .. }));
}

// ---------------------------------------------------------------------------
// R3 sequence-policy run-time prefix gate (AC#9)
// ---------------------------------------------------------------------------
//
// Dynamic-plan semantics: step A completes, then step B is proposed (would
// complete the forbidden pair). The dispatch loop's prefix gate evaluates the
// session's completed prefix + B BEFORE B dispatches — promote routes B into
// the existing approval pause; deny fails B pre-dispatch (tool never called).
// Real in-process tools (file_append / file_write against the temp dir) prove
// the "executes / never called" legs with observable side effects.

use crate::sequence_policy::application::SequencePolicyServiceImpl;
use crate::sequence_policy::domain::{
    ParamMatchKind, ParamPredicate, RuleAction, SequencePolicyConfig, SequencePolicyError,
    SequenceRule, StepPredicate,
};
use crate::sequence_policy::infrastructure::SequencePolicyRepository;

/// In-memory rule-config double (no filesystem).
struct R3PolicyRepo {
    config: Option<SequencePolicyConfig>,
}

#[async_trait]
impl SequencePolicyRepository for R3PolicyRepo {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        Ok(self.config.clone())
    }
}

/// AC9 conference-style rule: append to X then write Y (remove-then-reassign
/// shape) over the two concrete file paths, window 2.
fn r3_config(action: RuleAction, path_a: &str, path_b: &str) -> SequencePolicyConfig {
    SequencePolicyConfig {
        fail_closed: true,
        rules: vec![SequenceRule {
            id: "r3-remove-then-reassign".to_string(),
            name: "n".to_string(),
            description: "d".to_string(),
            steps: vec![
                StepPredicate {
                    tool: "file_append".to_string(),
                    params: vec![ParamPredicate {
                        pointer: "/path".to_string(),
                        kind: ParamMatchKind::Exact,
                        value: path_a.to_string(),
                    }],
                },
                StepPredicate {
                    tool: "file_write".to_string(),
                    params: vec![ParamPredicate {
                        pointer: "/path".to_string(),
                        kind: ParamMatchKind::Exact,
                        value: path_b.to_string(),
                    }],
                },
            ],
            window: Some(2),
            action,
        }],
    }
}

fn r3_executor(action: RuleAction, path_a: &str, path_b: &str) -> ParallelExecutionServiceImpl {
    let svc = SequencePolicyServiceImpl::new(Box::new(R3PolicyRepo {
        config: Some(r3_config(action, path_a, path_b)),
    }));
    create_executor().with_sequence_policy(Arc::new(svc))
}

/// Unique per-run temp paths (parallel-safe; leftover files from crashed
/// runs are removed so spy assertions observe only this run's side effects).
fn r3_paths(tag: &str) -> (String, String) {
    let dir = std::env::temp_dir();
    let run = Uuid::new_v4();
    let paths = (
        dir.join(format!("rigorix-r3-{tag}-{run}-a.tmp"))
            .display()
            .to_string(),
        dir.join(format!("rigorix-r3-{tag}-{run}-b.tmp"))
            .display()
            .to_string(),
    );
    let _ = std::fs::remove_file(&paths.0);
    let _ = std::fs::remove_file(&paths.1);
    paths
}

/// Sequential graph: step_a (file_append) → step_b (file_write). NEITHER node
/// declares `requires_approval` — any pause is caused by the R3 gate alone.
fn r3_chain_graph(
    path_a: &str,
    path_b: &str,
) -> (crate::dag_engine::domain::TaskGraph, Uuid, Uuid) {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    let a_id = Uuid::new_v4();
    let b_id = Uuid::new_v4();
    let a = TaskNode::new(
        a_id,
        "step_a",
        "file_append",
        vec![],
        format!(r#"{{"path": "{}", "content": "a"}}"#, path_a),
    );
    let b = TaskNode::new(
        b_id,
        "step_b",
        "file_write",
        vec![a_id],
        format!(r#"{{"path": "{}", "content": "b"}}"#, path_b),
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(a).unwrap();
    graph.add_unchecked(b).unwrap();
    graph.seal().unwrap();
    (graph, a_id, b_id)
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
