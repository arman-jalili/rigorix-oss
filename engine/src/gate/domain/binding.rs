//! The decision's identity in the form a signer needs, and the record it produces.
//!
//! The engine decides, the runtime signs (ADR-013).
//! @canonical .pi/architecture/modules/gate.md

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The decision's identity, in the form a signer needs.
///
/// # Why a digest and not an envelope
///
/// `BuildEnvelopeInput` describes a *run*: template, planning prompt, token totals,
/// duration, git commit. A gate decision knows none of those and must not invent
/// them, so the gate publishes what it does own — the canonical bytes of the decision
/// and their digest — and the runtime that holds the run context wraps it. Same
/// division as the crate: the engine decides, the runtime signs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateEnvelopeBinding {
    /// The execution this decision belongs to (from `PinnedInputs`).
    pub execution_id: Uuid,
    /// SHA-256 (hex) of the decision's canonical bytes.
    ///
    /// The bytes are the JSON encoding of `{verdict, findings}` in declaration order
    /// with no timestamps of their own, so two evaluations of the same input produce
    /// this same value — which is the determinism claim, as a value rather than a
    /// sentence.
    pub decision_digest: String,
    /// The instant the decision was evaluated with (ADR-014 §2), if pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_timestamp: Option<DateTime<Utc>>,
    /// Whether every input was pinned. A caller that signs an unpinned decision is
    /// signing a decision it cannot replay.
    pub pinned: bool,
}

/// The signed record of a decision, when the caller supplied a [`RunContext`].
///
/// Equality is by the bytes and the hash, not by the envelope's own derives: two records
/// are the same record when a verifier would see the same bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEnvelopeEvidence {
    /// The envelope the engine built and signed.
    pub envelope: crate::audit::domain::AuditEnvelope,
    /// Its canonical bytes — the exact bytes a signature covers (ADR-010).
    pub canonical_bytes: Vec<u8>,
    /// Its hash, as a verifier computes it.
    pub envelope_hash: String,
}
