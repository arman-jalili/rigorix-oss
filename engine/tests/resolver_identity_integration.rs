//! ResolverIdentity integration: who may settle, and on what evidence.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#resolveridentity
//! Implements: ISSUE-EFFECT-COMMITMENT-5 — upstream/downstream integration
//! Issue: #1118
//! ADR-019: F3/F4 — external evidence only, never the party that lost the response

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::ReconciliationPolicy;
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectRecord, ObservedOutcome, ResolverIdentity, ResolverKind,
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

fn evidence() -> ObservedOutcome {
    ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe")
}

#[test]
fn external_authority_can_resolve_and_is_recorded() {
    let record = ambiguous_record();
    let resolver = ResolverIdentity::new("stripe-readback", ResolverKind::Provider, "readback-1");
    let policy = ReconciliationPolicy::new();

    policy
        .authorize_resolution(&record, Some(&evidence()), &resolver)
        .expect("provider authority is allowed");

    let resolved = record
        .resolve(evidence(), resolver.clone(), Utc::now())
        .unwrap();
    assert_eq!(resolved.resolver(), Some(&resolver));
}

#[test]
fn the_dispatcher_and_agent_are_rejected_by_the_policy() {
    let record = ambiguous_record();
    let policy = ReconciliationPolicy::new();

    for kind in [ResolverKind::DispatchingProcess, ResolverKind::Agent] {
        let resolver = ResolverIdentity::new("bad-resolver", kind, "readback-1");
        assert!(!resolver.is_authorized());
        assert!(matches!(
            policy.authorize_resolution(&record, Some(&evidence()), &resolver),
            Err(CommitmentError::UnauthorizedResolver { .. })
        ));
    }
}

#[test]
fn a_resolver_without_an_evidence_digest_is_refused() {
    let record = ambiguous_record();
    let resolver = ResolverIdentity::new("stripe", ResolverKind::Provider, "  ");
    assert!(!resolver.has_evidence_digest());

    assert!(matches!(
        ReconciliationPolicy::new().authorize_resolution(&record, Some(&evidence()), &resolver),
        Err(CommitmentError::EvidenceMissing { .. })
    ));
}

#[test]
fn resolver_identity_round_trips_and_kinds_are_stable() {
    let resolver = ResolverIdentity::new("ops-1", ResolverKind::ExternalOperator, "readback-1");
    let json = serde_json::to_string(&resolver).unwrap();
    assert!(json.contains("\"kind\":\"external_operator\""));
    let decoded: ResolverIdentity = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, resolver);
}
