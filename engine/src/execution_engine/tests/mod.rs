//! ParallelExecutor implementation tests.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: ParallelExecutor — ParallelExecutionServiceImpl tests
//! Issue: issue-parallelexecutor
//!
//! Comprehensive tests for the ParallelExecutionServiceImpl and
//! RetryEvaluationServiceImpl implementations.

pub(crate) use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;

pub(crate) use async_trait::async_trait;

pub(crate) use std::sync::Arc;

pub(crate) use uuid::Uuid;

pub(crate) use crate::execution_engine::application::dto::{
    AbortExecutionInput, ApproveNodeInput, EvaluateRetryInput, ExecuteGraphInput, ExecuteNodeInput,
    GetExecutionStateInput, PauseExecutionInput, ResumeExecutionInput,
};

pub(crate) use crate::execution_engine::application::service::{
    ParallelExecutionService, RetryEvaluationService,
};

pub(crate) use crate::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};

pub(crate) use crate::execution_engine::domain::{
    BackoffStrategy, FailureContext, NodeExecutionState, ParallelExecutorConfig, RetryDecision,
    RetryPolicy, RetryStrategy,
};

pub(crate) use crate::failure_classification::application::failure_classifier_service_impl::FailureClassifierServiceImpl;

// ---------------------------------------------------------------------------
// Helper: create a configured service pair
// ---------------------------------------------------------------------------

pub(crate) use crate::sequence_policy::application::SequencePolicyServiceImpl;

pub(crate) use crate::sequence_policy::domain::{
    ParamMatchKind, ParamPredicate, RuleAction, SequencePolicyConfig, SequencePolicyError,
    SequenceRule, StepPredicate,
};

pub(crate) use crate::sequence_policy::infrastructure::SequencePolicyRepository;

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
        requirements: Vec::new(),
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
                        value: Some(path_a.to_string()),
                        step: None,
                    }],
                },
                StepPredicate {
                    tool: "file_write".to_string(),
                    params: vec![ParamPredicate {
                        pointer: "/path".to_string(),
                        kind: ParamMatchKind::Exact,
                        value: Some(path_b.to_string()),
                        step: None,
                    }],
                },
            ],
            window: Some(2),
            action,
            history: None,
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

mod approval;
mod dispatch;
mod execution;
mod hooks;
mod parallel;
mod retry;
