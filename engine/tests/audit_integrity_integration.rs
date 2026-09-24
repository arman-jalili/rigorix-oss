//! ADR-016 Phase A integration — local chain, verify-on-read, mode tagging.
//!
//! Exercises the REAL composition pieces (`AuditServiceImpl` + the local
//! repository + `EnvelopeHistoryAdapter`) rather than hand-built envelopes:
//! envelopes are built and persisted through the audit service (so they carry
//! the chain link and the `local_unanchored` tag), then read back through the
//! cross-run guard.

use std::sync::Arc;

use rigorix_engine::audit::application::audit_queue_impl::AuditQueueImpl;
use rigorix_engine::audit::application::audit_sender_impl::AuditSenderImpl;
use rigorix_engine::audit::application::audit_service_impl::AuditServiceImpl;
use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::application::service::AuditService;
use rigorix_engine::audit::domain::envelope::{EventStatus, ExecutionEventRef, HistoryIntegrity};
use rigorix_engine::audit::infrastructure::LocalAuditEnvelopeRepository;
use rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository;
use rigorix_engine::sequence_policy::infrastructure::{EnvelopeHistoryAdapter, ExecutionHistory};

const KEY: &str = "phase-a-test-key";

fn input(node: &str) -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id: uuid::Uuid::new_v4(),
        template_id: "attendance".to_string(),
        planning_prompt: "run".to_string(),
        events: vec![ExecutionEventRef {
            event_type: "node_completed".to_string(),
            summary: format!("{node} completed"),
            occurred_at: chrono::Utc::now(),
            correlation_id: None,
            status: EventStatus::Success,
            payload: Some(serde_json::json!({ "step_name": node })),
        }],
        source: Some("rigorix_mcp".to_string()),
        repository: None,
        author: Some("jeff@corp".to_string()),
        identity: None,
        effect_key: None,
        producer_id: None,
        sequence: None,
        prev_hash: None,
        history_policy: None,
        total_tokens: 0,
        duration_ms: 10,
        git_commit: None,
        git_branch: None,
        model_version: None,
        planning_prompt_content: None,
        file_paths: vec![],
        metadata: None,
        scoring_results: std::collections::HashMap::new(),
        sign: true,
    }
}

/// A real audit service over a local store, building signed chained envelopes.
fn audit_service(dir: &std::path::Path) -> AuditServiceImpl {
    let repo: Arc<dyn AuditEnvelopeRepository> =
        Arc::new(LocalAuditEnvelopeRepository::new(dir.to_path_buf()));
    AuditServiceImpl::new(
        Box::new(AuditEnvelopeFactoryImpl::new(Some(KEY.to_string()))),
        Arc::new(AuditSenderImpl::new(None, None)),
        Box::new(AuditQueueImpl::default()),
        false, // no remote backend — local trail only
    )
    .with_local_repository(repo)
    .with_producer_id("test-producer")
}

async fn build_n(service: &AuditServiceImpl, nodes: &[&str]) {
    for node in nodes {
        service
            .build_and_send(input(node))
            .await
            .expect("build + local persist");
    }
}

fn guard(repo: Arc<dyn AuditEnvelopeRepository>) -> EnvelopeHistoryAdapter {
    EnvelopeHistoryAdapter::new(repo).with_verifier(Arc::new(AuditEnvelopeFactoryImpl::new(Some(
        KEY.to_string(),
    ))))
}

fn since() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now() - chrono::Duration::hours(1)
}

/// AC #3: envelopes built through the real service carry the chain link and
/// the `local_unanchored` tag.
#[tokio::test]
async fn built_envelopes_carry_chain_and_integrity_tag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo: Arc<dyn AuditEnvelopeRepository> =
        Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));
    let service = audit_service(dir.path());
    build_n(&service, &["a", "b", "c"]).await;

    let envelopes = repo.list(None, None, Some(u32::MAX)).await.expect("list");
    assert_eq!(envelopes.len(), 3);
    // `list` is newest-first; order by sequence.
    let mut chained: Vec<_> = envelopes
        .iter()
        .filter_map(|e| e.sequence.map(|s| (s, e)))
        .collect();
    chained.sort_by_key(|(s, _)| *s);
    for (index, (sequence, envelope)) in chained.iter().enumerate() {
        assert_eq!(*sequence, index as u64);
        assert_eq!(envelope.producer_id.as_deref(), Some("test-producer"));
        assert_eq!(
            envelope.history_integrity,
            Some(HistoryIntegrity::LocalUnanchored)
        );
        if index == 0 {
            assert!(envelope.prev_hash.is_none(), "genesis has no predecessor");
        } else {
            assert!(
                envelope.prev_hash.is_some(),
                "successor links to predecessor"
            );
        }
    }
}

/// AC #1: a tampered envelope is rejected on read (verify-on-read).
#[tokio::test]
async fn tampered_envelope_is_rejected_on_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo: Arc<dyn AuditEnvelopeRepository> =
        Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));
    let service = audit_service(dir.path());
    build_n(&service, &["remove_attendance"]).await;

    // Mutate the persisted envelope WITHOUT re-signing.
    let mut envelopes = repo.list(None, None, Some(u32::MAX)).await.expect("list");
    let mut tampered = envelopes.pop().expect("one envelope");
    tampered.template_id = "tampered".to_string();
    repo.save(&tampered).await.expect("save tampered");

    let err = guard(repo)
        .prior_actions(since())
        .await
        .expect_err("tampered envelope must fail closed");
    assert!(err.to_string().contains("signature"), "{err}");
}

/// AC #2: an interior deletion is detected by chain verification.
#[tokio::test]
async fn interior_deletion_is_detected_on_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo: Arc<dyn AuditEnvelopeRepository> =
        Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));
    let service = audit_service(dir.path());
    build_n(&service, &["a", "b", "c"]).await;

    // Delete the middle envelope (sequence 1).
    let envelopes = repo.list(None, None, Some(u32::MAX)).await.expect("list");
    let middle = envelopes
        .iter()
        .find(|e| e.sequence == Some(1))
        .expect("middle envelope");
    repo.delete(&middle.execution_id).await.expect("delete");

    let err = guard(repo)
        .prior_actions(since())
        .await
        .expect_err("interior gap must fail closed");
    assert!(err.to_string().contains("chain"), "{err}");
}

/// F-20260907-05 × ADR-016: one execution may emit twice — a pause-point
/// snapshot, then a re-dispatched FINAL envelope after approval (the MCP host
/// refreshes the local trail post-approval). The local store keeps ONE envelope
/// per `execution_id`, so the re-emission must REUSE the sequence/`prev_hash` of
/// the link it replaces: a fresh sequence would orphan the replaced link and
/// read back as an interior deletion.
#[tokio::test]
async fn reemission_for_same_execution_reuses_its_chain_link() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo: Arc<dyn AuditEnvelopeRepository> =
        Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));
    let service = audit_service(dir.path());

    // A prior execution occupies genesis (sequence 0).
    build_n(&service, &["a"]).await;

    // A second execution emits its pause-point snapshot (sequence 1).
    let paused = input("transfer_seat");
    let paused_id = paused.execution_id;
    service
        .build_and_send(paused)
        .await
        .expect("pause snapshot");

    // The SAME execution re-emits with the completed evidence.
    let mut completed = input("transfer_seat");
    completed.execution_id = paused_id;
    service
        .build_and_send(completed)
        .await
        .expect("completed re-emission");

    let envelopes = repo.list(None, None, Some(u32::MAX)).await.expect("list");
    assert_eq!(envelopes.len(), 2, "one envelope per execution_id");
    let reemitted = envelopes
        .iter()
        .find(|e| e.execution_id == paused_id)
        .expect("re-emitted envelope");
    assert_eq!(
        reemitted.sequence,
        Some(1),
        "re-emission reuses the replaced link's sequence"
    );
    // The chain still verifies — no hole where the replaced link was.
    guard(repo)
        .prior_actions(since())
        .await
        .expect("chain valid after re-emission");
}

/// AC #6: a legacy (unchained) store is grandfathered — no behavior change.
/// No verifier is wired for legacy data (there is nothing to verify).
#[tokio::test]
async fn legacy_unchained_store_is_readable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo: Arc<dyn AuditEnvelopeRepository> =
        Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));

    let factory = AuditEnvelopeFactoryImpl::default();
    let mut legacy_input = input("remove_attendance");
    legacy_input.sign = false;
    let mut legacy = factory.build_envelope(legacy_input).await.expect("build");
    legacy.producer_id = None;
    legacy.sequence = None;
    legacy.prev_hash = None;
    legacy.history_integrity = None;
    repo.save(&legacy).await.expect("save legacy");

    let actions = EnvelopeHistoryAdapter::new(repo)
        .prior_actions(since())
        .await
        .expect("legacy history reads (status quo)");
    assert!(
        actions.iter().any(|a| a.node == "remove_attendance"),
        "legacy action must surface: {actions:?}"
    );
}
