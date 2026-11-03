//! `ArtifactStamp` — digests and the version stamp.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! Implements: Contract Freeze — ArtifactStamp
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - `engine_version` comes from the engine, not from `CARGO_PKG_VERSION`: the two are
//!   equal by release policy (ADR-011) and must not be conflated in code
//! - Digests are computed over canonical bytes, so two runs of one session are
//!   byte-identical (NFR-001)
//! - The stamp is produced once, at seal

use crate::domain::artifact::VersionStamp;
use crate::domain::session::ReplaySession;
use crate::domain::value::Digest;

/// Builds the stamp and the outcome digest.
pub trait ArtifactStamp {
    /// Stamp a session for export.
    fn stamp(&self, session: &ReplaySession) -> VersionStamp;

    /// Digest over the run outcome projection.
    fn outcome_digest(&self, session: &ReplaySession) -> Digest;
}
