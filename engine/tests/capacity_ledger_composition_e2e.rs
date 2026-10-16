//! ADR-018: end-to-end composition **reachability** test for the capacity seam.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#enforcementseam
//! Implements: ISSUE-CAPACITY-LEDGER-VERIFY — prove the seam is *reachable*, not
//!   merely *implemented*.
//!
//! Every prior test for this module exercised the port, the adapters or the
//! guard in isolation, which is why a fully green epic shipped a context that
//! **nothing called**: `CapacityLedgerFactory::create` had no caller and no
//! `enforcement`/`orchestrator`/`execution_engine` file mentioned the module.
//!
//! This test builds the executor the way production does —
//! `ParallelExecutionFactoryImpl::create` with a `CapacitySetup::from_env` built
//! from a real fixture file — and drives the real `run_dispatch_loop`, so:
//!
//! 1. a governed step over the cap is refused and **its tool is never called**
//!    (the marker file never appears);
//! 2. a governed step within the cap dispatches and its hold is **committed**
//!    (exposure reports it as committed, not outstanding);
//! 3. a failed step **releases** its hold rather than consuming it;
//! 4. an undeclared step is not governed (status quo);
//! 5. with no declaration file the run dispatches unchanged (fail-open-absent).
//!
//! # Falsifier
//!
//! Case 1 is the regression guard: remove the `capacity_verdict(...)` call from
//! `run_dispatch_loop` and the governed step is no longer refused, the marker
//! appears, and this test fails. A test that cannot fail proves nothing, so this
//! property was verified by temporarily deleting the call and observing the
//! failure before restoring it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rigorix_engine::capacity_ledger::CapacitySetup;
use rigorix_engine::capacity_ledger::application::dto::ExposureQuery;
use rigorix_engine::capacity_ledger::domain::ResourceId;
use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
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

const RESOURCE: &str = "reachability-budget";

/// A unique scratch repo root carrying a `.rigorix/capacity.toml` declaration.
///
/// `hard_limit` and the rule's `amount` are parameters so each case can choose
/// whether the single governed step fits.
fn scratch_with_declaration(tag: &str, hard_limit: u64, amount: u64) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "rigorix-capacity-e2e-{tag}-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let rigorix = dir.join(".rigorix");
    std::fs::create_dir_all(&rigorix).expect("create .rigorix");
    let config = format!(
        r#"
[[resources]]
id = "{RESOURCE}"
hard_limit = {hard_limit}
window_seconds = 3600

[[grants]]
step = "pay"
tool = "run_command"
resource = "{RESOURCE}"
amount = {amount}
lease_seconds = 60
"#
    );
    std::fs::write(rigorix.join("capacity.toml"), config).expect("write capacity.toml");
    dir
}

/// A scratch repo root with NO declaration at all.
fn scratch_without_declaration(tag: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "rigorix-capacity-e2e-{tag}-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create dir");
    dir
}

fn marker_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rigorix-capacity-marker-{tag}-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ))
}

/// Build the production-shaped executor and keep a handle to the armed seam so
/// the assertion can read exposure after the run.
async fn executor_with_declaration(
    repo_root: &Path,
) -> (Box<dyn ParallelExecutionService>, CapacitySetup) {
    let setup = CapacitySetup::from_env(repo_root)
        .await
        .expect("from_env")
        .expect("fixture present");
    let handle = setup.clone();
    let event_bus: Arc<dyn EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            executor_config: ParallelExecutorConfig::default(),
            event_bus: Some(event_bus),
            capacity: Some(setup),
            ..Default::default()
        })
        .await
        .expect("factory create");

    // The seam must be *reachable* from the built executor, not merely configured.
    (executor, handle)
}

fn single_node_graph(node_id: Uuid, name: &str, tool: &str, intent: String) -> TaskGraph {
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(TaskNode::new(node_id, name, tool, vec![], intent))
        .expect("add node");
    graph.seal().expect("seal");
    graph
}

async fn run(
    executor: &dyn ParallelExecutionService,
    dag_id: Uuid,
    graph: TaskGraph,
) -> ExecutionResult {
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .expect("execute_graph")
        .result
}

async fn exposure_committed(setup: &CapacitySetup) -> (u64, u64) {
    let out = setup
        .service
        .exposure(ExposureQuery {
            resource: ResourceId::new(RESOURCE),
        })
        .await
        .expect("exposure");
    (out.exposure.committed, out.exposure.outstanding)
}

// ---------------------------------------------------------------------------
// 1. Over the cap: refused, and the tool is never called.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn governed_step_over_the_cap_is_refused_and_its_tool_is_never_called() {
    // hard_limit 3 < amount 4, so the very first reserve must be refused. This is
    // deterministic: no concurrency or timing is involved.
    let repo = scratch_with_declaration("over", 3, 4);
    let marker = marker_path("over");
    let _ = std::fs::remove_file(&marker);

    let (executor, setup) = executor_with_declaration(&repo).await;
    assert!(setup.is_armed(), "the fixture must arm the seam");

    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let graph = single_node_graph(
        node_id,
        "pay",
        "run_command",
        format!("touch {}", marker.display()),
    );
    let result = run(executor.as_ref(), dag_id, graph).await;

    assert_eq!(
        result.failed_count, 1,
        "the governed step must be refused before dispatch"
    );
    assert!(
        !marker.exists(),
        "a refused step's tool must NEVER be called (marker must not appear)"
    );
    let node = result
        .node_results
        .get(&node_id)
        .expect("node result recorded");
    assert_eq!(
        node.failure_type.as_deref(),
        Some("capacity_denied"),
        "the refusal must be recorded with a deterministic capacity failure"
    );

    // Nothing was consumed: the refusal happened before any hold existed.
    let (committed, outstanding) = exposure_committed(&setup).await;
    assert_eq!((committed, outstanding), (0, 0));
}

// ---------------------------------------------------------------------------
// 2. Within the cap: dispatched, then committed.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn governed_step_within_the_cap_dispatches_and_commits_its_hold() {
    let repo = scratch_with_declaration("within", 10, 4);
    let marker = marker_path("within");
    let _ = std::fs::remove_file(&marker);

    let (executor, setup) = executor_with_declaration(&repo).await;

    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let graph = single_node_graph(
        node_id,
        "pay",
        "run_command",
        format!("touch {}", marker.display()),
    );
    let result = run(executor.as_ref(), dag_id, graph).await;

    assert_eq!(result.failed_count, 0, "the step fits and must dispatch");
    assert!(
        marker.exists(),
        "a permitted governed step must actually run its tool"
    );

    // The hold is settled as committed, not left outstanding until lease expiry.
    let (committed, outstanding) = exposure_committed(&setup).await;
    assert_eq!(
        committed, 4,
        "the granted capacity must be committed once the step succeeded"
    );
    assert_eq!(
        outstanding, 0,
        "no hold may be left outstanding after a terminal outcome"
    );
}

// ---------------------------------------------------------------------------
// 3. A failing step releases its hold rather than consuming it.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_failed_governed_step_releases_its_hold() {
    let repo = scratch_with_declaration("fail", 10, 4);
    let (executor, setup) = executor_with_declaration(&repo).await;

    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    // The tool runs and fails: the hold was taken, but must be handed back.
    let graph = single_node_graph(node_id, "pay", "run_command", "exit 1".to_string());
    let result = run(executor.as_ref(), dag_id, graph).await;

    assert_eq!(result.failed_count, 1, "the tool genuinely failed");
    let (committed, outstanding) = exposure_committed(&setup).await;
    assert_eq!(
        (committed, outstanding),
        (0, 0),
        "a failed step must release its hold, not consume or strand it"
    );
}

// ---------------------------------------------------------------------------
// 4. Undeclared steps are not governed.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_undeclared_step_is_not_governed_by_the_seam() {
    // The cap is exhausted for 'pay' by the rule, but this step has another name.
    let repo = scratch_with_declaration("undeclared", 1, 1);
    let marker = marker_path("undeclared");
    let _ = std::fs::remove_file(&marker);

    let (executor, _setup) = executor_with_declaration(&repo).await;

    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let graph = single_node_graph(
        node_id,
        "not_pay",
        "run_command",
        format!("touch {}", marker.display()),
    );
    let result = run(executor.as_ref(), dag_id, graph).await;

    assert_eq!(
        result.failed_count, 0,
        "a step no rule names must take the previous code path"
    );
    assert!(marker.exists(), "an ungoverned step must still run");
}

// ---------------------------------------------------------------------------
// 5. No declaration -> status quo (fail-open-absent).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn absent_declaration_dispatches_unchanged() {
    let repo = scratch_without_declaration("absent");
    let marker = marker_path("absent");
    let _ = std::fs::remove_file(&marker);

    let setup = CapacitySetup::from_env(&repo).await.expect("from_env");
    assert!(setup.is_none(), "no declaration must arm nothing");

    let event_bus: Arc<dyn EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            executor_config: ParallelExecutorConfig::default(),
            event_bus: Some(event_bus),
            capacity: None,
            ..Default::default()
        })
        .await
        .expect("factory create");

    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let graph = single_node_graph(
        node_id,
        "pay",
        "run_command",
        format!("touch {}", marker.display()),
    );
    let result = run(executor.as_ref(), dag_id, graph).await;

    assert_eq!(result.failed_count, 0);
    assert!(
        marker.exists(),
        "with no declaration the engine must behave exactly as before"
    );
}
