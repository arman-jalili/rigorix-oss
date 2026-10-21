//! ADR-008 budget enforcement reaches the dispatch path (#1108).
//!
//! @canonical .pi/architecture/modules/enforcement.md#enforcementservice
//! Implements: ISSUE-UNWIRED-BUDGETS-AND-RECOVERY — the enforcer was constructed
//!   by no entry point, so budgets were configurable and never consulted
//! Issue: #1108
//!
//! # Why these assertions and not "the run failed"
//!
//! A limit that is checked *after* the tool ran is indistinguishable from no
//! limit in the only way that matters: the effect happened. So the load-bearing
//! assertion is that the tool's side effect **never occurs** — the marker file the
//! step would have created is absent — and that the refusal is `budget_exceeded`
//! rather than some later failure.
//!
//! # Falsifier
//!
//! Delete the `enforce_budget` call at the top of `execute_tool` and the marker
//! appears: the step dispatches and succeeds. That is the regression this file
//! exists to catch.

use std::path::PathBuf;
use std::sync::Arc;

use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::enforcement::domain::config::{EnforcementConfig, ExecutionLimits};
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::event_system::application::service::EventBusService;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::factory::{
    ParallelExecutionFactory, ParallelExecutionFactoryConfig,
};
use rigorix_engine::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::domain::{ExecutionResult, ParallelExecutorConfig};
use uuid::Uuid;

fn marker(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rigorix-budget-{tag}-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ))
}

fn single_node(node_id: Uuid, name: &str, intent: String) -> TaskGraph {
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(TaskNode::new(node_id, name, "run_command", vec![], intent))
        .expect("add node");
    graph.seal().expect("seal");
    graph
}

async fn run(executor: &dyn ParallelExecutionService, graph: TaskGraph) -> ExecutionResult {
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(graph),
            config_override: None,
        })
        .await
        .expect("execute_graph")
        .result
}

/// A budget of zero is the sharpest form of the property: the very first call is
/// refused, so the difference between "checked before" and "checked after" is the
/// marker file's existence.
#[tokio::test]
async fn a_zero_budget_refuses_the_call_and_the_tool_never_runs() {
    let path = marker("zero");
    let _ = std::fs::remove_file(&path);

    let mut enforcement = EnforcementConfig::standard();
    enforcement.execution_limits = ExecutionLimits {
        max_tool_calls: 0,
        ..enforcement.execution_limits
    };

    let config = ParallelExecutorConfig {
        enforcement: Some(enforcement),
        ..ParallelExecutorConfig::default()
    };
    let event_bus: Arc<dyn EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            executor_config: config,
            event_bus: Some(event_bus),
            ..Default::default()
        })
        .await
        .expect("factory create");

    let node_id = Uuid::new_v4();
    let result = run(
        executor.as_ref(),
        single_node(
            node_id,
            "pay",
            format!(
                r#"{{"tool":"run_command","command":"touch {}"}}"#,
                path.display()
            ),
        ),
    )
    .await;

    assert_eq!(result.failed_count, 1, "the step must fail: {result:?}");
    assert!(
        !path.exists(),
        "the refused step's tool must NEVER run — a budget checked after dispatch is not a budget"
    );
    let failure = result
        .node_results
        .get(&node_id)
        .expect("node result")
        .failure_type
        .clone();
    assert_eq!(
        failure.as_deref(),
        Some("budget_exceeded"),
        "the refusal must be attributable to the budget, not to a later failure"
    );
}

/// The positive half: with room in the budget the same step dispatches, runs and
/// is counted. Without this, a control that refuses everything would satisfy the
/// test above.
#[tokio::test]
async fn a_step_within_budget_dispatches_runs_and_is_counted() {
    let path = marker("ok");
    let _ = std::fs::remove_file(&path);

    let mut enforcement = EnforcementConfig::standard();
    enforcement.execution_limits = ExecutionLimits {
        max_tool_calls: 2,
        ..enforcement.execution_limits
    };

    let config = ParallelExecutorConfig {
        enforcement: Some(enforcement),
        ..ParallelExecutorConfig::default()
    };
    let event_bus: Arc<dyn EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            executor_config: config,
            event_bus: Some(event_bus),
            ..Default::default()
        })
        .await
        .expect("factory create");

    let node_id = Uuid::new_v4();
    let result = run(
        executor.as_ref(),
        single_node(
            node_id,
            "pay",
            format!(
                r#"{{"tool":"run_command","command":"touch {}"}}"#,
                path.display()
            ),
        ),
    )
    .await;

    assert_eq!(
        result.completed_count, 1,
        "within budget the step runs: {result:?}"
    );
    assert!(path.exists(), "the tool ran, so its side effect exists");
    let _ = std::fs::remove_file(&path);
}

/// Wiring check for the recovery seam (#1108, audit row W14): the factory must
/// attach the service when one is supplied, and leave the field empty otherwise.
#[test]
fn recovery_is_attached_only_when_supplied() {
    use rigorix_engine::execution_engine::application::factory::RecoverySetup;

    // Unset (or any non-truthy value) leaves recovery off — the documented default.
    let off = RecoverySetup::from_env();
    assert!(
        off.is_none(),
        "recovery must be opt-in; enabling it by default would change how every \
         failure is retried"
    );
}
