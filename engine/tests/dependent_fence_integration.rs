//! DependentFence integration: an ambiguous upstream keeps dependents fenced.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#dependentfence
//! Implements: ISSUE-EFFECT-COMMITMENT-17 — upstream/downstream integration
//! Issue: #1130
//! ADR-019: F1 — no dependent is released while an upstream effect is Ambiguous

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::effect_commitment::application::{
    DispatchObservation, DispatchSeam, EffectCommitmentSeam, EffectCommitmentServiceImpl,
    Sha256IntentKeyDerivation,
};
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId,
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

/// A seam that always reports the result lost, so every effect settles
/// `Ambiguous` regardless of what the tool actually returned.
#[derive(Default)]
struct AlwaysAmbiguousSeam {
    records: Mutex<HashMap<EffectRecordId, EffectRecord>>,
}

#[async_trait]
impl DispatchSeam for AlwaysAmbiguousSeam {
    async fn before_dispatch(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        let record = EffectRecord::begin(intent);
        self.records
            .lock()
            .unwrap()
            .insert(record.id(), record.clone());
        Ok(record)
    }

    async fn after_dispatch(
        &self,
        record_id: EffectRecordId,
        _observation: DispatchObservation,
    ) -> Result<EffectRecord, CommitmentError> {
        let mut records = self.records.lock().unwrap();
        let record =
            records
                .get(&record_id)
                .cloned()
                .ok_or_else(|| CommitmentError::Unavailable {
                    detail: "record missing".to_string(),
                })?;
        let ambiguous = record.mark_ambiguous("simulated lost response", Utc::now())?;
        records.insert(record_id, ambiguous.clone());
        Ok(ambiguous)
    }
}

fn two_node_graph() -> (Uuid, Uuid, TaskGraph) {
    let root_id = Uuid::new_v4();
    let dependent_id = Uuid::new_v4();
    let root = TaskNode::new(
        root_id,
        "root",
        "run_command",
        vec![],
        r#"{"command": "echo root"}"#,
    );
    let dependent = TaskNode::new(
        dependent_id,
        "dependent",
        "run_command",
        vec![root_id],
        r#"{"command": "echo dependent"}"#,
    );

    let mut graph = TaskGraph::new();
    graph.add_unchecked(root).unwrap();
    graph.add_unchecked(dependent).unwrap();
    graph.seal().unwrap();
    (root_id, dependent_id, graph)
}

fn executor_with(seam: Arc<dyn DispatchSeam>) -> ParallelExecutionServiceImpl {
    ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig::default(),
        Box::new(RetryEvaluationServiceImpl::new()),
        Arc::new(EventBusServiceImpl::default()),
    )
    .with_effect_commitment_seam(seam)
}

#[tokio::test]
async fn an_ambiguous_upstream_does_not_release_its_dependent() {
    let executor = executor_with(Arc::new(AlwaysAmbiguousSeam::default()));
    let (root_id, dependent_id, graph) = two_node_graph();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.completed_count, 1);
    assert_eq!(output.result.failed_count, 0);
    assert_eq!(output.result.skipped_count, 0);
    assert_eq!(output.result.total_nodes, 2);

    // The upstream ran; the dependent was never dispatched.
    assert!(output.result.node_results.contains_key(&root_id));
    assert!(!output.result.node_results.contains_key(&dependent_id));
}

#[tokio::test]
async fn a_settled_upstream_releases_its_dependent() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let service = EffectCommitmentServiceImpl::new(
        ledger.clone(),
        Arc::new(Sha256IntentKeyDerivation::new()),
    );
    let executor = executor_with(Arc::new(EffectCommitmentSeam::new(Arc::new(service))));
    let (root_id, dependent_id, graph) = two_node_graph();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.completed_count, 2);
    assert_eq!(output.result.failed_count, 0);
    assert!(output.result.node_results.contains_key(&root_id));
    assert!(output.result.node_results.contains_key(&dependent_id));

    // Both effects are recorded and terminal.
    let records = ledger.query(EffectQuery::by_dag(dag_id)).await.unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record.is_terminal()));
}
