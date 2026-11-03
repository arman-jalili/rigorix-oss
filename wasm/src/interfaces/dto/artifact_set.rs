//! `ArtifactSetDto` — the exported record, on the wire.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! Implements: Contract Freeze — ArtifactSetDto
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

/// The sealed container a host can keep, share or verify.
///
/// # Contract (Frozen)
/// - Self-describing: the three version numbers plus both digests are inside
/// - Envelopes are base64 or hex text of the engine's bytes, never re-encoded
///   structures (ADR-010)
/// - No key material, no absolute path, no personal data (criterion 7)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSetDto {
    /// Container format version.
    pub artifact_format_version: u32,
    /// Engine version that produced the envelopes.
    pub engine_version: String,
    /// Scenario digest.
    pub scenario_digest: String,
    /// Fixture transcript digest.
    pub fixture_digest: String,
    /// Session identity.
    pub session_id: String,
    /// Opaque key reference.
    pub key_ref: String,
    /// Signed envelopes, encoded.
    pub envelopes: Vec<String>,
    /// Evidence references.
    pub evidence: Vec<super::outcome::EvidenceRefDto>,
    /// Digest over the run outcome.
    pub outcome_digest: String,
}
