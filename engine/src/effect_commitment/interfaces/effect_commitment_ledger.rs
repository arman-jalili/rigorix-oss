//! The `EffectCommitmentLedger` port.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentledger
//! Implements: ISSUE-CONTRACT-FREEZE — the port consumed by the dispatch seam
//! Issue: #1113
//! ADR-019: engine owns the contract; storage owns the atomicity
//!
//! The port: `begin`, `settle_definite`, `mark_ambiguous`, `resolve`, `query`.
//! Program against this trait; adapters (`InMemoryEffectLedger`,
//! `HttpEffectLedger`) implement it. No storage type appears in this contract.
//!
//! # Invariants (Frozen)
//!
//! - **L1 — Write-ahead**: `begin` is durable before the effect is sent. If it
//!   fails, the caller MUST NOT send the effect.
//! - **L1 — Fences**: an `Ambiguous` record blocks dependents and retry until it
//!   is resolved.
//! - **Fail-closed**: any transport ambiguity maps to
//!   [`CommitmentError::Unavailable`]; the caller never assumes a commitment it
//!   did not observe.
//! - **Custody**: a repeated `begin` with the same intent-derived key returns
//!   the existing record; it never creates a second one. A same-key/different-
//!   intent call is [`CommitmentError::Contended`].
//! - **Terminal uniqueness**: a settled record cannot be settled again
//!   ([`CommitmentError::AlreadySettled`]).
//! - **Evidence**: only `resolve` may leave `Ambiguous`, and only with an
//!   [`ObservedOutcome`] and an authorised [`ResolverIdentity`].

use async_trait::async_trait;

use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity,
};

/// The effect-commitment port (ADR-019).
///
/// Implementations own durability and atomicity; the domain owns the state
/// machine and the fences.
#[async_trait]
pub trait EffectCommitmentLedger: Send + Sync {
    /// Record the write-ahead intent and return a `Dispatched` record.
    ///
    /// MUST be durable before the caller dispatches. A repeated call with the
    /// same intent-derived key returns the existing record (custody).
    async fn begin(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError>;

    /// Settle a `Dispatched` record with definite provider evidence.
    ///
    /// Returns `Committed` or `NotCommitted` according to the evidence verdict.
    async fn settle_definite(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
    ) -> Result<EffectRecord, CommitmentError>;

    /// Mark a `Dispatched` record `Ambiguous` because the outcome is unknown.
    ///
    /// The record keeps blocking dependents and retry until resolved.
    async fn mark_ambiguous(
        &self,
        record_id: EffectRecordId,
        reason: String,
    ) -> Result<EffectRecord, CommitmentError>;

    /// Leave `Ambiguous` with provider evidence and a recorded resolver.
    ///
    /// The adapter MUST enforce F3/F4 through `ReconciliationPolicy` and MUST
    /// reject the dispatching process and the agent as resolvers.
    async fn resolve(
        &self,
        record_id: EffectRecordId,
        outcome: ObservedOutcome,
        resolver: ResolverIdentity,
    ) -> Result<EffectRecord, CommitmentError>;

    /// Read records by identity, by DAG, or list ambiguous effects.
    async fn query(&self, query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError>;

    /// Whether **this adapter's owner** may sweep expired `Dispatched` records.
    ///
    /// The predicate `started_at + lease < now` mixes two clocks when the record
    /// was written by another process. An in-process ledger has one clock and
    /// its owner must sweep (it is the only thing closing the crash window); a
    /// distributed ledger is shared by many processes, so the service that owns
    /// the store must sweep, and a client must not (#1173).
    ///
    /// This is a capability, not `backend.kind == "http"`: the adapter knows how
    /// its records are shared, and callers must not have to re-derive it.
    fn can_sweep_dispatched(&self) -> bool;
}
