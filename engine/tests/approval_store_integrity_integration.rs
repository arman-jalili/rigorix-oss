//! Approval store integrity (#1237).
//!
//! @canonical .pi/architecture/modules/approval.md#durability
//! Canonical Reference: .pi/architecture/modules/approval.md#durability
//!
//! The approval lifecycle (`status`, `expires_at`, `nonce`) is authority over
//! whether a dispatch may happen. A record whose fields no longer authenticate
//! under the run key is refused on load — never compared and matched.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rigorix_engine::approval::application::{
    ApprovalService, ApprovalServiceImpl, IntentVerification, NodeIntentResolver, ResolvedNode,
};
use rigorix_engine::approval::domain::{
    ApprovalError, ApprovalRecord, ApprovalStatus, DecisionContext, IntentHash,
};
use rigorix_engine::approval::infrastructure::repository::{
    ApprovalRepository, FileBackedApprovalRepository,
};
use uuid::Uuid;

const RUN_KEY: &[u8] = b"approval-store-integrity-key";

fn sample_record(node_id: Uuid) -> ApprovalRecord {
    let now = Utc::now();
    ApprovalRecord {
        step_name: "commit_cost".into(),
        node_id,
        intent_hash: IntentHash("00".repeat(32)),
        intent_payload: serde_json::json!({"tool": "run_command"}),
        approver_id: "tester@example".into(),
        authority: Some("approver".into()),
        decided_at: now,
        expires_at: now + ChronoDuration::hours(1),
        nonce: Uuid::new_v4(),
        token_claims_ref: None,
        status: ApprovalStatus::Pending,
        decision_context: DecisionContext {
            rendered_step: "run_command".into(),
            upstream_evidence: None,
            state_snapshot: None,
            summary: "run_command".into(),
            full_payload: None,
        },
        approver_scope: vec![],
        mac: None,
    }
}

async fn write_record(path: &std::path::Path, record: ApprovalRecord) {
    let repo =
        FileBackedApprovalRepository::open_with_key(path, Some(RUN_KEY)).expect("open keyed store");
    repo.save(&record).await.expect("save record");
}

#[tokio::test]
async fn an_untouched_record_verifies_and_loads() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("approvals.json");
    let node_id = Uuid::new_v4();
    write_record(&path, sample_record(node_id)).await;

    let repo = FileBackedApprovalRepository::open_with_key(&path, Some(RUN_KEY)).expect("reopen");
    let loaded = repo
        .load(node_id)
        .await
        .expect("verified load")
        .expect("record");
    assert!(loaded.verify_integrity(RUN_KEY));
    assert_eq!(repo.verify_integrity().await.expect("verify all"), 1);
}

#[tokio::test]
async fn a_consumed_record_edited_back_to_pending_is_refused() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("approvals.json");
    let node_id = Uuid::new_v4();
    let mut record = sample_record(node_id);
    record.status = ApprovalStatus::Consumed;
    write_record(&path, record).await;

    // An agent with workspace write access edits the plaintext lifecycle back.
    let raw = std::fs::read_to_string(&path).expect("read store");
    let edited = raw.replace("\"consumed\"", "\"pending\"");
    assert_ne!(raw, edited, "seed must actually change the status");
    std::fs::write(&path, edited).expect("write tampered store");

    let repo = FileBackedApprovalRepository::open_with_key(&path, Some(RUN_KEY)).expect("reopen");
    let err = repo
        .load(node_id)
        .await
        .expect_err("tampered record refused");
    assert!(matches!(
        err,
        ApprovalError::RecordIntegrity {
            node_id: failed,
            ref step_name,
        } if failed == node_id && step_name == "commit_cost"
    ));
    assert!(err.to_string().contains("commit_cost"));
}

#[tokio::test]
async fn an_extended_expiry_is_refused() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("approvals.json");
    let node_id = Uuid::new_v4();
    let record = sample_record(node_id);
    let original_expiry = record.expires_at;
    write_record(&path, record).await;

    let raw = std::fs::read_to_string(&path).expect("read store");
    let mut json: serde_json::Value = serde_json::from_str(&raw).expect("store json");
    let record = json
        .get_mut(node_id.to_string())
        .expect("record by node id");
    record["expires_at"] =
        serde_json::Value::String((original_expiry + ChronoDuration::days(1)).to_rfc3339());
    let edited = serde_json::to_string_pretty(&json).expect("serialize tampered store");
    assert_ne!(raw, edited, "seed must actually change the expiry");
    std::fs::write(&path, edited).expect("write tampered store");

    let repo = FileBackedApprovalRepository::open_with_key(&path, Some(RUN_KEY)).expect("reopen");
    assert!(
        repo.load(node_id).await.is_err(),
        "extended expiry must not authenticate"
    );
}

struct NoIntent;
#[async_trait::async_trait]
impl NodeIntentResolver for NoIntent {
    async fn resolve_by_step_name(&self, _: &str) -> Option<ResolvedNode> {
        None
    }
    async fn resolve_by_node_id(&self, _: Uuid) -> Option<ResolvedNode> {
        None
    }
}

#[tokio::test]
async fn verify_intent_refuses_a_tampered_record_with_a_diagnosis() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("approvals.json");
    let node_id = Uuid::new_v4();
    let mut record = sample_record(node_id);
    record.status = ApprovalStatus::Consumed;
    write_record(&path, record).await;

    let raw = std::fs::read_to_string(&path).expect("read store");
    std::fs::write(&path, raw.replace("\"consumed\"", "\"pending\"")).expect("tamper");

    let repository = Arc::new(
        FileBackedApprovalRepository::open_with_key(&path, Some(RUN_KEY)).expect("reopen"),
    );
    let service = ApprovalServiceImpl::new(
        repository,
        Arc::new(NoIntent),
        RUN_KEY.to_vec(),
        Duration::from_secs(3600),
    );

    match service
        .verify_intent(node_id)
        .await
        .expect("refusal is a verdict")
    {
        IntentVerification::IntegrityFailure {
            node_id: failed,
            step_name,
        } => {
            assert_eq!(failed, node_id);
            assert_eq!(step_name, "commit_cost");
        }
        other => panic!("tampered record must not verify: {other:?}"),
    }
}
