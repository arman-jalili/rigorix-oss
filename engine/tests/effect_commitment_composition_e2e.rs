//! EffectCommitment composition e2e: declaration -> seam -> dispatch -> record.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#compositionroots
//! Implements: ISSUE-EFFECT-COMMITMENT-20 — upstream/downstream integration
//! Issue: #1133
//! ADR-019: the context is reachable only if a composition root constructs it

use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::effect_commitment::application::{EffectCommitmentSetup, QueryEffectInput};
use rigorix_engine::effect_commitment::domain::EffectQuery;
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use rigorix_engine::execution_engine::domain::ParallelExecutorConfig;
use std::sync::Arc;
use uuid::Uuid;

fn repo_with(declaration: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rigorix-effect-e2e-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(dir.join(".rigorix")).unwrap();
    std::fs::write(dir.join(".rigorix/effect-commitment.toml"), declaration).unwrap();
    dir
}

fn one_node_graph() -> TaskGraph {
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "run_command",
        vec![],
        r#"{"command": "echo hi"}"#,
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node).unwrap();
    graph.seal().unwrap();
    graph
}

fn executor_with(
    seam: Arc<dyn rigorix_engine::effect_commitment::DispatchSeam>,
) -> ParallelExecutionServiceImpl {
    ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig::default(),
        Box::new(RetryEvaluationServiceImpl::new()),
        Arc::new(EventBusServiceImpl::default()),
    )
    .with_effect_commitment_seam(seam)
}

#[tokio::test]
async fn a_declared_in_memory_seam_is_armed_and_records_a_dispatch() {
    let repo = repo_with("[backend]\nkind = \"in_memory\"\n");
    let setup = EffectCommitmentSetup::from_env(&repo)
        .await
        .expect("valid declaration")
        .expect("armed seam");
    assert!(!setup.can_prove_commitment());

    let executor = executor_with(setup.dispatch_seam());
    let dag_id = Uuid::new_v4();
    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(one_node_graph()),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(output.result.completed_count, 1);

    // The record is reachable through the same setup root that built the seam.
    let records = setup
        .service
        .query(QueryEffectInput {
            query: EffectQuery::by_dag(dag_id),
        })
        .await
        .unwrap()
        .records;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].state().as_str(), "committed");
    assert!(records[0].outcome().is_some());
}

#[tokio::test]
async fn a_declared_receipt_reader_is_constructed_and_lifts_the_claim_ceiling() {
    let repo = repo_with(
        r#"
[backend]
kind = "in_memory"

[receipt_reader]
base_url = "https://payments.example"
provider_id = "stripe"
"#,
    );
    let setup = EffectCommitmentSetup::from_env(&repo)
        .await
        .expect("valid declaration")
        .expect("armed seam");

    // The optional L3 adapter is present; its absence is the only honest
    // not-settled ceiling.
    assert!(setup.can_prove_commitment());
    assert!(setup.receipt_reader.is_some());
}

#[tokio::test]
async fn an_absent_declaration_arms_nothing_and_a_malformed_one_fails_closed() {
    let empty = std::env::temp_dir().join(format!(
        "rigorix-effect-e2e-absent-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&empty).unwrap();
    assert!(
        EffectCommitmentSetup::from_env(&empty)
            .await
            .expect("absent is not an error")
            .is_none()
    );

    let malformed = repo_with("not = = toml");
    assert!(EffectCommitmentSetup::from_env(&malformed).await.is_err());
}
