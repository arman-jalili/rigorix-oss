//! A live approval repository re-reads the store and ledger (#1261).
//!
//! @canonical .pi/architecture/modules/approval.md#durability
//! Canonical Reference: .pi/architecture/modules/approval.md#durability
//!
//! Two MCP sessions hold two repositories open on one store. #1254 made writes
//! re-read and merge; reads still served the open-time snapshot, so a session
//! that had seen a pending record could consume an approval another session had
//! already consumed. Reads now take a shared file lock and re-read both files.

use chrono::{Duration as ChronoDuration, Utc};
use rigorix_engine::approval::domain::{
    ApprovalError, ApprovalRecord, ApprovalStatus, ConsumptionSettlement, DecisionContext,
    IntentHash,
};
use rigorix_engine::approval::infrastructure::repository::{
    ApprovalRepository, FileBackedApprovalRepository,
};
use uuid::Uuid;

const RUN_KEY: &[u8] = b"approval-live-read-test-key";

fn sample(node_id: Uuid) -> ApprovalRecord {
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

fn marker_timestamp(ledger: &std::path::Path, node: &ApprovalRecord) -> String {
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ledger).unwrap()).unwrap();
    value["entries"][format!("{}:{}", node.node_id, node.nonce)]["recorded_at"]
        .as_str()
        .expect("marker recorded_at")
        .to_string()
}

#[tokio::test]
async fn an_open_session_sees_records_and_refuses_a_second_consumption() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");

    // Both sessions open before the approval exists.
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

    let mut pending = sample(Uuid::new_v4());
    a.save(&pending).await.expect("A saves pending");
    let stale = b
        .load(pending.node_id)
        .await
        .expect("B re-reads the store")
        .expect("the record written after B opened is visible");
    assert_eq!(stale.status, ApprovalStatus::Pending);
    assert_eq!(b.verify_integrity().await.unwrap(), 1);

    pending.status = ApprovalStatus::Consumed;
    a.save(&pending).await.expect("A consumes");
    let live = b
        .load(pending.node_id)
        .await
        .expect("B re-reads")
        .expect("record");
    assert_eq!(
        live.status,
        ApprovalStatus::Consumed,
        "B must not keep serving its pending snapshot"
    );

    let first_timestamp = marker_timestamp(&ledger, &pending);
    let mut replay = stale;
    replay.status = ApprovalStatus::Consumed;
    let error = b
        .save(&replay)
        .await
        .expect_err("a second terminal write is a replay");
    let text = error.to_string();
    assert!(
        matches!(error, ApprovalError::ConsumptionReplay { .. }),
        "{text}"
    );
    assert!(text.contains("commit_cost"), "{text}");
    assert!(text.contains(&pending.node_id.to_string()), "{text}");
    assert!(text.contains("consumed.json"), "{text}");
    assert_eq!(
        marker_timestamp(&ledger, &pending),
        first_timestamp,
        "the replay must not rewrite the marker"
    );
}

/// #1264: a configured ledger that is gone must not read as "nothing spent".
/// #1267: the marker names the settlement, so classification needs no process
/// memory and survives a restart.
#[tokio::test]
async fn the_consumption_marker_records_the_settling_run_and_attempt() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");

    let mut record = sample(Uuid::new_v4());
    record.status = ApprovalStatus::Consumed;
    let settlement = ConsumptionSettlement {
        dag_id: Uuid::new_v4(),
        node_id: record.node_id,
        attempt: 2,
    };
    repo.save_with_settlement(&record, settlement)
        .await
        .expect("consume with attribution");
    assert_eq!(
        repo.consumption_settlement(record.node_id, record.nonce)
            .await
            .unwrap(),
        Some(settlement)
    );

    let replay = repo
        .save_with_settlement(
            &record,
            ConsumptionSettlement {
                attempt: 3,
                ..settlement
            },
        )
        .await
        .expect_err("a different settlement is a replay");
    assert!(matches!(replay, ApprovalError::ConsumptionReplay { .. }));
    assert_eq!(
        repo.consumption_settlement(record.node_id, record.nonce)
            .await
            .unwrap(),
        Some(settlement),
        "the first settlement stays attributed"
    );
}

#[tokio::test]
async fn a_terminal_record_without_its_ledger_is_refused_by_path_and_node() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let mut record = sample(Uuid::new_v4());
    record.status = ApprovalStatus::Consumed;
    repo.save(&record).await.expect("consume");
    std::fs::remove_file(&ledger).expect("the host lost its ledger");

    let error = repo
        .load(record.node_id)
        .await
        .expect_err("a terminal record cannot be checked without its ledger");
    let text = error.to_string();
    assert!(
        matches!(error, ApprovalError::ConsumptionLedgerMissing { .. }),
        "{text}"
    );
    assert!(text.contains(&ledger.display().to_string()), "{text}");
    assert!(text.contains(&record.node_id.to_string()), "{text}");
    assert!(text.contains("1 terminal record(s)"), "{text}");

    let verify = repo
        .verify_integrity()
        .await
        .expect_err("verify must refuse too");
    assert!(matches!(
        verify,
        ApprovalError::ConsumptionLedgerMissing {
            terminal_records: 1,
            ..
        }
    ));
}

/// #1264: absence with nothing terminal to check remains an allowed empty case.
#[tokio::test]
async fn no_ledger_and_no_terminal_records_is_allowed() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    assert_eq!(repo.verify_integrity().await.expect("empty store"), 0);
    let pending = sample(Uuid::new_v4());
    repo.save(&pending).await.expect("save pending");
    assert!(repo.load(pending.node_id).await.expect("load").is_some());
    assert_eq!(repo.verify_integrity().await.expect("only pending"), 1);
    assert!(
        !ledger.exists(),
        "a pending save does not create the ledger"
    );
}

#[tokio::test]
async fn a_restored_pending_record_is_the_rollback_refusal() {
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

    let mut record = sample(Uuid::new_v4());
    a.save(&record).await.expect("save pending");
    let snapshot = std::fs::read(&store).expect("snapshot");
    record.status = ApprovalStatus::Consumed;
    a.save(&record).await.expect("consume");

    // An authentic older store copy: the marker survives outside it.
    std::fs::write(&store, &snapshot).expect("restore pending store");
    let error = b
        .load(record.node_id)
        .await
        .expect_err("B re-reads and refuses the restored pending record");
    assert!(matches!(
        error,
        ApprovalError::RecordRollback {
            node_id,
            ref step_name,
        } if node_id == record.node_id && step_name == "commit_cost"
    ));
}

/// #1272: doctor needs the attributed/unattributed split, not a bare count.
#[tokio::test]
async fn markers_split_by_whether_they_name_a_settlement() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");

    let mut attributed = sample(Uuid::new_v4());
    attributed.status = ApprovalStatus::Consumed;
    repo.save_with_settlement(
        &attributed,
        ConsumptionSettlement {
            dag_id: Uuid::new_v4(),
            node_id: attributed.node_id,
            attempt: 1,
        },
    )
    .await
    .expect("attributed marker");

    let mut unattributed = sample(Uuid::new_v4());
    unattributed.status = ApprovalStatus::Consumed;
    repo.save(&unattributed).await.expect("legacy marker");

    let split = repo
        .consumption_marker_attribution()
        .await
        .expect("attribution");
    assert_eq!(split.settlements_known, 1);
    assert_eq!(split.settlements_unknown, 1);
    assert_eq!(repo.consumption_marker_count().await.unwrap(), 2);
}
