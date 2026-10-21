//! ObservedOutcome — the only thing that may settle an effect.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#observedoutcome
//! Implements: ISSUE-CONTRACT-FREEZE — provider evidence value object
//! Issue: #1113
//! ADR-019: authority for commitment is external to the engine
//!
//! Evidence from the provider: receipt id, read-back digest, observed
//! timestamp, source identity, and the verdict the read-back proved.
//!
//! # Contract (Frozen)
//! - A local guess is **not** an `ObservedOutcome`
//! - An outcome carries an explicit verdict (`Committed` or `NotCommitted`);
//!   absence of evidence is `Ambiguous`, never `NotCommitted`
//! - `source` identifies the provider/read-back surface, not the engine

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::EffectState;

/// The verdict proved by a provider read-back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeVerdict {
    /// The provider observed the effect take place.
    Committed,
    /// The provider observed the effect definitively not take place.
    NotCommitted,
}

impl OutcomeVerdict {
    /// The terminal `EffectState` this verdict authorises.
    pub fn terminal_state(&self) -> EffectState {
        match self {
            Self::Committed => EffectState::Committed,
            Self::NotCommitted => EffectState::NotCommitted,
        }
    }
}

/// Provider evidence about one effect.
///
/// # Fields (Frozen)
/// - `receipt_id: String` — provider receipt identity
/// - `readback_digest: String` — digest of the provider read-back payload
/// - `observed_at: DateTime<Utc>` — when the provider observed it
/// - `source: String` — the provider/read-back surface that produced it
/// - `verdict: OutcomeVerdict` — what the read-back proved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedOutcome {
    /// Provider receipt identity.
    pub receipt_id: String,

    /// Digest of the provider read-back payload.
    pub readback_digest: String,

    /// When the provider observed the outcome.
    pub observed_at: DateTime<Utc>,

    /// The provider/read-back surface that produced this evidence.
    pub source: String,

    /// What the read-back proved.
    pub verdict: OutcomeVerdict,
}

impl ObservedOutcome {
    /// Evidence that the provider observed the effect commit.
    pub fn committed(
        receipt_id: impl Into<String>,
        readback_digest: impl Into<String>,
        observed_at: DateTime<Utc>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            readback_digest: readback_digest.into(),
            observed_at,
            source: source.into(),
            verdict: OutcomeVerdict::Committed,
        }
    }

    /// Evidence that the provider observed the effect definitively not commit.
    pub fn not_committed(
        receipt_id: impl Into<String>,
        readback_digest: impl Into<String>,
        observed_at: DateTime<Utc>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            readback_digest: readback_digest.into(),
            observed_at,
            source: source.into(),
            verdict: OutcomeVerdict::NotCommitted,
        }
    }

    /// Whether the provider proved the effect committed.
    pub fn is_committed(&self) -> bool {
        self.verdict == OutcomeVerdict::Committed
    }

    /// The terminal state this evidence authorises.
    pub fn terminal_state(&self) -> EffectState {
        self.verdict.terminal_state()
    }
}
