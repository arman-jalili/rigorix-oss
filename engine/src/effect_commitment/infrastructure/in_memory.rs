//! InMemoryEffectLedger — the L1 adapter.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#inmemoryeffectledger
//! Implements: ISSUE-CONTRACT-FREEZE — single-process adapter stub
//! Issue: #1113
//!
//! The standalone default: exact within one process, meaningless across
//! processes. Same honesty rule as the capacity ledger's in-process adapter — no
//! deployment may claim L2 while running this.
//!
//! # Status
//! ISSUE-EFFECT-COMMITMENT-10 supplies the implementation. Until then every
//! operation fails closed with [`CommitmentError::Unavailable`]: a caller must
//! never proceed as if an unrecorded intent were durable.

use async_trait::async_trait;

use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity,
};
use crate::effect_commitment::interfaces::EffectCommitmentLedger;

/// The in-process commitment ledger.
#[derive(Debug, Default)]
pub struct InMemoryEffectLedger;

impl InMemoryEffectLedger {
    /// Create an empty in-process ledger.
    pub fn new() -> Self {
        Self
    }

    fn not_implemented(operation: &str) -> CommitmentError {
        CommitmentError::Unavailable {
            detail: format!(
                "ISSUE-EFFECT-COMMITMENT-10: InMemoryEffectLedger::{operation} is not implemented"
            ),
        }
    }
}

#[async_trait]
impl EffectCommitmentLedger for InMemoryEffectLedger {
    async fn begin(&self, _intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("begin"))
    }

    async fn settle_definite(
        &self,
        _record_id: EffectRecordId,
        _outcome: ObservedOutcome,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("settle_definite"))
    }

    async fn mark_ambiguous(
        &self,
        _record_id: EffectRecordId,
        _reason: String,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("mark_ambiguous"))
    }

    async fn resolve(
        &self,
        _record_id: EffectRecordId,
        _outcome: ObservedOutcome,
        _resolver: ResolverIdentity,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("resolve"))
    }

    async fn query(&self, _query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError> {
        Err(Self::not_implemented("query"))
    }
}
