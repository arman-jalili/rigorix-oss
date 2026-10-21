//! ObservedOutcome integration: provider evidence ↔ settlement.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#observedoutcome
//! Implements: ISSUE-EFFECT-COMMITMENT-4 — upstream/downstream integration
//! Issue: #1117
//! ADR-019: only provider evidence may settle an effect

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::ReconciliationPolicy;
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectRecord, ObservedOutcome, OutcomeVerdict, ResolverIdentity,
    ResolverKind,
};
use uuid::Uuid;

fn ambiguous_record() -> EffectRecord {
    EffectRecord::begin(EffectIntent::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        1,
        IdempotencyKey::new("intent-key"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    ))
    .mark_ambiguous("response lost", Utc::now())
    .unwrap()
}

#[test]
fn receipt_evidence_settles_a_dispatched_record() {
    let record = EffectRecord::begin(EffectIntent::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        1,
        IdempotencyKey::new("intent-key"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    ));

    let evidence = ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe");
    assert!(evidence.is_complete());
    let settled = record
        .settle_definite(evidence.clone(), Utc::now())
        .unwrap();

    assert_eq!(settled.outcome(), Some(&evidence));
    assert_eq!(settled.state().as_str(), "committed");
}

#[test]
fn not_committed_is_only_available_from_explicit_evidence() {
    let record = ambiguous_record();
    let evidence = ObservedOutcome::not_committed("receipt-1", "readback-1", Utc::now(), "stripe");
    assert!(!evidence.is_committed());

    let resolved = record
        .resolve(
            evidence.clone(),
            ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1"),
            Utc::now(),
        )
        .unwrap();

    assert_eq!(resolved.state().as_str(), "not_committed");
    assert_eq!(
        resolved.outcome().map(|o| o.verdict),
        Some(OutcomeVerdict::NotCommitted)
    );
}

#[test]
fn the_policy_rejects_incomplete_evidence() {
    let record = ambiguous_record();
    let resolver = ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1");
    let policy = ReconciliationPolicy::new();

    for incomplete in [
        ObservedOutcome {
            receipt_id: String::new(),
            readback_digest: "readback-1".to_string(),
            observed_at: Utc::now(),
            source: "stripe".to_string(),
            verdict: OutcomeVerdict::Committed,
        },
        ObservedOutcome {
            receipt_id: "receipt-1".to_string(),
            readback_digest: String::new(),
            observed_at: Utc::now(),
            source: "stripe".to_string(),
            verdict: OutcomeVerdict::Committed,
        },
        ObservedOutcome {
            receipt_id: "receipt-1".to_string(),
            readback_digest: "readback-1".to_string(),
            observed_at: Utc::now(),
            source: String::new(),
            verdict: OutcomeVerdict::Committed,
        },
    ] {
        assert!(!incomplete.is_complete());
        assert!(matches!(
            policy.authorize_resolution(&record, Some(&incomplete), &resolver),
            Err(CommitmentError::EvidenceMissing { .. })
        ));
    }
}

#[test]
fn evidence_round_trips_through_the_wire_contract() {
    let evidence = ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe");
    let json = serde_json::to_string(&evidence).unwrap();
    assert!(json.contains("\"verdict\":\"committed\""));
    let decoded: ObservedOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, evidence);
    assert!(decoded.is_complete());
}
