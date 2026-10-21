//! ReconciliationPolicy — F3/F4 as a pure function.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#reconciliationpolicy
//! Implements: ISSUE-CONTRACT-FREEZE — only provider evidence may leave Ambiguous
//! Issue: #1113
//! ADR-019: F3 (evidence required) and F4 (who may resolve)
//!
//! Encodes the rule the whole context exists for:
//!
//! - only a resolver presenting an [`ObservedOutcome`] may leave `Ambiguous`
//! - the dispatching process and the agent are never that resolver
//!
//! Pure function, no I/O, so the rule is testable without a database.
//!
//! # Contract (Frozen)
//! - A `Provider` or `ExternalOperator` resolver presenting evidence is allowed
//! - `DispatchingProcess` and `Agent` are rejected as `UnauthorizedResolver`
//! - Missing evidence is `EvidenceMissing`, never an implicit `NotCommitted`

use crate::effect_commitment::domain::{
    CommitmentError, EffectRecord, EffectState, ObservedOutcome, ResolverIdentity,
};

/// The frozen reconciliation rule.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReconciliationPolicy;

impl ReconciliationPolicy {
    /// Construct the stateless policy.
    pub const fn new() -> Self {
        Self
    }

    /// Authorise leaving `Ambiguous` with `evidence` and `resolver`.
    ///
    /// # Errors
    /// - [`CommitmentError::NotAmbiguous`] when the record is not `Ambiguous`
    /// - [`CommitmentError::EvidenceMissing`] when no provider evidence is given
    /// - [`CommitmentError::UnauthorizedResolver`] when the resolver is the
    ///   dispatching process or the agent (F4)
    pub fn authorize_resolution(
        &self,
        record: &EffectRecord,
        evidence: Option<&ObservedOutcome>,
        resolver: &ResolverIdentity,
    ) -> Result<(), CommitmentError> {
        if record.state() != EffectState::Ambiguous {
            return Err(CommitmentError::NotAmbiguous {
                record_id: record.id(),
                state: record.state(),
            });
        }

        let Some(evidence) = evidence else {
            return Err(CommitmentError::EvidenceMissing {
                record_id: record.id(),
            });
        };

        if !resolver.is_authorized() {
            return Err(CommitmentError::UnauthorizedResolver {
                resolver_id: resolver.resolver_id.clone(),
                reason: format!(
                    "resolver kind '{:?}' cannot prove an external outcome (F4)",
                    resolver.kind
                ),
            });
        }

        // The resolver must present *some* evidence digest; an empty digest is
        // an unrecorded claim, which is the same as no evidence.
        if resolver.evidence_digest.trim().is_empty() {
            return Err(CommitmentError::EvidenceMissing {
                record_id: record.id(),
            });
        }

        if evidence.readback_digest.trim().is_empty() {
            return Err(CommitmentError::EvidenceMissing {
                record_id: record.id(),
            });
        }

        Ok(())
    }

    /// Whether `to` is a legal successor of `from`.
    ///
    /// `Ambiguous -> Dispatched` is illegal: a retry while unsettled is exactly
    /// the double-spend path this context closes.
    pub fn is_legal_transition(&self, from: EffectState, to: EffectState) -> bool {
        matches!(
            (from, to),
            (EffectState::Dispatched, EffectState::Committed)
                | (EffectState::Dispatched, EffectState::NotCommitted)
                | (EffectState::Dispatched, EffectState::Ambiguous)
                | (EffectState::Ambiguous, EffectState::Committed)
                | (EffectState::Ambiguous, EffectState::NotCommitted)
        )
    }
}
