//! One settlement per run, and the ambiguity that survives a restart (#1528).
//!
//! The crash case is modelled the way the issue demands: **re-open the store over the same
//! directory**, not kill a process — `rigorix-engine` is a library with no `[[bin]]`, so there is
//! nothing in this crate to kill, and a test that pretended otherwise would be asserting on timing
//! rather than on state. A real process kill belongs to a host-level test in `cli`/`mcp`.
//!
//! Gate: `effect_commitment` is engine-only, so this module carries the same
//! `#[cfg(not(feature = "portable"))]` gate as its siblings.

use std::sync::Arc;

use rigorix_engine::effect_commitment::application::settlement_service::{
    SettlementGuard, SettlementService, SettlementStore,
};
use rigorix_engine::effect_commitment::domain::settlement::{
    AmbiguousSettlement, ExecutionSettlement, SettlementState, SettlementTerminal,
};
use rigorix_engine::state_persistence::infrastructure::filesystem_settlement_store::FileSystemSettlementStore;
use uuid::Uuid;

fn tempdir(tag: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!(
        "rigorix-settlement-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("a temporary directory");
    base
}

async fn store_at(dir: &std::path::Path) -> Arc<dyn SettlementStore> {
    Arc::new(
        FileSystemSettlementStore::new(dir.to_path_buf())
            .await
            .expect("store"),
    )
}

fn execution() -> Uuid {
    Uuid::from_u128(0x1528_0000_0000_0000_0000_0000_0000_0001)
}

const RECEIPT: &str = "receipt-digest-1528";

#[tokio::test]
async fn execution_settlement_a_clean_run_completes_with_both_effects_definite() {
    let dir = tempdir("clean");
    let service = SettlementService::new(store_at(&dir).await);
    let mut guard = service
        .open(execution(), Some(RECEIPT.to_string()), "t0".to_string())
        .await
        .expect("opens");
    guard
        .record_effect("record-a", SettlementState::Committed)
        .await
        .expect("records");
    guard
        .record_effect("record-b", SettlementState::NotCommitted)
        .await
        .expect("records");
    guard
        .close_completed("t1".to_string())
        .await
        .expect("closes");

    let read_back = service.settlement_for(execution()).await.expect("reads");
    assert_eq!(read_back.terminal, Some(SettlementTerminal::Completed));
    assert_eq!(read_back.terminal_as_read(), SettlementTerminal::Completed);
    assert_eq!(read_back.effects.len(), 2);
    assert!(read_back.orphaned().is_empty());
    assert_eq!(read_back.finished_at.as_deref(), Some("t1"));
}

#[tokio::test]
async fn execution_settlement_a_run_that_stopped_mid_dispatch_reads_as_ambiguous() {
    // The guard is dropped without closing — the shape of a panic inside the dispatch future, which
    // no `Drop` can write out of. A reopened store must read it as `Ambiguous`, not as running.
    let dir = tempdir("crash");
    {
        let service = SettlementService::new(store_at(&dir).await);
        let mut guard = service
            .open(execution(), Some(RECEIPT.to_string()), "t0".to_string())
            .await
            .expect("opens");
        guard
            .record_effect("record-a", SettlementState::Dispatched)
            .await
            .expect("records");
        // no close: the process that would have closed it did not come back.
        drop(guard);
    }

    // A *second* store over the same directory — the reopen, not a wish.
    let reopened = SettlementService::new(store_at(&dir).await);
    let settlement = reopened.settlement_for(execution()).await.expect("reads");
    assert_eq!(settlement.terminal, None, "nothing wrote a terminal");
    assert_eq!(settlement.terminal_as_read(), SettlementTerminal::Ambiguous);
    assert_eq!(settlement.orphaned(), vec!["record-a".to_string()]);

    // Removing the drop-guard behaviour — closing on drop — would make this assertion fail, because
    // the record would carry a terminal.
    let refusal = settlement
        .refuse_resume()
        .expect("a resume must be refused");
    assert_eq!(refusal.record_ids, vec!["record-a".to_string()]);
}

#[tokio::test]
async fn execution_settlement_a_resume_over_an_ambiguity_is_refused_naming_the_records() {
    let dir = tempdir("resume");
    let service = SettlementService::new(store_at(&dir).await);
    {
        let mut guard = service
            .open(execution(), Some(RECEIPT.to_string()), "t0".to_string())
            .await
            .expect("opens");
        guard
            .record_effect("record-a", SettlementState::Ambiguous)
            .await
            .expect("records");
        drop(guard);
    }

    let refusal: AmbiguousSettlement = service
        .refuse_resume(RECEIPT)
        .await
        .expect_err("an ambiguous settlement refuses the resume");
    assert_eq!(refusal.execution_id, execution());
    assert_eq!(refusal.record_ids, vec!["record-a".to_string()]);
    // The message names the record, because a refusal that says "something is ambiguous" leaves the
    // operator to guess which effect they must resolve.
    let message = refusal.to_string();
    assert!(message.contains("record-a"), "{message}");
    assert!(
        message.contains(RECEIPT) || message.contains("ambiguous"),
        "{message}"
    );
}

#[tokio::test]
async fn execution_settlement_a_completed_run_does_not_refuse_its_resume() {
    let dir = tempdir("completed");
    let service = SettlementService::new(store_at(&dir).await);
    let mut guard = service
        .open(execution(), Some(RECEIPT.to_string()), "t0".to_string())
        .await
        .expect("opens");
    guard
        .record_effect("record-a", SettlementState::Committed)
        .await
        .expect("records");
    guard
        .close_completed("t1".to_string())
        .await
        .expect("closes");
    assert!(service.refuse_resume(RECEIPT).await.is_ok());
}

#[tokio::test]
async fn execution_settlement_ambiguities_are_scoped_to_their_receipt() {
    // Two receipts in one store: the query returns exactly the ambiguous settlements of the receipt
    // it was asked about, which is what makes it a query rather than a filter the caller re-does.
    let dir = tempdir("scoped");
    let service = SettlementService::new(store_at(&dir).await);
    let other_execution = Uuid::from_u128(0x1528_0000_0000_0000_0000_0000_0000_0002);
    for (execution_id, receipt, record) in [
        (execution(), RECEIPT, "record-a"),
        (other_execution, "another-receipt", "record-z"),
    ] {
        let mut guard = service
            .open(execution_id, Some(receipt.to_string()), "t0".to_string())
            .await
            .expect("opens");
        guard
            .record_effect(record, SettlementState::Dispatched)
            .await
            .expect("records");
        drop(guard);
    }

    let mine = service.ambiguities_for(RECEIPT).await.expect("reads");
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!(mine[0].execution_id, execution());
    assert_eq!(mine[0].orphaned(), vec!["record-a".to_string()]);

    let theirs = service
        .ambiguities_for("another-receipt")
        .await
        .expect("reads");
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0].orphaned(), vec!["record-z".to_string()]);

    // And a receipt nobody has run has no ambiguities — the store is not a global yes.
    assert!(
        service
            .ambiguities_for("never-run")
            .await
            .expect("reads")
            .is_empty()
    );
}

#[tokio::test]
async fn execution_settlement_an_aborted_run_is_not_ambiguous_when_its_effects_are_definite() {
    let dir = tempdir("aborted");
    let service = SettlementService::new(store_at(&dir).await);
    let mut guard = service
        .open(execution(), Some(RECEIPT.to_string()), "t0".to_string())
        .await
        .expect("opens");
    guard
        .record_effect("record-a", SettlementState::NotCommitted)
        .await
        .expect("records");
    guard
        .close_aborted("the operator cancelled", "t1".to_string())
        .await
        .expect("closes");
    let settlement = service.settlement_for(execution()).await.expect("reads");
    match settlement.terminal_as_read() {
        SettlementTerminal::Aborted { reason } => assert_eq!(reason, "the operator cancelled"),
        other => panic!("expected Aborted, got {other:?}"),
    }
    assert!(settlement.refuse_resume().is_none());
}

#[tokio::test]
async fn execution_settlement_a_close_that_would_lie_about_an_effect_is_ambiguous_instead() {
    // `close_completed` on a run with a dispatched effect cannot say "completed": the settlement
    // computes its own terminal, so the caller's optimism is recorded as ambiguity.
    let dir = tempdir("lying");
    let service = SettlementService::new(store_at(&dir).await);
    let mut guard = service
        .open(execution(), Some(RECEIPT.to_string()), "t0".to_string())
        .await
        .expect("opens");
    guard
        .record_effect("record-a", SettlementState::Dispatched)
        .await
        .expect("records");
    guard
        .close_completed("t1".to_string())
        .await
        .expect("closes");
    let settlement = service.settlement_for(execution()).await.expect("reads");
    assert_eq!(settlement.terminal_as_read(), SettlementTerminal::Ambiguous);
}

#[tokio::test]
async fn execution_settlement_a_settlement_with_no_effects_at_all_is_still_open_to_reopen() {
    // A run that died before its first `begin` has no orphan to name — and no terminal either. It
    // reads as Ambiguous and refuses nothing, because there is no record whose fate is unknown: the
    // absence of a close is real, but it is not evidence about an effect.
    let dir = tempdir("empty");
    {
        let service = SettlementService::new(store_at(&dir).await);
        let guard = service
            .open(execution(), Some(RECEIPT.to_string()), "t0".to_string())
            .await
            .expect("opens");
        drop(guard);
    }
    let reopened = SettlementService::new(store_at(&dir).await);
    let settlement = reopened.settlement_for(execution()).await.expect("reads");
    assert_eq!(settlement.terminal_as_read(), SettlementTerminal::Ambiguous);
    assert!(settlement.orphaned().is_empty());
    assert!(settlement.refuse_resume().is_none());
}

#[test]
fn execution_settlement_the_guard_type_cannot_be_built_without_opening() {
    // The write-ahead rule is a type, not a convention: `SettlementGuard` has no constructor outside
    // `SettlementService::open`, so a caller cannot record an effect before the settlement exists.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/effect_commitment/application/settlement_service.rs"),
    )
    .expect("the service is readable");
    let guard_section = source
        .split("pub struct SettlementGuard")
        .nth(1)
        .expect("the guard is declared");
    assert!(
        !guard_section.contains("pub fn new("),
        "SettlementGuard must not be constructible apart from `open`"
    );
}

#[test]
fn execution_settlement_the_aggregate_carries_no_effect_field_of_its_own() {
    // ADR-019's per-effect truth stays in `EffectRecord`; the settlement references record ids. A
    // settlement that copied effect state would be a second answer to "was this committed?".
    let settlement = ExecutionSettlement::open(execution(), Some(RECEIPT.to_string()), "t0".into());
    assert!(settlement.effects.is_empty());
    let mut settlement = settlement;
    settlement.record_effect("record-a", SettlementState::Committed);
    settlement.record_effect("record-a", SettlementState::Ambiguous);
    assert_eq!(settlement.effects.len(), 1, "one record, one statement");
    assert_eq!(settlement.effects[0].state, SettlementState::Ambiguous);
}

/// A guard kept in scope on purpose, so the drop path is exercised by the compiler rather than by a
/// comment.
#[allow(dead_code)]
fn keep(_guard: SettlementGuard) {}
