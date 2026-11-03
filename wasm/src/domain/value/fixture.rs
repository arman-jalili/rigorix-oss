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

    /// Pin a transcript from raw host JSON.
    ///
    /// # Contract
    /// - A non-object transcript is `FixtureUnparseable`; there is no empty default,
    ///   because an empty world is indistinguishable from a world with no answers and
    ///   would turn every check into "missing" instead of telling the host what is wrong
    /// - The digest is over the canonical bytes, so equal transcripts are equal worlds
    pub fn identify(transcript: serde_json::Value) -> Result<Self, FixtureWorldError> {
        if !transcript.is_object() {
            return Err(FixtureWorldError::NotAnObject);
        }
        let canonical =
            serde_json::to_vec(&transcript).map_err(|_| FixtureWorldError::NotAnObject)?;
        Ok(Self {
            transcript_digest: Digest::of(&canonical),
            transcript: FixtureTranscript(transcript),
        })
    }
}

/// Why a transcript could not be identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FixtureWorldError {
    /// The value was not a JSON object.
    #[error("the fixture transcript is not a JSON object")]
    NotAnObject,
}
