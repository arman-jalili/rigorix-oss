//! EffectCommitmentService integration: service ↔ ledger port.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentservice
//! Implements: ISSUE-EFFECT-COMMITMENT-7 — upstream/downstream integration
//! Issue: #1120
//! ADR-019: write-ahead, custody, and fail-closed resolution

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{
    BeginEffectInput, EffectCommitmentService, EffectCommitmentServiceImpl, IntentKeyDerivation,
    MarkAmbiguousInput, QueryEffectInput, ResolveEffectInput,
};
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity, ResolverKind,
};
use rigorix_engine::effect_commitment::interfaces::EffectCommitmentLedger;
use uuid::Uuid;

/// A deterministic derivation for the integration test. ISSUE-EFFECT-COMMITMENT-8
/// supplies the SHA-256 implementation; the contract only requires stability.
struct TestDerivation;

impl IntentKeyDerivation for TestDerivation {
    fn derive_key(&self, intent: &EffectIntent) -> IdempotencyKey {
        IdempotencyKey::new(format!(
            "{}:{}:{}",
            intent.dag_id,
            intent.node_id,
            intent.intent_digest()
        ))
    }
}

#[derive(Default)]
struct TestLedger {
    records: Mutex<HashMap<EffectRecordId, EffectRecord>>,
    by_key: Mutex<HashMap<IdempotencyKey, EffectRecordId>>,
}

impl TestLedger {
    fn get(&self, id: EffectRecordId) -> Result<EffectRecord, CommitmentError> {
        self.records
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| CommitmentError::Unavailable {
                detail: format!("record {id} not found"),
            })
    }

    fn put(&self, record: EffectRecord) -> EffectRecord {
        self.records
            .lock()
            .unwrap()
            .insert(record.id(), record.clone());
        record
    }
}

#[async_trait]
impl EffectCommitmentLedger for TestLedger {
    async fn begin(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        let key = intent.idempotency_key.clone();
        if let Some(existing) = self.by_key.lock().unwrap().get(&key).copied() {
            let record = self.get(existing)?;
            // Custody: same key + same payload reads the existing record.
            if record.intent().same_intent(&intent) {
                return Ok(record);
            }
            return Err(CommitmentError::Contended {
                record_id: Some(existing),
                detail: "same key with a different intent payload".to_string(),
            });
        }

        let record = EffectRecord::begin(intent);
        self.by_key.lock().unwrap().insert(key, record.id());
        Ok(self.put(record))
    }

    async fn settle_definite(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
    ) -> Result<EffectRecord, CommitmentError> {
        let record = self.get(record_id)?.settle_definite(outcome, Utc::now())?;
        Ok(self.put(record))
    }

    async fn mark_ambiguous(
        &self,
        record_id: EffectRecordId,
        reason: String,
    ) -> Result<EffectRecord, CommitmentError> {
        let record = self.get(record_id)?.mark_ambiguous(reason, Utc::now())?;
        Ok(self.put(record))
    }

    async fn resolve(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
        resolver: ResolverIdentity,
    ) -> Result<EffectRecord, CommitmentError> {
        let record = self
            .get(record_id)?
            .resolve(outcome, resolver, Utc::now())?;
        Ok(self.put(record))
    }

    async fn query(&self, query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError> {
        let mut records: Vec<_> = self
            .records
            .lock()
            .unwrap()
            .values()
            .filter(|record| record.matches_query(&query))
            .cloned()
            .collect();
        records.sort_by_key(|record| record.id());
        Ok(records)
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

fn service() -> EffectCommitmentServiceImpl {
    EffectCommitmentServiceImpl::new(Arc::new(TestLedger::default()), Arc::new(TestDerivation))
}

fn evidence() -> ObservedOutcome {
    ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe")
}

#[tokio::test]
async fn a_retry_of_the_same_intent_reads_the_existing_record() {
    let service = service();

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

    assert_eq!(first.record.id(), retry.record.id());
    assert_eq!(
        first.record.intent().idempotency_key,
        retry.record.intent().idempotency_key
    );
}

#[tokio::test]
async fn a_changed_payload_does_not_collapse_onto_the_existing_record() {
    let service = service();

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
    assert_ne!(
        first.record.intent().idempotency_key,
        changed.record.intent().idempotency_key
    );
}

#[tokio::test]
async fn an_ambiguous_effect_is_visible_and_only_a_provider_can_resolve_it() {
    let service = service();
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

    let pending = service
        .query(QueryEffectInput {
            query: EffectQuery::ambiguous(None),
        })
        .await
        .unwrap();
    assert_eq!(pending.records, vec![ambiguous.record.clone()]);

    // The agent cannot settle it.
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

    // The provider can, and the record leaves the ambiguous queue.
    let resolved = service
        .resolve(ResolveEffectInput {
            record_id: begun.record.id(),
            outcome: evidence(),
            resolver: ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1"),
        })
        .await
        .unwrap();
    assert_eq!(resolved.record.state().as_str(), "committed");

    let pending = service
        .query(QueryEffectInput {
            query: EffectQuery::ambiguous(None),
        })
        .await
        .unwrap();
    assert!(pending.records.is_empty());
}
