//! EffectCommitmentServiceImpl — the application use cases over the port.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentservice
//! Implements: ISSUE-EFFECT-COMMITMENT-7 — concrete begin/settle/mark/resolve/query
//! Issue: #1120
//! ADR-019: fail-closed; an unreachable ledger refuses the dispatch
//!
//! The service owns no storage and no local counter. It derives the
//! intent-derived key before recording, applies [`ReconciliationPolicy`] before
//! leaving `Ambiguous`, and delegates the durable transitions to a
//! [`EffectCommitmentLedger`] adapter. The adapter re-checks the same rules
//! atomically, so the service pre-check is a fast, precise error rather than the
//! authority.
//!
//! # Contract (Frozen)
//! - `begin` MUST be durable before the caller sends the effect; a failure
//!   refuses the dispatch
//! - The key on the recorded intent is always intent-derived, even if the
//!   caller supplied one
//! - `resolve` refuses the dispatching process and the agent (F4)

use std::sync::Arc;

use async_trait::async_trait;

use crate::effect_commitment::domain::{CommitmentError, EffectQuery, EffectRecord};
use crate::effect_commitment::interfaces::EffectCommitmentLedger;

use super::dto::{
    BeginEffectInput, BeginEffectOutput, MarkAmbiguousInput, MarkAmbiguousOutput, QueryEffectInput,
    QueryEffectOutput, ResolveEffectInput, ResolveEffectOutput, SettleDefiniteInput,
    SettleDefiniteOutput,
};
use super::key::IntentKeyDerivation;
use super::policy::ReconciliationPolicy;
use super::service::EffectCommitmentService;

/// The default application service.
pub struct EffectCommitmentServiceImpl {
    ledger: Arc<dyn EffectCommitmentLedger>,
    key_derivation: Arc<dyn IntentKeyDerivation>,
    policy: ReconciliationPolicy,
}

impl EffectCommitmentServiceImpl {
    /// Construct the service over a ledger and a key derivation.
    pub fn new(
        ledger: Arc<dyn EffectCommitmentLedger>,
        key_derivation: Arc<dyn IntentKeyDerivation>,
    ) -> Self {
        Self {
            ledger,
            key_derivation,
            policy: ReconciliationPolicy::new(),
        }
    }

    /// Replace the reconciliation policy (the default is the frozen rule).
    pub fn with_policy(mut self, policy: ReconciliationPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// The active ledger port.
    pub fn ledger(&self) -> &Arc<dyn EffectCommitmentLedger> {
        &self.ledger
    }

    /// Read the snapshot needed to pre-check a resolution.
    async fn read_for_resolution(
        &self,
        input: &ResolveEffectInput,
    ) -> Result<EffectRecord, CommitmentError> {
        let records = self
            .ledger
            .query(EffectQuery::by_id(input.record_id))
            .await?;
        records
            .into_iter()
            .next()
            .ok_or_else(|| CommitmentError::Unavailable {
                detail: format!(
                    "effect record {} not found; refusing to resolve an unknown effect",
                    input.record_id
                ),
            })
    }
}

#[async_trait]
impl EffectCommitmentService for EffectCommitmentServiceImpl {
    async fn begin(&self, input: BeginEffectInput) -> Result<BeginEffectOutput, CommitmentError> {
        let mut intent = input.intent;

        // The key identifies the intent, so the service derives it rather than
        // trusting an attempt-derived key supplied by a caller.
        intent.idempotency_key = self.key_derivation.derive_key(&intent);

        let record = self.ledger.begin(intent).await?;
        Ok(BeginEffectOutput { record })
    }

    async fn settle_definite(
        &self,
        input: SettleDefiniteInput,
    ) -> Result<SettleDefiniteOutput, CommitmentError> {
        let record = self
            .ledger
            .settle_definite(input.record_id, input.outcome)
            .await?;
        Ok(SettleDefiniteOutput { record })
    }

    async fn mark_ambiguous(
        &self,
        input: MarkAmbiguousInput,
    ) -> Result<MarkAmbiguousOutput, CommitmentError> {
        let record = self
            .ledger
            .mark_ambiguous(input.record_id, input.reason)
            .await?;
        Ok(MarkAmbiguousOutput { record })
    }

    async fn resolve(
        &self,
        input: ResolveEffectInput,
    ) -> Result<ResolveEffectOutput, CommitmentError> {
        let existing = self.read_for_resolution(&input).await?;
        self.policy
            .authorize_resolution(&existing, Some(&input.outcome), &input.resolver)?;

        let record = self
            .ledger
            .resolve(input.record_id, input.outcome, input.resolver)
            .await?;
        Ok(ResolveEffectOutput { record })
    }

    async fn query(&self, input: QueryEffectInput) -> Result<QueryEffectOutput, CommitmentError> {
        let records = self.ledger.query(input.query).await?;
        Ok(QueryEffectOutput { records })
    }
}
