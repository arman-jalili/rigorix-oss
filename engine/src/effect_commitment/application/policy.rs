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

    /// Authorise a definite settlement of a dispatched effect.
    ///
    /// The state machine and the evidence completeness rule are checked here so
    /// every adapter reaches the same verdict before mutating the record.
    ///
    /// # Errors
    /// - [`CommitmentError::AlreadySettled`] when the record is terminal
    /// - [`CommitmentError::NotAmbiguous`] when it is `Ambiguous` (resolve it)
    /// - [`CommitmentError::EvidenceMissing`] for incomplete evidence
    pub fn authorize_settlement(
        &self,
        record: &EffectRecord,
        evidence: &ObservedOutcome,
    ) -> Result<(), CommitmentError> {
        if record.is_terminal() {
            return Err(CommitmentError::AlreadySettled {
                record_id: record.id(),
                state: record.state(),
            });
        }
        if record.state() == EffectState::Ambiguous {
            return Err(CommitmentError::NotAmbiguous {
                record_id: record.id(),
                state: record.state(),
            });
        }
        if !evidence.is_complete() {
            return Err(CommitmentError::EvidenceMissing {
                record_id: record.id(),
            });
        }
        Ok(())
    }

    /// Authorise marking a dispatched effect ambiguous.
    ///
    /// # Errors
    /// - [`CommitmentError::AlreadySettled`] when the record is terminal
    /// - [`CommitmentError::Contended`] when it is already `Ambiguous`
    pub fn authorize_mark_ambiguous(&self, record: &EffectRecord) -> Result<(), CommitmentError> {
        if record.is_terminal() {
            return Err(CommitmentError::AlreadySettled {
                record_id: record.id(),
                state: record.state(),
            });
        }
        if record.is_ambiguous() {
            return Err(CommitmentError::Contended {
                record_id: Some(record.id()),
                detail: "effect is already ambiguous".to_string(),
            });
        }
        Ok(())
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
        if !resolver.has_evidence_digest() {
            return Err(CommitmentError::EvidenceMissing {
                record_id: record.id(),
            });
        }

        if !evidence.is_complete() {
            return Err(CommitmentError::EvidenceMissing {
                record_id: record.id(),
            });
        }

        Ok(())
    }

    /// Whether `to` is a legal successor of `from`.
    ///
    /// The transition graph lives on [`EffectState`]; this policy method is the
    /// single place reconciliation callers consult.
    pub fn is_legal_transition(&self, from: EffectState, to: EffectState) -> bool {
        from.can_transition_to(to)
    }
}
