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
    graph_with_node(Uuid::new_v4(), tool, intent)
}

fn graph_with_node(node_id: Uuid, tool: &str, intent: &str) -> TaskGraph {
    let node = TaskNode::new(node_id, "step", tool, vec![], intent);
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

/// #1196: turning the seam on must not turn retries off. The first attempt ends
/// in a definite non-commitment; the same intent (same key) is dispatched again
/// and succeeds. The record model keeps both attempts.
#[tokio::test]
async fn a_retry_after_a_definite_failure_dispatches_a_new_attempt() {
    let (ledger, seam) = in_memory_seam();
    let executor = executor_with(seam.clone());
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let file = std::env::temp_dir().join(format!(
        "rigorix-retry-{}-{}.txt",
        std::process::id(),
        Uuid::new_v4()
    ));
    std::fs::write(&file, "WRONG").unwrap();
    // Fails once (the search anchor is absent), then succeeds on the same
    // intent after the file changes: the tool, params, and therefore the key are
    // identical across both attempts.
    let patch = format!(
        r#"{{"path": {path}, "search": "MARKER", "insert": "\npatched"}}"#,
        path = serde_json::to_string(file.to_str().unwrap()).unwrap()
    );

    let first = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph_with_node(node_id, "file_patch", &patch)),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(first.result.failed_count, 1);

    let after_failure = ledger.query(EffectQuery::by_dag(dag_id)).await.unwrap();
    assert_eq!(after_failure.len(), 1);
    assert_eq!(after_failure[0].state().as_str(), "not_committed");

    // The transient condition clears before the retry.
    std::fs::write(&file, "MARKER").unwrap();

    // A fresh executor: the first one retains the DAG as in-progress by design.
    let executor = executor_with(seam);
    let second = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph_with_node(node_id, "file_patch", &patch)),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(second.result.completed_count, 1);

    let history = ledger.query(EffectQuery::by_dag(dag_id)).await.unwrap();
    assert_eq!(history.len(), 2, "both attempts are recorded");
    assert_eq!(history[0].state().as_str(), "not_committed");
    assert_eq!(history[1].state().as_str(), "committed");
    assert_ne!(history[0].id(), history[1].id());
    assert_eq!(
        history[0].intent().idempotency_key,
        history[1].intent().idempotency_key,
        "the retry reuses the intent key"
    );
    let _ = std::fs::remove_file(&file);
}

/// #1196: only `NotCommitted` re-arms. A committed record is still refused,
/// and an ambiguous one is still refused by F2 — neither may dispatch again.
#[tokio::test]
async fn terminal_commitments_are_still_refused_by_the_write_ahead() {
    use chrono::Utc;
    use rigorix_engine::effect_commitment::domain::{EffectRecord, ResolverKind};
    use rigorix_engine::effect_commitment::{DispatchObservation, DispatchSeam};

    struct FixedSeam(EffectRecord);

    #[async_trait]
    impl DispatchSeam for FixedSeam {
        async fn before_dispatch(
            &self,
            _intent: EffectIntent,
        ) -> Result<EffectRecord, CommitmentError> {
            Ok(self.0.clone())
        }
        async fn after_dispatch(
            &self,
            _record_id: EffectRecordId,
            _observation: DispatchObservation,
        ) -> Result<EffectRecord, CommitmentError> {
            unreachable!("the dispatch is refused before this point")
        }
    }

    let base = EffectRecord::begin(EffectIntent::new(
        Uuid::from_u128(10),
        Uuid::from_u128(11),
        1,
        rigorix_engine::capacity_ledger::domain::IdempotencyKey::new("fixed-intent"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    ));

    let committed = base
        .clone()
        .settle_definite(
            ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe"),
            Utc::now(),
        )
        .unwrap();
    let error = executor_with(Arc::new(FixedSeam(committed)))
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(graph("run_command", r#"{"command": "echo hi"}"#)),
            config_override: None,
        })
        .await
        .expect_err("a committed intent must not dispatch");
    assert!(error.to_string().contains("already committed"), "{error}");

    let ambiguous = base
        .clone()
        .mark_ambiguous("response lost", Utc::now())
        .unwrap();
    let error = executor_with(Arc::new(FixedSeam(ambiguous)))
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(graph("run_command", r#"{"command": "echo hi"}"#)),
            config_override: None,
        })
        .await
        .expect_err("an ambiguous intent must not dispatch");
    assert!(error.to_string().contains("Ambiguous"), "{error}");

    // Keep the resolver helpers linked: `ResolverIdentity`/`ResolverKind` are
    // part of the retry surface, not a new API.
    let _ = ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1");
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
