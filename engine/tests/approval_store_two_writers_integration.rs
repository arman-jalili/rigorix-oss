//! Two writers on one approval store and one consumption ledger (#1254).
//!
//! @canonical .pi/architecture/modules/approval.md#durability
//! Canonical Reference: .pi/architecture/modules/approval.md#durability
//!
//! The store and the ledger are whole-file rewrites. A writer that publishes its
//! cached snapshot erases another writer's record and marker. The guard is a
//! read-modify-write under an exclusive file lock, so the file is the source of
//! truth at write time.

use chrono::{Duration as ChronoDuration, Utc};
use rigorix_engine::approval::domain::{
    ApprovalRecord, ApprovalStatus, DecisionContext, IntentHash,
};
use rigorix_engine::approval::infrastructure::repository::{
    ApprovalRepository, FileBackedApprovalRepository,
};
use uuid::Uuid;

const RUN_KEY: &[u8] = b"approval-two-writer-test-key";

fn consumed(node_id: Uuid) -> ApprovalRecord {
    let now = Utc::now();
    ApprovalRecord {
        step_name: "commit_cost".into(),
        node_id,
        intent_hash: IntentHash("00".repeat(32)),
        intent_payload: serde_json::json!({"tool": "run_command"}),
        approver_id: "tester@example".into(),
        authority: None,
        decided_at: now,
        expires_at: now + ChronoDuration::hours(1),
        nonce: Uuid::new_v4(),
        token_claims_ref: None,
        status: ApprovalStatus::Consumed,
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

#[tokio::test]
async fn a_second_writer_keeps_the_first_records_and_markers() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");

    let a = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open A");
    let b = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open B");

    let record_a = consumed(Uuid::new_v4());
    let record_b = consumed(Uuid::new_v4());
    a.save(&record_a).await.expect("A saves");
    b.save(&record_b).await.expect("B saves");

    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("reopen");

    assert!(reopened.load(record_a.node_id).await.unwrap().is_some());
    assert!(reopened.load(record_b.node_id).await.unwrap().is_some());
    assert_eq!(
        reopened.consumption_marker_count().await.unwrap(),
        2,
        "both terminal transitions must stay remembered"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_writers_do_not_lose_a_record_or_marker() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");

    let a = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open A");
    let b = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open B");
    let record_a = consumed(Uuid::new_v4());
    let record_b = consumed(Uuid::new_v4());

    let (saved_a, saved_b) = tokio::join!(a.save(&record_a), b.save(&record_b));
    saved_a.expect("A save");
    saved_b.expect("B save");

    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("reopen");
    assert!(reopened.load(record_a.node_id).await.unwrap().is_some());
    assert!(reopened.load(record_b.node_id).await.unwrap().is_some());
    assert_eq!(reopened.consumption_marker_count().await.unwrap(), 2);
}
