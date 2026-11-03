//! `ArtifactSet` — engine bytes, our metadata, sealed once.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! Implements: Contract Freeze — ArtifactSet, EnvelopeBytes
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

use super::VersionStamp;
use crate::domain::outcome::EvidenceRef;
use crate::domain::value::{Digest, KeyRef, ReplaySessionId};

/// One signed envelope, exactly as the engine emitted it.
///
/// # Contract (Frozen)
/// - Bytes, not a re-serialised structure: re-encoding is how byte identity dies
/// - Never inspected, never rewritten (ADR-010)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeBytes(pub Vec<u8>);

/// The sealed record of one session.
///
/// # Contract (Frozen)
/// - Sealed: no `set_*`, no `&mut` access; a second seal is refused (criterion 6)
/// - Carries no key material, no absolute path and no personal data; the export scan
///   refuses rather than sanitises (criterion 7, NFR-003)
/// - Self-describing: everything a verifier needs is inside
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSet {
    /// Container format, engine version and scenario digest.
    pub stamp: VersionStamp,
    /// Digest of the fixture transcript that produced the run.
    pub fixture_digest: Digest,
    /// Identity of the session.
    pub session_id: ReplaySessionId,
    /// Opaque reference to the signing key.
    pub key_ref: KeyRef,
    /// Signed envelopes, byte for byte.
    pub envelopes: Vec<EnvelopeBytes>,
    /// Evidence references, ordered.
    pub evidence: Vec<EvidenceRef>,
    /// Digest over the run outcome projection.
    pub outcome_digest: Digest,
}

impl ArtifactSet {
    /// The container format this crate emits.
    pub const FORMAT_VERSION: u32 = 1;
}
