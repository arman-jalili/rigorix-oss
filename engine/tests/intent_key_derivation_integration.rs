//! IntentKeyDerivation integration: the service records the derived key.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#intentkeyderivation
//! Implements: ISSUE-EFFECT-COMMITMENT-8 — upstream/downstream integration
//! Issue: #1121
//! ADR-019: same intent + same payload ⇒ same key ⇒ read, not re-send

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{
    BeginEffectInput, EffectCommitmentService, EffectCommitmentServiceImpl, IntentKeyDerivation,
    Sha256IntentKeyDerivation,
};
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity,
};
use rigorix_engine::effect_commitment::interfaces::EffectCommitmentLedger;
use uuid::Uuid;

/// Records the exact intent the service hands to the port.
#[derive(Default)]
struct RecordingLedger {
    last_intent: Mutex<Option<EffectIntent>>,
}

#[async_trait]
impl EffectCommitmentLedger for RecordingLedger {
    fn can_sweep_dispatched(&self) -> bool {
        true
    }

    async fn begin(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        *self.last_intent.lock().unwrap() = Some(intent.clone());
        Ok(EffectRecord::begin(intent))
    }

    async fn settle_definite(
        &self,
        _record_id: EffectRecordId,
        _outcome: ObservedOutcome,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(CommitmentError::Unavailable {
            detail: "not used".to_string(),
        })
    }

    async fn mark_ambiguous(
        &self,
        _record_id: EffectRecordId,
        _reason: String,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(CommitmentError::Unavailable {
            detail: "not used".to_string(),
        })
    }

    async fn resolve(
        &self,
        _record_id: EffectRecordId,
        _outcome: ObservedOutcome,
        _resolver: ResolverIdentity,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(CommitmentError::Unavailable {
            detail: "not used".to_string(),
        })
    }

    async fn query(&self, _query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError> {
        Ok(Vec::new())
    }
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

#[tokio::test]
async fn begin_records_the_intent_derived_key() {
    let ledger = Arc::new(RecordingLedger::default());
    let service = EffectCommitmentServiceImpl::new(
        ledger.clone(),
        Arc::new(Sha256IntentKeyDerivation::new()),
    );

    let caller_intent = intent(1, "args");
    let expected = Sha256IntentKeyDerivation::new().derive_key(&caller_intent);
    let out = service
        .begin(BeginEffectInput {
            intent: caller_intent,
        })
        .await
        .unwrap();

    // The caller's attempt-derived key was replaced by the intent-derived one.
    assert_eq!(out.record.intent().idempotency_key, expected);
    assert_ne!(
        out.record.intent().idempotency_key,
        IdempotencyKey::new("attempt-1")
    );
    assert_eq!(
        ledger
            .last_intent
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .idempotency_key,
        expected
    );
}

#[tokio::test]
async fn a_retry_derives_the_same_key_and_a_changed_payload_does_not() {
    let service = EffectCommitmentServiceImpl::new(
        Arc::new(RecordingLedger::default()),
        Arc::new(Sha256IntentKeyDerivation::new()),
    );

    let first = service
        .begin(BeginEffectInput {
            intent: intent(1, "args"),
        })
        .await
        .unwrap();
    let retry = service
        .begin(BeginEffectInput {
            intent: intent(2, "args"),
        })
        .await
        .unwrap();
    assert_eq!(
        first.record.intent().idempotency_key,
        retry.record.intent().idempotency_key
    );

    let changed = service
        .begin(BeginEffectInput {
            intent: intent(3, "different-args"),
        })
        .await
        .unwrap();
    assert_ne!(
        first.record.intent().idempotency_key,
        changed.record.intent().idempotency_key
    );
}
