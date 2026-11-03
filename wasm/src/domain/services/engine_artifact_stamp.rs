//! `EngineArtifactStamp` — the shipped `ArtifactStamp`.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! @canonical .pi/architecture/decisions/ADR-011-crate-identity-and-release-train.md
//! Implements: `ArtifactStamp` (AC 11)
//! Issue: #1296
//!
//! `engine_version` comes from the engine when the `engine` feature is on, and falls back
//! to this crate's version otherwise — the two are equal by release policy (ADR-011), and
//! the fallback exists only because FR-011 has not landed. It is one function, not a
//! second source of truth: when the seam arrives, only `engine_version()` changes.

use crate::domain::artifact::ArtifactSet;
use crate::domain::artifact::VersionStamp;
use crate::domain::services::ArtifactStamp;
use crate::domain::session::ReplaySession;
use crate::domain::value::Digest;

/// Builds the stamp and the outcome digest from a session.
#[derive(Debug, Default, Clone, Copy)]
pub struct EngineArtifactStamp;

impl EngineArtifactStamp {
    /// Construct the stamp builder.
    pub const fn new() -> Self {
        Self
    }

    /// The engine version that produced the envelopes.
    ///
    /// TODO(#1295..): return `rigorix_engine::ENGINE_VERSION` once the `engine` feature
    /// exists (FR-011). Until then the crates are released in lockstep, so the value is
    /// the same number by ADR-011 — but the *source* is this crate, which is why this is
    /// the one place to change.
    pub fn engine_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

impl ArtifactStamp for EngineArtifactStamp {
    fn stamp(&self, session: &ReplaySession) -> VersionStamp {
        VersionStamp {
            artifact_format_version: ArtifactSet::FORMAT_VERSION,
            engine_version: Self::engine_version().to_string(),
            scenario_digest: session.scenario().digest.clone(),
        }
    }

    fn outcome_digest(&self, session: &ReplaySession) -> Digest {
        // Canonical JSON of the outcome projection; the digest names *what was decided*
        // without carrying the decision into a second place.
        let material = match session.outcome() {
            Some(outcome) => serde_json::to_vec(outcome).unwrap_or_default(),
            None => Vec::new(),
        };
        Digest::of(&material)
    }
}
