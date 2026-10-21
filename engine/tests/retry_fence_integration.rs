//! RetryFence integration: an unsettled intent is never re-dispatched.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#retryfence
//! Implements: ISSUE-EFFECT-COMMITMENT-18 — upstream/downstream integration
//! Issue: #1131
//! ADR-019: F2 — a retry before resolution is refused

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::effect_commitment::application::{DispatchObservation, DispatchSeam};
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectRecord, ObservedOutcome,
};
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use rigorix_engine::execution_engine::domain::ParallelExecutorConfig;
use uuid::Uuid;

/// A seam whose write-ahead read returns an already-ambiguous record, as it
/// would after a previous attempt lost its response.
struct PreAmbiguousSeam;

#[async_trait]
impl DispatchSeam for PreAmbiguousSeam {
    async fn before_dispatch(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        EffectRecord::begin(intent).mark_ambiguous("previous attempt lost", Utc::now())
    }

    async fn after_dispatch(
        &self,
        _record_id: rigorix_engine::effect_commitment::domain::EffectRecordId,
        _observation: DispatchObservation,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(CommitmentError::Unavailable {
            detail: "must not be called".to_string(),
        })
    }
}

/// A seam whose write-ahead read returns a terminal record.
struct PreCommittedSeam;

#[async_trait]
impl DispatchSeam for PreCommittedSeam {
    async fn before_dispatch(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        EffectRecord::begin(intent).settle_definite(
            ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe"),
            Utc::now(),
        )
    }

    async fn after_dispatch(
        &self,
        _record_id: rigorix_engine::effect_commitment::domain::EffectRecordId,
        _observation: DispatchObservation,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(CommitmentError::Unavailable {
            detail: "must not be called".to_string(),
        })
    }
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

fn executor_with(seam: Arc<dyn DispatchSeam>) -> ParallelExecutionServiceImpl {
    ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig::default(),
        Box::new(RetryEvaluationServiceImpl::new()),
        Arc::new(EventBusServiceImpl::default()),
    )
    .with_effect_commitment_seam(seam)
}

async fn execute_and_expect_error(seam: Arc<dyn DispatchSeam>) -> String {
    let executor = executor_with(seam);
    let error = executor
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(one_node_graph()),
            config_override: None,
        })
        .await
        .unwrap_err();
    error.to_string()
}

#[tokio::test]
async fn an_ambiguous_effect_is_not_re_dispatched() {
    let message = execute_and_expect_error(Arc::new(PreAmbiguousSeam)).await;
    assert!(
        message.contains("retry refused") && message.contains("Ambiguous"),
        "an ambiguous write-ahead read must refuse the retry, got: {message}"
    );
}

#[tokio::test]
async fn a_terminal_effect_is_not_re_dispatched() {
    let message = execute_and_expect_error(Arc::new(PreCommittedSeam)).await;
    assert!(
        message.contains("already settled"),
        "a terminal write-ahead read must refuse the re-dispatch, got: {message}"
    );
}
