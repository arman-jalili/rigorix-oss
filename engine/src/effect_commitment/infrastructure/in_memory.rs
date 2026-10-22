//! InMemoryEffectLedger — the L1 adapter.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#inmemoryeffectledger
//! Implements: ISSUE-EFFECT-COMMITMENT-10 — in-process commitment ledger
//! Issue: #1123
//! ADR-019: write-ahead record + fences, exact within one process
//!
//! The standalone default: exact within one process, meaningless across
//! processes. Same honesty rule as the capacity ledger's in-process adapter — no
//! deployment may claim L2 while running this.
//!
//! # Guarantee ceiling (Frozen)
//! This adapter provides **single-process durability** (L1): the record is
//! written before dispatch and every operation in this process linearizes on one
//! mutex. It MUST NOT be described as surviving process death; that is the HTTP
//! adapter's job (L2). Without a provider receipt reader the context can prove
//! **not-settled** only.
//!
//! # Invariants implemented
//! - **Write-ahead**: `begin` inserts the record before returning.
//! - **Custody**: a retry with the same intent-derived key returns the existing
//!   record; the same key with a different payload is `Contended`.
//! - **Terminal uniqueness**: a settled record cannot be settled again.
//! - **Evidence**: `resolve` runs `ReconciliationPolicy` (F3/F4) before leaving
//!   `Ambiguous`.
//! - **Fail-closed**: an unknown record or a poisoned lock is `Unavailable`.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;

use crate::capacity_ledger::domain::IdempotencyKey;
use crate::effect_commitment::application::ReconciliationPolicy;
use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity,
};
use crate::effect_commitment::interfaces::EffectCommitmentLedger;

#[derive(Default)]
struct LedgerState {
    records: HashMap<EffectRecordId, EffectRecord>,
    by_key: HashMap<IdempotencyKey, EffectRecordId>,
    /// Insertion order, so query results are deterministic.
    order: Vec<EffectRecordId>,
}

impl LedgerState {
    fn get(&self, id: EffectRecordId) -> Result<EffectRecord, CommitmentError> {
        self.records
            .get(&id)
            .cloned()
            .ok_or_else(|| CommitmentError::Unavailable {
                detail: format!("unknown effect record {id}"),
            })
    }

    fn put(&mut self, record: EffectRecord) -> EffectRecord {
        self.records.insert(record.id(), record.clone());
        record
    }
}

/// The in-process commitment ledger.
pub struct InMemoryEffectLedger {
    state: Mutex<LedgerState>,
    policy: ReconciliationPolicy,
}

impl InMemoryEffectLedger {
    /// Create an empty in-process ledger with the frozen reconciliation policy.
    pub fn new() -> Self {
        Self::with_policy(ReconciliationPolicy::new())
    }

    /// Create an empty in-process ledger with an explicit policy.
    pub fn with_policy(policy: ReconciliationPolicy) -> Self {
        Self {
            state: Mutex::new(LedgerState::default()),
            policy,
        }
    }

    fn locked(&self) -> Result<std::sync::MutexGuard<'_, LedgerState>, CommitmentError> {
        self.state.lock().map_err(|_| CommitmentError::Unavailable {
            detail: "in-process effect ledger poisoned".to_string(),
        })
    }
}

impl Default for InMemoryEffectLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EffectCommitmentLedger for InMemoryEffectLedger {
    fn can_sweep_dispatched(&self) -> bool {
        // One process, one clock: this owner is the only possible sweeper.
        true
    }

    async fn begin(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        let key = intent.idempotency_key.clone();
        let mut state = self.locked()?;

        if let Some(existing_id) = state.by_key.get(&key).copied() {
            let existing = state.get(existing_id)?;
            if existing.intent().same_intent(&intent) {
                // Custody: a retry reads what is already recorded.
                return Ok(existing);
            }
            return Err(CommitmentError::Contended {
                record_id: Some(existing_id),
                detail: "idempotency key reused with a different intent payload".to_string(),
            });
        }

        let record = EffectRecord::begin(intent);
        state.by_key.insert(key, record.id());
        state.order.push(record.id());
        tracing::debug!(
            record = %record.id(),
            dag = %record.intent().dag_id,
            node = %record.intent().node_id,
            "effect intent recorded (write-ahead)"
        );
        Ok(state.put(record))
    }

    async fn settle_definite(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
    ) -> Result<EffectRecord, CommitmentError> {
        let mut state = self.locked()?;
        let record = state.get(record_id)?;
        self.policy.authorize_settlement(&record, &outcome)?;
        let settled = record.settle_definite(outcome, Utc::now())?;
        Ok(state.put(settled))
    }

    async fn mark_ambiguous(
        &self,
        record_id: EffectRecordId,
        reason: String,
    ) -> Result<EffectRecord, CommitmentError> {
        let mut state = self.locked()?;
        let record = state.get(record_id)?;
        self.policy.authorize_mark_ambiguous(&record)?;
        let ambiguous = record.mark_ambiguous(reason, Utc::now())?;
        tracing::warn!(
            record = %ambiguous.id(),
            "effect outcome unknown; record is Ambiguous and fences dependents/retry"
        );
        Ok(state.put(ambiguous))
    }

    async fn resolve(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
        resolver: ResolverIdentity,
    ) -> Result<EffectRecord, CommitmentError> {
        let mut state = self.locked()?;
        let record = state.get(record_id)?;
        self.policy
            .authorize_resolution(&record, Some(&outcome), &resolver)?;
        let resolved = record.resolve(outcome, resolver, Utc::now())?;
        tracing::info!(
            record = %resolved.id(),
            state = %resolved.state(),
            resolver = %resolved
                .resolver()
                .map(|r| r.resolver_id.as_str())
                .unwrap_or("<unknown>"),
            "ambiguous effect resolved with provider evidence"
        );
        Ok(state.put(resolved))
    }

    async fn query(&self, query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError> {
        let state = self.locked()?;
        Ok(state
            .order
            .iter()
            .filter_map(|id| state.records.get(id))
            .filter(|record| record.matches_query(&query))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn satisfies_the_frozen_port_contract() {
        let ledger = InMemoryEffectLedger::new();
        crate::effect_commitment::interfaces::contract::run_contract(&ledger).await;
    }
}
