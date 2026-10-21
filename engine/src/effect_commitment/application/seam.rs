//! DispatchSeam — write-ahead and settle around a dispatch.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#dispatchseam
//! Implements: ISSUE-CONTRACT-FREEZE — dispatch seam port
//! Issue: #1113
//! ADR-019: the component whose absence leaves the context compiled and unreachable
//!
//! Inside the dispatch loop, after the precondition/sequence/capacity gates and
//! immediately before `spawn_concurrent_node`, the seam records the intent
//! durably. After the node returns it settles with evidence, or marks the effect
//! ambiguous on a lost response, timeout, or cancellation in flight.
//!
//! # Contract (Frozen)
//! - A failed [`DispatchSeam::before_dispatch`] **refuses** the dispatch
//! - `after_dispatch` is called exactly once per begun record
//! - A lost response is [`DispatchObservation::Lost`], which maps to `Ambiguous`
//!   and never to `NotCommitted`

use async_trait::async_trait;

use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectRecord, EffectRecordId, ObservedOutcome,
};

/// What the dispatch loop observed when the node returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchObservation {
    /// Provider/definite evidence of what happened.
    Observed(ObservedOutcome),

    /// The response was lost, timed out, or the node was cancelled in flight.
    Lost {
        /// Why the outcome is unknown.
        reason: String,
    },
}

/// The seam between the dispatch loop and the commitment ledger.
#[async_trait]
pub trait DispatchSeam: Send + Sync {
    /// Record the intent durably before the effect is sent.
    ///
    /// # Errors
    /// Any [`CommitmentError`] means the dispatch must not proceed.
    async fn before_dispatch(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError>;

    /// Settle the record with what the dispatch observed.
    ///
    /// `Observed` calls `settle_definite`; `Lost` calls `mark_ambiguous`.
    async fn after_dispatch(
        &self,
        record_id: EffectRecordId,
        observation: DispatchObservation,
    ) -> Result<EffectRecord, CommitmentError>;
}
