//! EffectState integration: vocabulary, records, and fences agree.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectstate
//! Implements: ISSUE-EFFECT-COMMITMENT-2 — upstream/downstream integration
//! Issue: #1115
//! ADR-019: the four-state machine and its fencing rules

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{
    NodeStateVocabulary, ReconciliationPolicy, may_release_dependents, may_retry,
};
use rigorix_engine::effect_commitment::domain::{
    EffectIntent, EffectRecord, EffectState, ObservedOutcome, ResolverIdentity, ResolverKind,
};
use uuid::Uuid;

fn intent() -> EffectIntent {
    EffectIntent::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        1,
        IdempotencyKey::new("intent-key"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    )
}

#[test]
fn every_state_round_trips_through_its_wire_vocabulary() {
    for state in [
        EffectState::Dispatched,
        EffectState::Committed,
        EffectState::NotCommitted,
        EffectState::Ambiguous,
    ] {
        assert_eq!(EffectState::parse(state.as_str()), Some(state));
    }

    // The two new node words are exactly the two non-terminal effect states.
    assert_eq!(
        NodeStateVocabulary::effect_state_for(NodeStateVocabulary::DISPATCHED),
        Some(EffectState::Dispatched)
    );
    assert_eq!(
        NodeStateVocabulary::effect_state_for(NodeStateVocabulary::AMBIGUOUS),
        Some(EffectState::Ambiguous)
    );
    assert!(!NodeStateVocabulary::is_terminal(
        NodeStateVocabulary::DISPATCHED
    ));
    assert!(!NodeStateVocabulary::is_terminal(
        NodeStateVocabulary::AMBIGUOUS
    ));
}

#[test]
fn records_and_vocabulary_agree_on_terminality() {
    let dispatched = EffectRecord::begin(intent());
    assert_eq!(
        dispatched.state().is_terminal(),
        NodeStateVocabulary::is_terminal(NodeStateVocabulary::label(dispatched.state()))
    );

    let ambiguous = dispatched
        .clone()
        .mark_ambiguous("lost", Utc::now())
        .unwrap();
    assert!(!ambiguous.state().is_terminal());
    assert_eq!(
        NodeStateVocabulary::label(ambiguous.state()),
        NodeStateVocabulary::AMBIGUOUS
    );

    let committed = dispatched
        .clone()
        .settle_definite(
            ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe"),
            Utc::now(),
        )
        .unwrap();
    assert!(committed.state().is_terminal());
    assert_eq!(NodeStateVocabulary::label(committed.state()), "completed");
}

#[test]
fn ambiguous_is_the_only_state_that_fences() {
    let dispatched = EffectRecord::begin(intent());
    assert!(may_retry(&dispatched));
    assert!(may_release_dependents(std::slice::from_ref(&dispatched)));

    let ambiguous = dispatched
        .clone()
        .mark_ambiguous("lost", Utc::now())
        .unwrap();
    assert!(!may_retry(&ambiguous));
    assert!(!may_release_dependents(std::slice::from_ref(&ambiguous)));

    let resolved = ambiguous
        .resolve(
            ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe"),
            ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1"),
            Utc::now(),
        )
        .unwrap();
    assert!(may_retry(&resolved));
    assert!(may_release_dependents(std::slice::from_ref(&resolved)));
}

#[test]
fn the_policy_uses_the_type_owned_transition_graph() {
    let policy = ReconciliationPolicy::new();
    for from in [
        EffectState::Dispatched,
        EffectState::Committed,
        EffectState::NotCommitted,
        EffectState::Ambiguous,
    ] {
        for to in [
            EffectState::Dispatched,
            EffectState::Committed,
            EffectState::NotCommitted,
            EffectState::Ambiguous,
        ] {
            assert_eq!(
                policy.is_legal_transition(from, to),
                from.can_transition_to(to)
            );
        }
    }
}
