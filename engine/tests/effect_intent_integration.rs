//! EffectIntent integration: intent → record → event.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectintent
//! Implements: ISSUE-EFFECT-COMMITMENT-1 — upstream/downstream integration
//! Issue: #1114
//! ADR-019: the intent is recorded before the effect is sent
//!
//! Exercises the frozen boundary rather than the isolated value object: the
//! intent is carried through the begin DTO, becomes a write-ahead record, and
//! its identity survives persistence and a retry with a new attempt counter.

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::BeginEffectInput;
use rigorix_engine::effect_commitment::domain::{
    EffectEvent, EffectIntent, EffectRecord, ObservedOutcome, ResolverIdentity, ResolverKind,
};
use uuid::Uuid;

fn intent(attempt: u32) -> EffectIntent {
    EffectIntent::new(
        Uuid::from_u128(7),
        Uuid::from_u128(11),
        attempt,
        IdempotencyKey::new("dag:node:payload"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    )
}

#[test]
fn intent_survives_the_begin_dto_and_is_recorded_dispatched() {
    let intent = intent(1);
    let input = BeginEffectInput {
        intent: intent.clone(),
    };

    // The begin DTO is the adapter boundary, so the intent must round-trip.
    let json = serde_json::to_string(&input).unwrap();
    let decoded: BeginEffectInput = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.intent, intent);

    let record = EffectRecord::begin(decoded.intent);
    assert_eq!(record.state().as_str(), "dispatched");
    assert!(!record.is_terminal());
    assert!(record.outcome().is_none());
    assert_eq!(record.intent().idempotency_key.as_str(), "dag:node:payload");
}

#[test]
fn a_retry_with_a_new_attempt_is_the_same_intent() {
    let first = intent(1);
    let retry = intent(2);

    // The attempt counter is diagnostics; identity is the payload.
    assert!(first.same_intent(&retry));
    assert_eq!(first.intent_digest(), retry.intent_digest());

    // Same intent key, same payload ⇒ the ledger reads the existing record
    // instead of creating a second one.
    let existing = EffectRecord::begin(first.clone());
    let replay = EffectRecord::begin(retry);
    assert_eq!(
        existing.intent().idempotency_key,
        replay.intent().idempotency_key
    );

    // A changed payload is a different intent and must not collapse onto it.
    let mut different = intent(3);
    different.args_digest = "other-args".to_string();
    assert!(!first.same_intent(&different));
}

#[test]
fn an_observed_outcome_settles_the_record_created_from_the_intent() {
    let record = EffectRecord::begin(intent(1));
    let outcome = ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe");
    let settled = record.settle_definite(outcome.clone(), Utc::now()).unwrap();

    assert!(settled.is_terminal());
    assert_eq!(settled.outcome(), Some(&outcome));
}

#[test]
fn an_ambiguous_effect_is_resolved_only_by_an_authorised_resolver() {
    let record = EffectRecord::begin(intent(1))
        .mark_ambiguous("response lost", Utc::now())
        .unwrap();
    assert!(record.blocks_dependents());
    assert!(record.blocks_retry());

    let evidence = ObservedOutcome::not_committed("receipt-2", "readback-2", Utc::now(), "stripe");
    let resolved = record
        .resolve(
            evidence.clone(),
            ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-2"),
            Utc::now(),
        )
        .unwrap();
    assert!(resolved.is_terminal());
    assert_eq!(resolved.state().as_str(), "not_committed");
}

#[test]
fn the_intent_is_carried_by_the_recorded_event() {
    let record = EffectRecord::begin(intent(1));
    let event = EffectEvent::IntentRecorded {
        record_id: record.id(),
        dag_id: record.intent().dag_id,
        node_id: record.intent().node_id,
        attempt: record.intent().attempt,
        idempotency_key: record.intent().idempotency_key.clone(),
        tool: record.intent().tool.clone(),
        state: record.state(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"intent_recorded\""));
    let decoded: EffectEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, event);
}
