//! CommitmentError — the closed failure vocabulary of Effect Commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#commitmenterror
//! Implements: ISSUE-CONTRACT-FREEZE — typed failures + wire error codes
//! Issue: #1113
//!
//! # Contract (Frozen)
//! - Exactly six variants:
//!   `AlreadySettled | NotAmbiguous | UnauthorizedResolver | EvidenceMissing |
//!    Unavailable | Contended`
//! - `Unavailable` is the fail-closed bucket for every transport ambiguity
//! - No `anyhow` / stringly-typed error crosses this boundary
//! - `code()` is the stable wire discriminator used by the HTTP error body

use thiserror::Error;

use super::{EffectRecordId, EffectState};

/// Typed failures for commitment operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommitmentError {
    /// The effect already reached a terminal state.
    #[error("effect {record_id} is already settled ({state})")]
    AlreadySettled {
        /// The effect that could not move.
        record_id: EffectRecordId,
        /// The terminal state it is in.
        state: EffectState,
    },

    /// The operation only applies to an `Ambiguous` effect.
    #[error("effect {record_id} is not ambiguous ({state})")]
    NotAmbiguous {
        /// The effect in the wrong state.
        record_id: EffectRecordId,
        /// Its current state.
        state: EffectState,
    },

    /// The resolver is the dispatching process or the agent (F4).
    #[error("resolver '{resolver_id}' is not authorised: {reason}")]
    UnauthorizedResolver {
        /// The rejected resolver.
        resolver_id: String,
        /// Why it is rejected.
        reason: String,
    },

    /// An `Ambiguous` effect cannot be settled without provider evidence (F3).
    #[error("effect {record_id} has no observed outcome")]
    EvidenceMissing {
        /// The effect missing evidence.
        record_id: EffectRecordId,
    },

    /// The ledger (or its backing store) could not be reached.
    ///
    /// Every ambiguous transport outcome maps here so the caller fails closed.
    #[error("effect commitment ledger is unavailable: {detail}")]
    Unavailable {
        /// Human-readable detail for diagnostics.
        detail: String,
    },

    /// Two writers contended for the same record and the operation was lost.
    #[error("effect commitment contention: {detail}")]
    Contended {
        /// The contended record, when known.
        record_id: Option<EffectRecordId>,
        /// Human-readable detail for diagnostics.
        detail: String,
    },
}

impl CommitmentError {
    /// Stable wire code for the HTTP error body.
    pub fn code(&self) -> &'static str {
        match self {
            Self::AlreadySettled { .. } => "already_settled",
            Self::NotAmbiguous { .. } => "not_ambiguous",
            Self::UnauthorizedResolver { .. } => "unauthorized_resolver",
            Self::EvidenceMissing { .. } => "evidence_missing",
            Self::Unavailable { .. } => "unavailable",
            Self::Contended { .. } => "contended",
        }
    }

    /// Whether retrying with the same intent may succeed.
    ///
    /// Contention and outages are transient; a settled record, a wrong state,
    /// a rejected resolver, or missing evidence are not fixed by retrying the
    /// identical request.
    pub fn is_retriable(&self) -> bool {
        matches!(self, Self::Contended { .. } | Self::Unavailable { .. })
    }

    /// Whether the ledger itself could not be reached (an outage).
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable { .. })
    }

    /// The affected record, when the variant carries one.
    ///
    /// This is the single source of truth for the `record_id` field of the
    /// frozen HTTP error body.
    pub fn record_id(&self) -> Option<EffectRecordId> {
        match self {
            Self::AlreadySettled { record_id, .. }
            | Self::NotAmbiguous { record_id, .. }
            | Self::EvidenceMissing { record_id } => Some(*record_id),
            Self::Contended { record_id, .. } => *record_id,
            Self::UnauthorizedResolver { .. } | Self::Unavailable { .. } => None,
        }
    }
}
