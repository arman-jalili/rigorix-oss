//! #1164: a `Dispatched` record with no live owner becomes `Ambiguous`.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentservice
//! Implements: ISSUE-1162 — startup sweep for orphaned in-flight records
//! Issue: #1164
//!
//! The crash window: `begin_effect` wrote the record, the process died before
//! the effect settled. The record is still `Dispatched`, `may_retry` does not
//! block it, and the DR plan's "re-query the ambiguous queue" cannot see it.
//! The startup sweep moves expired records to `Ambiguous`; it never claims the
//! effect did or did not happen.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{DEFAULT_DISPATCH_LEASE, EffectReconciler};
use rigorix_engine::effect_commitment::domain::{EffectIntent, EffectQuery, EffectState};
use rigorix_engine::effect_commitment::infrastructure::{
    HttpEffectLedger, HttpEffectLedgerConfig, InMemoryEffectLedger,
};
use rigorix_engine::effect_commitment::interfaces::EffectCommitmentLedger;
use uuid::Uuid;

fn intent(age: Duration) -> EffectIntent {
    let mut intent = EffectIntent::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        0,
        IdempotencyKey::new("placeholder"),
        "run_command",
        "args",
        "precondition",
        "authority",
    );
    intent.created_at = Utc::now() - chrono::Duration::from_std(age).unwrap();
    intent
}

#[tokio::test]
async fn an_expired_dispatched_record_becomes_ambiguous() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    assert!(
        ledger.can_sweep_dispatched(),
        "an in-process ledger has one clock; its owner must sweep"
    );
    let old = ledger
        .begin(intent(DEFAULT_DISPATCH_LEASE + Duration::from_secs(60)))
        .await
        .unwrap();
    assert_eq!(old.state(), EffectState::Dispatched);

    let reconciler = EffectReconciler::with_default_lease(ledger.clone());
    assert_eq!(reconciler.reconcile().await.unwrap(), 1);

    let swept = ledger.query(EffectQuery::by_id(old.id())).await.unwrap();
    assert_eq!(swept.len(), 1);
    assert_eq!(swept[0].state(), EffectState::Ambiguous);
    assert!(
        swept[0]
            .ambiguous_reason()
            .unwrap_or_default()
            .contains("deadline exceeded"),
        "the reason must say why the outcome is unknown: {:?}",
        swept[0].ambiguous_reason()
    );
    assert!(
        swept[0].outcome().is_none(),
        "the sweep never invents evidence"
    );
}

#[tokio::test]
async fn a_fresh_dispatched_record_is_untouched() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let fresh = ledger.begin(intent(Duration::from_secs(1))).await.unwrap();

    let reconciler = EffectReconciler::with_default_lease(ledger.clone());
    assert_eq!(reconciler.reconcile().await.unwrap(), 0);

    let still = ledger.query(EffectQuery::by_id(fresh.id())).await.unwrap();
    assert_eq!(still[0].state(), EffectState::Dispatched);
}

#[tokio::test]
async fn concurrent_roots_converge_on_one_ambiguous_transition() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let old = ledger
        .begin(intent(DEFAULT_DISPATCH_LEASE + Duration::from_secs(60)))
        .await
        .unwrap();

    let a = EffectReconciler::new(ledger.clone(), Duration::from_secs(60));
    let b = EffectReconciler::new(ledger.clone(), Duration::from_secs(60));
    let (swept_a, swept_b) = tokio::join!(a.reconcile(), b.reconcile());
    let total = swept_a.unwrap() + swept_b.unwrap();

    // The conditional transition lets exactly one caller claim the sweep; the
    // other observes Contended and treats it as success.
    assert_eq!(total, 1, "one root sweeps, the other converges");
    let records = ledger.query(EffectQuery::by_id(old.id())).await.unwrap();
    assert_eq!(records[0].state(), EffectState::Ambiguous);
}

#[tokio::test]
async fn a_terminal_record_is_never_swept() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let begun = ledger
        .begin(intent(DEFAULT_DISPATCH_LEASE + Duration::from_secs(60)))
        .await
        .unwrap();
    let evidence = rigorix_engine::effect_commitment::domain::ObservedOutcome::committed(
        "receipt-1",
        "readback-1",
        Utc::now(),
        "stripe",
    );
    ledger.settle_definite(begun.id(), evidence).await.unwrap();

    let reconciler = EffectReconciler::with_default_lease(ledger.clone());
    assert_eq!(reconciler.reconcile().await.unwrap(), 0);
    let records = ledger.query(EffectQuery::by_id(begun.id())).await.unwrap();
    assert_eq!(records[0].state(), EffectState::Committed);
}

#[tokio::test]
async fn a_shared_ledger_is_not_swept_by_a_client_root() {
    // #1173: the predicate mixes the writer's `started_at` with the sweeper's
    // clock. A client of a shared store must not sweep: the service owns both
    // the records and the clock.
    let ledger = Arc::new(HttpEffectLedger::new(HttpEffectLedgerConfig::new(
        "http://127.0.0.1:1",
        100,
        1024,
        None,
    )));
    assert!(
        !ledger.can_sweep_dispatched(),
        "a shared ledger must report that its owner, not this client, sweeps"
    );

    let reconciler = EffectReconciler::with_default_lease(ledger);
    assert!(!reconciler.can_sweep());
    assert_eq!(
        reconciler.reconcile().await.unwrap(),
        0,
        "a client must not issue the query, let alone transition a record"
    );
}
