//! Approval store anti-rollback (#1241).
//!
//! @canonical .pi/architecture/modules/approval.md#durability
//! Canonical Reference: .pi/architecture/modules/approval.md#durability
//!
//! The MAC proves a record was written by the run key. The consumption ledger
//! proves the *file* is not an older authentic copy: terminal transitions are
//! recorded outside the store, and a pending record whose nonce is already
//! recorded terminal is refused.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use hmac::{Hmac, KeyInit, Mac};
use rigorix_engine::approval::application::{
    ApprovalService, ApprovalServiceImpl, IntentVerification, NodeIntentResolver, ResolvedNode,
};
use rigorix_engine::approval::domain::{
    ApprovalError, ApprovalRecord, ApprovalStatus, ConsumptionSettlement, DecisionContext,
    IntentHash,
};
use rigorix_engine::approval::infrastructure::repository::{
    ApprovalRepository, FileBackedApprovalRepository, LedgerMigration,
};
use serde::Serialize;
use sha2::Sha256;
use uuid::Uuid;

const RUN_KEY: &[u8] = b"approval-rollback-test-key";

fn sample_record(node_id: Uuid) -> ApprovalRecord {
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

/// The pre-#1268 on-disk marker: four fields, declaration order.
#[derive(Serialize)]
struct LegacyMarker {
    node_id: Uuid,
    nonce: Uuid,
    status: ApprovalStatus,
    recorded_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct LegacyLedger {
    version: u8,
    entries: BTreeMap<String, LegacyMarker>,
    mac: String,
}

/// Write a ledger exactly as a release before #1268 did: markers without
/// `dag_id`/`attempt`, MAC over `(1u8, entries)`. Built from local structs so
/// the fixture does not depend on today's serializer.
fn write_legacy_ledger(path: &Path, markers: Vec<LegacyMarker>) {
    let entries: BTreeMap<_, _> = markers
        .into_iter()
        .map(|marker| (format!("{}:{}", marker.node_id, marker.nonce), marker))
        .collect();
    let bytes = serde_json::to_vec(&(1u8, &entries)).expect("legacy MAC material");
    let mut mac = Hmac::<Sha256>::new_from_slice(RUN_KEY).expect("key");
    mac.update(&bytes);
    let file = LegacyLedger {
        version: 1,
        entries,
        mac: hex::encode(mac.finalize().into_bytes()),
    };
    std::fs::write(
        path,
        serde_json::to_string_pretty(&file).expect("legacy ledger"),
    )
    .expect("write legacy ledger");
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
async fn restoring_an_authentic_older_store_is_refused() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let pending = sample_record(node_id);
    repo.save(&pending).await.expect("save pending");

    // Snapshot while the record is authentic and pending.
    let snapshot = std::fs::read(&store).expect("snapshot");

    // Consume it: the store and the separate ledger both advance.
    let mut consumed = pending.clone();
    consumed.status = ApprovalStatus::Consumed;
    repo.save(&consumed).await.expect("save consumed");
    assert!(ledger.is_file(), "terminal save records consumption");

    // Restore only the store — every MAC in it is still valid.
    std::fs::write(&store, &snapshot).expect("restore snapshot");
    let restored = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("reopen");

    let error = restored
        .load(node_id)
        .await
        .expect_err("a regressed store must be refused");
    assert!(matches!(
        error,
        ApprovalError::RecordRollback {
            node_id: failed,
            ref step_name,
        } if failed == node_id && step_name == "commit_cost"
    ));
    assert!(error.to_string().contains("commit_cost"));

    // The same refusal reaches the dispatch decision, not a Matched verdict.
    let service = ApprovalServiceImpl::new(
        Arc::new(restored),
        Arc::new(NoIntent),
        RUN_KEY.to_vec(),
        Duration::from_secs(3600),
    );
    match service.verify_intent(node_id).await.expect("verdict") {
        IntentVerification::IntegrityFailure {
            node_id: failed,
            step_name,
        } => {
            assert_eq!(failed, node_id);
            assert_eq!(step_name, "commit_cost");
        }
        other => panic!("restored store must not verify: {other:?}"),
    }
}

#[tokio::test]
async fn a_fresh_store_without_a_marker_still_works() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    repo.save(&sample_record(node_id)).await.expect("save");
    let loaded = repo.load(node_id).await.expect("fresh state works");
    assert!(loaded.is_some());
    assert!(
        !ledger.exists(),
        "nothing terminal has happened, so no marker is needed"
    );
}

#[tokio::test]
async fn a_current_store_still_dispatches() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let pending = sample_record(node_id);
    repo.save(&pending).await.expect("save");

    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("reopen");
    let loaded = reopened
        .load(node_id)
        .await
        .expect("current state loads")
        .expect("record");
    assert_eq!(loaded.status, ApprovalStatus::Pending);
    assert_eq!(loaded.nonce, pending.nonce);
}

#[tokio::test]
async fn deleting_one_marker_from_the_ledger_is_an_integrity_failure() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let pending = sample_record(node_id);
    repo.save(&pending).await.expect("save pending");
    let mut consumed = pending.clone();
    consumed.status = ApprovalStatus::Consumed;
    repo.save(&consumed).await.expect("save consumed");

    // Delete one marker from the unsigned-looking file. The whole-set MAC is
    // what makes this an edit rather than an absence (#1244).
    let mut raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ledger).expect("read ledger"))
            .expect("ledger json");
    let entries = raw["entries"].as_object_mut().expect("entries");
    assert_eq!(entries.len(), 1);
    entries.clear();
    std::fs::write(
        &ledger,
        serde_json::to_string_pretty(&raw).expect("serialize"),
    )
    .expect("truncate ledger");

    let error = match FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    ) {
        Ok(_) => panic!("a shortened ledger must not open"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        ApprovalError::ConsumptionLedgerIntegrity { .. }
    ));
    let text = error.to_string();
    assert!(text.contains("integrity"), "{text}");
    assert!(text.contains("approvals.json"), "{text}");
    assert!(text.contains("consumed.json"), "{text}");
}

#[tokio::test]
async fn a_live_ledger_verifies_and_reports_its_marker_count() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let mut record = sample_record(node_id);
    record.status = ApprovalStatus::Consumed;
    repo.save(&record).await.expect("save consumed");

    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("live ledger verifies");
    assert_eq!(reopened.consumption_marker_count().await.expect("count"), 1);
    assert_eq!(reopened.verify_integrity().await.expect("records"), 1);
}

#[tokio::test]
async fn a_stripped_wrapper_is_refused_by_name_not_adopted() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    std::fs::write(&ledger, "{}").expect("write stripped ledger");

    let error = match FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    ) {
        Ok(_) => panic!("a ledger without a wrapper must not be adopted"),
        Err(error) => error,
    };
    let text = error.to_string();
    assert!(text.contains("unauthenticated ledger"), "{text}");
    assert!(
        text.contains("rigorix approve ledger migrate"),
        "the refusal must name the migration command: {text}"
    );
    assert!(text.contains("consumed.json"), "{text}");
}

#[tokio::test]
async fn explicit_migration_adopts_a_bare_map_and_reports_its_count() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();
    let nonce = Uuid::new_v4();
    let legacy = serde_json::json!({
        format!("{node_id}:{nonce}"): {
            "node_id": node_id,
            "nonce": nonce,
            "status": "consumed",
            "recorded_at": "2026-10-31T00:00:00Z"
        }
    });
    std::fs::write(&ledger, serde_json::to_string(&legacy).unwrap()).expect("legacy ledger");

    let outcome =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, false)
            .expect("migrate");
    assert_eq!(
        outcome,
        LedgerMigration::Migrated {
            markers: 1,
            terminal_records: 0,
            missing: vec![],
            unmatched_markers: 1,
            store_present: false,
            store_path: store.clone(),
        }
    );

    let sealed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    assert_eq!(sealed["version"], 1);
    assert!(sealed["mac"].as_str().is_some_and(|mac| !mac.is_empty()));
    assert!(
        sealed["entries"]
            .get(format!("{node_id}:{nonce}"))
            .is_some()
    );

    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("the migrated ledger verifies");
    assert_eq!(reopened.consumption_marker_count().await.unwrap(), 1);

    let again =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, false)
            .expect("idempotent");
    assert_eq!(
        again,
        LedgerMigration::AlreadySealed {
            markers: 1,
            terminal_records: 0,
            missing: vec![],
            unmatched_markers: 1,
            store_present: false,
            store_path: store.clone(),
        }
    );
}

/// #1249: the count is only a check if it is compared to the store.
#[tokio::test]
async fn migration_refuses_a_ledger_that_misses_a_terminal_store_record() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let mut consumed = sample_record(node_id);
    consumed.status = ApprovalStatus::Consumed;
    repo.save(&consumed).await.expect("save consumed");
    drop(repo);

    // A stripped or truncated export: one terminal record in the store, no
    // marker in the legacy map.
    std::fs::write(&ledger, "{}").expect("write stripped legacy map");

    let error = match FileBackedApprovalRepository::migrate_consumption_ledger(
        &store, &ledger, RUN_KEY, false,
    ) {
        Ok(_) => panic!("a truncated legacy map must not be adopted silently"),
        Err(error) => error,
    };
    let text = error.to_string();
    assert!(matches!(error, ApprovalError::MigrationIncomplete { .. }));
    assert!(text.contains(&node_id.to_string()), "{text}");
    assert!(text.contains("commit_cost"), "{text}");
    assert!(text.contains("refuses to seal"), "{text}");

    let outcome =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, true)
            .expect("--accept-missing");
    assert_eq!(
        outcome,
        LedgerMigration::Migrated {
            markers: 1,
            terminal_records: 1,
            missing: vec![(node_id, "commit_cost".into())],
            unmatched_markers: 0,
            store_present: true,
            store_path: store.clone(),
        }
    );
    let sealed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    assert!(sealed["mac"].as_str().is_some_and(|mac| !mac.is_empty()));
    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("accepted terminal records are adopted into the ledger");
    assert!(reopened.load(node_id).await.expect("load").is_some());
}

/// #1253: markers naming no store record are a population, not a fraction.
#[tokio::test]
async fn migration_reports_markers_that_name_no_store_record() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let mut consumed = sample_record(node_id);
    consumed.status = ApprovalStatus::Consumed;
    repo.save(&consumed).await.expect("save consumed");
    drop(repo);

    let sealed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    let mut legacy = sealed["entries"].as_object().cloned().expect("entries");
    let foreign = Uuid::new_v4();
    let foreign_nonce = Uuid::new_v4();
    legacy.insert(
        format!("{foreign}:{foreign_nonce}"),
        serde_json::json!({
            "node_id": foreign,
            "nonce": foreign_nonce,
            "status": "consumed",
            "recorded_at": "2026-10-31T00:00:00Z",
        }),
    );
    std::fs::write(&ledger, serde_json::to_string(&legacy).unwrap()).expect("legacy export");

    let outcome =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, false)
            .expect("migrate");
    match outcome {
        LedgerMigration::Migrated {
            markers,
            terminal_records,
            unmatched_markers,
            missing,
            ..
        } => {
            assert_eq!(markers, 2);
            assert_eq!(terminal_records, 1);
            assert_eq!(unmatched_markers, 1, "the foreign marker has no record");
            assert!(missing.is_empty(), "the store record is covered");
        }
        other => panic!("expected a migration, got {other:?}"),
    }
}

/// #1256: absence and emptiness are no-ops, not integrity failures.
#[tokio::test]
async fn migration_of_an_absent_or_empty_ledger_is_a_no_op() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");

    let absent =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, false)
            .expect("absent ledger");
    assert_eq!(
        absent,
        LedgerMigration::NothingToMigrate {
            ledger_path: ledger.clone(),
            empty: false,
        }
    );
    assert!(!ledger.exists(), "a no-op must not create the ledger");

    std::fs::write(&ledger, "").expect("empty ledger");
    let empty =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, false)
            .expect("empty ledger");
    assert_eq!(
        empty,
        LedgerMigration::NothingToMigrate {
            ledger_path: ledger.clone(),
            empty: true,
        }
    );
    assert_eq!(
        std::fs::read_to_string(&ledger).unwrap(),
        "",
        "a no-op must not rewrite the empty file"
    );
}

/// #1255: a terminal record whose marker went missing must not load silently.
#[tokio::test]
async fn a_terminal_record_without_a_marker_is_refused_on_load() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let mut consumed = sample_record(node_id);
    consumed.status = ApprovalStatus::Consumed;
    repo.save(&consumed).await.expect("save consumed");
    drop(repo);

    // A valid sealed ledger with no markers, as a second writer dropping them
    // (#1254) or a marker-dropping bug would leave.
    let empty_store = dir.path().join("empty-store.json");
    std::fs::write(dir.path().join("empty.json"), "{}").expect("legacy empty map");
    FileBackedApprovalRepository::migrate_consumption_ledger(
        &empty_store,
        dir.path().join("empty.json"),
        RUN_KEY,
        false,
    )
    .expect("seal empty ledger");
    std::fs::copy(dir.path().join("empty.json"), &ledger).expect("drop the marker");

    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("ledger itself verifies");
    let error = reopened
        .load(node_id)
        .await
        .expect_err("a terminal record with no marker must be refused");
    let text = error.to_string();
    assert!(
        matches!(error, ApprovalError::UncoveredTerminalRecord { .. }),
        "{text}"
    );
    assert!(text.contains(&node_id.to_string()), "{text}");
    assert!(text.contains("commit_cost"), "{text}");
    assert!(text.contains("rigorix approve ledger migrate"), "{text}");
    assert!(text.contains("--accept-missing"), "{text}");

    let migrated =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, true)
            .expect("the documented override adopts the terminal record");
    let (markers, missing) = match migrated {
        LedgerMigration::Migrated {
            markers, missing, ..
        }
        | LedgerMigration::AlreadySealed {
            markers, missing, ..
        } => (markers, missing),
        other => panic!("expected an adopted ledger, got {other:?}"),
    };
    assert_eq!(markers, 1, "the terminal record was adopted");
    assert_eq!(missing.len(), 1, "and reported as previously uncovered");
    let accepted = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open after migration");
    assert!(accepted.load(node_id).await.expect("load").is_some());
}

#[tokio::test]
async fn migration_adopts_n_of_n_when_the_ledger_covers_every_terminal_record() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let mut consumed = sample_record(node_id);
    consumed.status = ApprovalStatus::Consumed;
    repo.save(&consumed).await.expect("save consumed");
    drop(repo);

    // Convert the sealed ledger back to the legacy bare-map shape, preserving
    // every marker.
    let sealed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    std::fs::write(&ledger, serde_json::to_string(&sealed["entries"]).unwrap())
        .expect("write legacy export");

    let outcome =
        FileBackedApprovalRepository::migrate_consumption_ledger(&store, &ledger, RUN_KEY, false)
            .expect("migrate");
    assert_eq!(
        outcome,
        LedgerMigration::Migrated {
            markers: 1,
            terminal_records: 1,
            missing: vec![],
            unmatched_markers: 0,
            store_present: true,
            store_path: store.clone(),
        }
    );
}

/// #1269: a ledger written before markers carried settlement still opens and
/// still refuses a second consumption. A non-skipped field or a version bump
/// would turn this into `ConsumptionLedgerIntegrity` at a customer's site.
#[tokio::test]
async fn a_pre_1268_ledger_still_verifies_and_refuses_consumption() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");
    let mut consumed = sample_record(node_id);
    consumed.status = ApprovalStatus::Consumed;
    repo.save(&consumed).await.expect("save consumed");
    drop(repo);

    // Replace the ledger with the exact pre-change shape.
    write_legacy_ledger(
        &ledger,
        vec![LegacyMarker {
            node_id,
            nonce: consumed.nonce,
            status: ApprovalStatus::Consumed,
            recorded_at: Utc::now(),
        }],
    );
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ledger).expect("read")).expect("json");
    let entry = &raw["entries"][format!("{node_id}:{}", consumed.nonce)];
    assert_eq!(
        entry.as_object().expect("marker").len(),
        4,
        "the fixture must stay a four-field marker: {entry}"
    );
    assert!(entry.get("dag_id").is_none());
    assert!(entry.get("attempt").is_none());
    assert_eq!(raw["version"], 1);

    let reopened = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("a pre-#1268 ledger still verifies");

    let loaded = reopened
        .load(node_id)
        .await
        .expect("legacy ledger loads")
        .expect("record");
    assert_eq!(loaded.status, ApprovalStatus::Consumed);
    assert_eq!(
        reopened
            .consumption_settlement(node_id, consumed.nonce)
            .await
            .expect("marker"),
        None,
        "a legacy marker carries no settlement"
    );

    let error = reopened
        .save(&consumed)
        .await
        .expect_err("re-consuming a legacy terminal record is refused");
    assert!(
        matches!(error, ApprovalError::ConsumptionReplay { .. }),
        "{error}"
    );
}

/// #1269: an old marker for the same node must not shadow the current
/// approval's marker when classification asks which settlement recorded it.
#[tokio::test]
async fn the_current_nonce_wins_over_an_older_marker_for_the_same_node() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = dir.path().join("approvals.json");
    let ledger = dir.path().join("consumed.json");
    let node_id = Uuid::new_v4();
    let old_nonce = Uuid::from_u128(1);
    let current_nonce = Uuid::from_u128(2);

    let repo = FileBackedApprovalRepository::open_with_key_and_ledger(
        &store,
        Some(RUN_KEY),
        Some(&ledger),
    )
    .expect("open");

    let old_settlement = ConsumptionSettlement {
        dag_id: Uuid::from_u128(10),
        node_id,
        attempt: 1,
    };
    let mut old = sample_record(node_id);
    old.nonce = old_nonce;
    old.status = ApprovalStatus::Consumed;

    let current_settlement = ConsumptionSettlement {
        dag_id: Uuid::from_u128(11),
        node_id,
        attempt: 1,
    };
    let mut current = sample_record(node_id);
    current.nonce = current_nonce;
    current.status = ApprovalStatus::Consumed;

    // The current approval is recorded first; the marker for the older nonce is
    // written *last*, so recency would answer with the wrong approval.
    repo.save_with_settlement(&current, current_settlement)
        .await
        .expect("consume the current approval");
    std::thread::sleep(Duration::from_millis(2));
    repo.save_with_settlement(&old, old_settlement)
        .await
        .expect("the old marker is written later");

    assert_eq!(
        repo.consumption_marker_count().await.expect("count"),
        2,
        "both markers are retained"
    );
    assert_eq!(
        repo.consumption_settlement(node_id, current_nonce)
            .await
            .expect("marker"),
        Some(current_settlement),
        "classification must use the current nonce, not the latest recorded_at"
    );
    assert_eq!(
        repo.consumption_settlement(node_id, Uuid::from_u128(99))
            .await
            .expect("missing nonce"),
        None,
        "a nonce the store no longer holds is fail-safe"
    );
}
