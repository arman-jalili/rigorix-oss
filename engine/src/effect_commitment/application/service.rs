//! EffectCommitmentService — the application use cases.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentservice
//! Implements: ISSUE-CONTRACT-FREEZE — begin/settle_definite/mark_ambiguous/resolve/query
//! Issue: #1113
//! ADR-019: fail-closed; an unreachable ledger refuses the dispatch
//!
//! The use cases over the [`EffectCommitmentLedger`] port. The service owns no
//! storage and no local counter; it delegates to an adapter, applies
//! [`ReconciliationPolicy`] before leaving `Ambiguous`, and derives intent keys
//! through [`IntentKeyDerivation`].
//!
//! # Contract (Frozen)
//! - `begin` MUST be durable before the caller sends the effect
//! - A failed `begin` refuses the dispatch: proceeding unrecorded is exactly
//!   the window this context exists to close
//! - `settle_definite` applies only to `Dispatched`
//! - `resolve` applies only to `Ambiguous` and only with provider evidence
//! - Every use case takes a DTO and returns a DTO or a typed [`CommitmentError`]
//!
//! [`EffectCommitmentLedger`]: crate::effect_commitment::interfaces::EffectCommitmentLedger
//! [`ReconciliationPolicy`]: super::ReconciliationPolicy
//! [`IntentKeyDerivation`]: super::IntentKeyDerivation

use async_trait::async_trait;

use crate::effect_commitment::domain::CommitmentError;

use super::dto::{
    BeginEffectInput, BeginEffectOutput, MarkAmbiguousInput, MarkAmbiguousOutput, QueryEffectInput,
    QueryEffectOutput, ResolveEffectInput, ResolveEffectOutput, SettleDefiniteInput,
    SettleDefiniteOutput,
};

/// Application service exposing effect-commitment use cases.
#[async_trait]
pub trait EffectCommitmentService: Send + Sync {
    /// Record the write-ahead intent.
    ///
    /// Returns the `Dispatched` record. A retry with the same intent-derived key
    /// returns the existing record instead of creating a second one.
    async fn begin(&self, input: BeginEffectInput) -> Result<BeginEffectOutput, CommitmentError>;

    /// Settle a dispatched effect with definite provider evidence.
    async fn settle_definite(
        &self,
        input: SettleDefiniteInput,
    ) -> Result<SettleDefiniteOutput, CommitmentError>;

    /// Mark a dispatched effect whose outcome is unknown.
    async fn mark_ambiguous(
        &self,
        input: MarkAmbiguousInput,
    ) -> Result<MarkAmbiguousOutput, CommitmentError>;

    /// Resolve an ambiguous effect with provider evidence and a recorded
    /// resolver.
    async fn resolve(
        &self,
        input: ResolveEffectInput,
    ) -> Result<ResolveEffectOutput, CommitmentError>;

    /// Read records by identity, by DAG, or list ambiguous effects.
    async fn query(&self, input: QueryEffectInput) -> Result<QueryEffectOutput, CommitmentError>;
}
