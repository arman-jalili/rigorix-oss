//! `FixtureWorld` — the host-supplied world the policy evaluates.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — FixtureWorld, FixtureTranscript
//! Issue: #1294 (contract freeze)
//!
//! The transcript answers port calls: provider determinations, business-sink
//! expectations, approval **claims**. It is data the engine evaluates — never
//! authority (FR-009). It is immutable for the life of a session, and it is equal by
//! digest, which is what makes a replay reproducible.

use serde::{Deserialize, Serialize};

use super::Digest;

/// The raw fixture transcript exactly as the host supplied it.
///
/// # Contract (Frozen)
/// - Stored verbatim; the crate never rewrites, defaults or repairs host data
/// - An incomplete transcript is a typed refusal at open (FR-001), not a patched one
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureTranscript(pub serde_json::Value);

/// An immutable, versioned host-supplied world.
///
/// # Contract (Frozen)
/// - Equal by `transcript_digest`; the transcript is retained for the port adapters
/// - Carries no secret, no absolute path and no personal data (NFR-003)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureWorld {
    /// Canonical digest over the transcript.
    pub transcript_digest: Digest,
    /// The transcript itself.
    pub transcript: FixtureTranscript,
}

impl FixtureWorld {
    /// Pin a transcript and its digest.
    pub fn new(transcript_digest: Digest, transcript: FixtureTranscript) -> Self {
        Self {
            transcript_digest,
            transcript,
        }
    }
}
