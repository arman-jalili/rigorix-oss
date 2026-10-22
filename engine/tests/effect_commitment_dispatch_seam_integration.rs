//! DispatchSeam integration: the execution engine writes ahead and settles.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#dispatchseam
//! Implements: ISSUE-EFFECT-COMMITMENT-16 — upstream/downstream integration
//! Issue: #1129
//! ADR-019: a failed begin refuses the dispatch

use std::sync::Arc;

use async_trait::async_trait;
use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::effect_commitment::application::{
    EffectCommitmentSeam, EffectCommitmentServiceImpl, Sha256IntentKeyDerivation,
};
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity,
};
use rigorix_engine::effect_commitment::infrastructure::InMemoryEffectLedger;
use rigorix_engine::effect_commitment::interfaces::EffectCommitmentLedger;
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use rigorix_engine::execution_engine::domain::ParallelExecutorConfig;
use uuid::Uuid;

fn graph(tool: &str, intent: &str) -> TaskGraph {
    let node = TaskNode::new(Uuid::new_v4(), "step", tool, vec![], intent);
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

fn in_memory_seam() -> (
    Arc<InMemoryEffectLedger>,
    Arc<dyn rigorix_engine::effect_commitment::DispatchSeam>,
) {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let service = EffectCommitmentServiceImpl::new(
        ledger.clone(),
        Arc::new(Sha256IntentKeyDerivation::new()),
    );
    (
        ledger,
        Arc::new(EffectCommitmentSeam::new(Arc::new(service))),
    )
}

#[tokio::test]
async fn a_successful_dispatch_is_recorded_before_and_settled_after() {
    let (ledger, seam) = in_memory_seam();
    let executor = executor_with(seam);
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph("run_command", r#"{"command": "echo hi"}"#)),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(output.result.completed_count, 1);

    let records = ledger.query(EffectQuery::by_dag(dag_id)).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].state().as_str(), "committed");
    assert!(records[0].outcome().is_some());
    assert!(!records[0].blocks_retry());
}

#[tokio::test]
async fn a_definite_failure_is_recorded_as_not_committed() {
    let (ledger, seam) = in_memory_seam();
    let executor = executor_with(seam);
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph("no_such_tool", "boom")),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(output.result.failed_count, 1);

    let records = ledger.query(EffectQuery::by_dag(dag_id)).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].state().as_str(), "not_committed");
}

#[tokio::test]
async fn a_failed_write_ahead_refuses_the_dispatch() {
    struct UnavailableLedger;

    #[async_trait]
    impl EffectCommitmentLedger for UnavailableLedger {
        fn can_sweep_dispatched(&self) -> bool {
            true
        }

        async fn begin(&self, _intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
            Err(CommitmentError::Unavailable {
                detail: "store down".to_string(),
            })
        }
        async fn settle_definite(
            &self,
            _record_id: EffectRecordId,
            _outcome: ObservedOutcome,
        ) -> Result<EffectRecord, CommitmentError> {
            Err(CommitmentError::Unavailable {
                detail: "store down".to_string(),
            })
        }
        async fn mark_ambiguous(
            &self,
            _record_id: EffectRecordId,
            _reason: String,
        ) -> Result<EffectRecord, CommitmentError> {
            Err(CommitmentError::Unavailable {
                detail: "store down".to_string(),
            })
        }
        async fn resolve(
            &self,
            _record_id: EffectRecordId,
            _outcome: ObservedOutcome,
            _resolver: ResolverIdentity,
        ) -> Result<EffectRecord, CommitmentError> {
            Err(CommitmentError::Unavailable {
                detail: "store down".to_string(),
            })
        }
        async fn query(&self, _query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError> {
            Err(CommitmentError::Unavailable {
                detail: "store down".to_string(),
            })
        }
    }

    let service = EffectCommitmentServiceImpl::new(
        Arc::new(UnavailableLedger),
        Arc::new(Sha256IntentKeyDerivation::new()),
    );
    let executor = executor_with(Arc::new(EffectCommitmentSeam::new(Arc::new(service))));

    let error = executor
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(graph("run_command", r#"{"command": "echo hi"}"#)),
            config_override: None,
        })
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("refusing to dispatch"),
        "the failed begin must refuse the dispatch, got: {message}"
    );
}
