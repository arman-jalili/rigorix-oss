//! EffectRecord integration: persisted aggregate ↔ query surface.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectrecord
//! Implements: ISSUE-EFFECT-COMMITMENT-3 — upstream/downstream integration
//! Issue: #1116
//! ADR-019: the aggregate and its read side agree

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, EffectState,
    ObservedOutcome, ResolverIdentity, ResolverKind,
};
use uuid::Uuid;

fn intent_for(dag: u128) -> EffectIntent {
    EffectIntent::new(
        Uuid::from_u128(dag),
        Uuid::from_u128(dag + 100),
        1,
        IdempotencyKey::new(format!("dag-{dag}")),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    )
}

#[test]
fn a_persisted_ambiguous_record_hydrates_and_still_fences() {
    let original = EffectRecord::begin(intent_for(1))
        .mark_ambiguous("response lost", Utc::now())
        .unwrap();

    let hydrated = EffectRecord::hydrate(
        original.id(),
        original.intent().clone(),
        original.state(),
        original.outcome().cloned(),
        original.resolver().cloned(),
        original.ambiguous_reason().map(str::to_string),
        original.updated_at(),
    )
    .expect("a valid persisted record hydrates");

    assert_eq!(hydrated, original);
    assert!(hydrated.blocks_dependents());
    assert!(hydrated.blocks_retry());
    assert!(hydrated.is_ambiguous());
}

#[test]
fn a_terminal_record_cannot_be_hydrated_without_evidence() {
    let err = EffectRecord::hydrate(
        EffectRecordId::new(),
        intent_for(1),
        EffectState::Committed,
        None,
        None,
        None,
        Utc::now(),
    )
    .unwrap_err();

    assert!(matches!(err, CommitmentError::EvidenceMissing { .. }));
}

#[test]
fn the_query_surface_reads_by_id_by_dag_and_ambiguity() {
    let dispatched = EffectRecord::begin(intent_for(1));
    let ambiguous = EffectRecord::begin(intent_for(1))
        .mark_ambiguous("timeout", Utc::now())
        .unwrap();
    let other_dag = EffectRecord::begin(intent_for(2))
        .mark_ambiguous("lost", Utc::now())
        .unwrap();

    let records = [dispatched.clone(), ambiguous.clone(), other_dag.clone()];

    let by_id: Vec<_> = records
        .iter()
        .filter(|r| r.matches_query(&EffectQuery::by_id(ambiguous.id())))
        .collect();
    assert_eq!(by_id, vec![&ambiguous]);

    let by_dag: Vec<_> = records
        .iter()
        .filter(|r| r.matches_query(&EffectQuery::by_dag(Uuid::from_u128(1))))
        .collect();
    assert_eq!(by_dag, vec![&dispatched, &ambiguous]);

    let all_ambiguous: Vec<_> = records
        .iter()
        .filter(|r| r.matches_query(&EffectQuery::ambiguous(None)))
        .collect();
    assert_eq!(all_ambiguous, vec![&ambiguous, &other_dag]);

    let dag_two_ambiguous: Vec<_> = records
        .iter()
        .filter(|r| r.matches_query(&EffectQuery::ambiguous(Some(Uuid::from_u128(2)))))
        .collect();
    assert_eq!(dag_two_ambiguous, vec![&other_dag]);
}

#[test]
fn resolving_an_ambiguous_record_records_who_and_on_what_evidence() {
    let record = EffectRecord::begin(intent_for(1))
        .mark_ambiguous("lost", Utc::now())
        .unwrap();
    let evidence = ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe");
    let resolver = ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1");

    let resolved = record
        .resolve(evidence.clone(), resolver.clone(), Utc::now())
        .unwrap();

    assert_eq!(resolved.state(), EffectState::Committed);
    assert_eq!(resolved.outcome(), Some(&evidence));
    assert_eq!(resolved.resolver(), Some(&resolver));
    assert!(resolved.ambiguous_reason().is_none());
    // A resolved record is no longer in the operator's ambiguous queue.
    assert!(!resolved.matches_query(&EffectQuery::ambiguous(None)));
}
