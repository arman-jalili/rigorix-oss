//! ReconciliationPolicy integration: F3/F4 as the runtime gate.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#reconciliationpolicy
//! Implements: ISSUE-EFFECT-COMMITMENT-9 — upstream/downstream integration
//! Issue: #1122
//! ADR-019: only provider evidence, presented by external authority, settles

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::ReconciliationPolicy;
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectRecord, EffectState, ObservedOutcome, ResolverIdentity,
    ResolverKind,
};
use uuid::Uuid;

fn dispatched() -> EffectRecord {
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
}

fn evidence() -> ObservedOutcome {
    ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe")
}

fn resolver(kind: ResolverKind) -> ResolverIdentity {
    ResolverIdentity::new("resolver", kind, "readback-1")
}

#[test]
fn settlement_requires_a_dispatched_record_and_complete_evidence() {
    let policy = ReconciliationPolicy::new();
    assert!(
        policy
            .authorize_settlement(&dispatched(), &evidence())
            .is_ok()
    );

    let ambiguous = dispatched().mark_ambiguous("lost", Utc::now()).unwrap();
    assert!(matches!(
        policy.authorize_settlement(&ambiguous, &evidence()),
        Err(CommitmentError::NotAmbiguous { .. })
    ));

    let settled = dispatched()
        .settle_definite(evidence(), Utc::now())
        .unwrap();
    assert!(matches!(
        policy.authorize_settlement(&settled, &evidence()),
        Err(CommitmentError::AlreadySettled { .. })
    ));
}

#[test]
fn resolution_requires_ambiguity_complete_evidence_and_external_authority() {
    let policy = ReconciliationPolicy::new();
    let ambiguous = dispatched().mark_ambiguous("lost", Utc::now()).unwrap();

    // F3: evidence is mandatory.
    assert!(matches!(
        policy.authorize_resolution(&ambiguous, None, &resolver(ResolverKind::Provider)),
        Err(CommitmentError::EvidenceMissing { .. })
    ));

    // F4: the parties that cannot know are rejected.
    for kind in [ResolverKind::DispatchingProcess, ResolverKind::Agent] {
        assert!(matches!(
            policy.authorize_resolution(&ambiguous, Some(&evidence()), &resolver(kind)),
            Err(CommitmentError::UnauthorizedResolver { .. })
        ));
    }

    // External authority with complete evidence is authorised.
    for kind in [ResolverKind::Provider, ResolverKind::ExternalOperator] {
        assert!(
            policy
                .authorize_resolution(&ambiguous, Some(&evidence()), &resolver(kind))
                .is_ok()
        );
    }
}

#[test]
fn marking_ambiguous_is_only_legal_from_dispatched() {
    let policy = ReconciliationPolicy::new();
    let dispatched = dispatched();
    assert!(policy.authorize_mark_ambiguous(&dispatched).is_ok());

    let ambiguous = dispatched
        .clone()
        .mark_ambiguous("lost", Utc::now())
        .unwrap();
    assert!(matches!(
        policy.authorize_mark_ambiguous(&ambiguous),
        Err(CommitmentError::Contended { .. })
    ));

    let committed = dispatched.settle_definite(evidence(), Utc::now()).unwrap();
    assert!(matches!(
        policy.authorize_mark_ambiguous(&committed),
        Err(CommitmentError::AlreadySettled { .. })
    ));
}

#[test]
fn the_policy_matches_the_state_machine_for_every_edge() {
    let policy = ReconciliationPolicy::new();
    let states = [
        EffectState::Dispatched,
        EffectState::Committed,
        EffectState::NotCommitted,
        EffectState::Ambiguous,
    ];
    for from in states {
        for to in states {
            assert_eq!(
                policy.is_legal_transition(from, to),
                from.can_transition_to(to),
                "{from} -> {to}"
            );
        }
    }

    // The forbidden retry edge is explicit.
    assert!(!policy.is_legal_transition(EffectState::Ambiguous, EffectState::Dispatched));
}
