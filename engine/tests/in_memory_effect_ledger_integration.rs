//! InMemoryEffectLedger integration: the L1 seam end to end.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#inmemoryeffectledger
//! Implements: ISSUE-EFFECT-COMMITMENT-10 — upstream/downstream integration
//! Issue: #1123
//! ADR-019: write-ahead, custody, fences, and provider-only resolution

use std::sync::Arc;

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{
    BeginEffectInput, EffectCommitmentService, EffectCommitmentServiceImpl, MarkAmbiguousInput,
    QueryEffectInput, ResolveEffectInput, Sha256IntentKeyDerivation,
};
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, ObservedOutcome, ResolverIdentity, ResolverKind,
};
use rigorix_engine::effect_commitment::infrastructure::InMemoryEffectLedger;
use rigorix_engine::effect_commitment::interfaces::EffectCommitmentLedger;
use uuid::Uuid;

fn build_service(ledger: Arc<InMemoryEffectLedger>) -> EffectCommitmentServiceImpl {
    EffectCommitmentServiceImpl::new(ledger, Arc::new(Sha256IntentKeyDerivation::new()))
}

fn intent(attempt: u32, args: &str) -> EffectIntent {
    EffectIntent::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        attempt,
        IdempotencyKey::new(format!("attempt-{attempt}")),
        "payments.charge",
        args,
        "precondition-digest",
        "authority-digest",
    )
}

fn evidence() -> ObservedOutcome {
    ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe")
}

#[tokio::test]
async fn begin_is_write_ahead_and_custody_collapses_retries() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let service = build_service(ledger);

    let first = service
        .begin(BeginEffectInput {
            intent: intent(1, "args"),
        })
        .await
        .unwrap();
    assert_eq!(first.record.state().as_str(), "dispatched");

    let retry = service
        .begin(BeginEffectInput {
            intent: intent(2, "args"),
        })
        .await
        .unwrap();
    assert_eq!(first.record.id(), retry.record.id());
    assert_eq!(
        first.record.intent().idempotency_key,
        retry.record.intent().idempotency_key
    );

    // The record is durably visible in the ledger before any settle call.
    let by_id = service
        .query(QueryEffectInput {
            query: EffectQuery::by_id(first.record.id()),
        })
        .await
        .unwrap();
    assert_eq!(by_id.records, vec![first.record]);
}

#[tokio::test]
async fn a_changed_payload_is_a_different_intent() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let service = build_service(ledger);

    let first = service
        .begin(BeginEffectInput {
            intent: intent(1, "args"),
        })
        .await
        .unwrap();
    let changed = service
        .begin(BeginEffectInput {
            intent: intent(2, "different-args"),
        })
        .await
        .unwrap();

    assert_ne!(first.record.id(), changed.record.id());
}

#[tokio::test]
async fn a_lost_response_fences_the_effect_until_a_provider_resolves_it() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let service = build_service(ledger.clone());

    let begun = service
        .begin(BeginEffectInput {
            intent: intent(1, "args"),
        })
        .await
        .unwrap();
    let ambiguous = service
        .mark_ambiguous(MarkAmbiguousInput {
            record_id: begun.record.id(),
            reason: "response lost".to_string(),
        })
        .await
        .unwrap();
    assert!(ambiguous.record.blocks_retry());

    // The ambiguous queue is the operator surface.
    let pending = service
        .query(QueryEffectInput {
            query: EffectQuery::ambiguous(None),
        })
        .await
        .unwrap();
    assert_eq!(pending.records, vec![ambiguous.record.clone()]);

    // F4: the agent is rejected.
    assert!(matches!(
        service
            .resolve(ResolveEffectInput {
                record_id: begun.record.id(),
                outcome: evidence(),
                resolver: ResolverIdentity::new("agent", ResolverKind::Agent, "readback-1"),
            })
            .await,
        Err(CommitmentError::UnauthorizedResolver { .. })
    ));

    // F3/F4: provider evidence settles it and records who resolved it.
    let resolved = service
        .resolve(ResolveEffectInput {
            record_id: begun.record.id(),
            outcome: evidence(),
            resolver: ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1"),
        })
        .await
        .unwrap();
    assert_eq!(resolved.record.state().as_str(), "committed");
    assert_eq!(
        resolved.record.resolver().map(|r| r.resolver_id.as_str()),
        Some("stripe")
    );

    // L1: a new service over the same in-process ledger still sees the record.
    let restarted = build_service(ledger);
    let seen = restarted
        .query(QueryEffectInput {
            query: EffectQuery::by_id(begun.record.id()),
        })
        .await
        .unwrap();
    assert_eq!(seen.records, vec![resolved.record]);
}

#[tokio::test]
async fn a_settled_effect_cannot_be_resolved_or_settled_again() {
    let ledger: Arc<dyn EffectCommitmentLedger> = Arc::new(InMemoryEffectLedger::new());
    let begun = ledger.begin(intent(1, "args")).await.unwrap();
    let settled_evidence = evidence();

    ledger
        .settle_definite(begun.id(), settled_evidence.clone())
        .await
        .unwrap();

    assert!(matches!(
        ledger.settle_definite(begun.id(), settled_evidence).await,
        Err(CommitmentError::AlreadySettled { .. })
    ));
    assert!(matches!(
        ledger
            .resolve(
                begun.id(),
                evidence(),
                ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1"),
            )
            .await,
        Err(CommitmentError::NotAmbiguous { .. })
    ));
}
