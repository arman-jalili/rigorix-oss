//! `ExportArtifactSet` — seal, stamp and scan before bytes leave (FR-005, FR-010, FR-014).
//!
//! @canonical .pi/architecture/modules/portable-session.md#application-services-from-exploration-use-cases
//! Implements: `ExportArtifactSet` (AC 6, 7, 11, 12)
//! Issue: #1297 (application layer)
//!
//! # Contract
//! - Serializes the **engine's** envelopes; never re-encodes them (ADR-010)
//! - Stamps `artifact_format_version`, `engine_version`, scenario and fixture digests,
//!   and the outcome digest (criterion 11)
//! - Scans for secrets, absolute paths and personal data **before** the bytes leave, and
//!   **refuses** rather than sanitising (criterion 7, NFR-003)
//! - Says nothing about verification: the crate never emits `not_verified` or
//!   `unverifiable` (ADR-013, criterion 12)

use crate::domain::artifact::ArtifactSet;
use crate::domain::errors::SealError;
use crate::domain::services::{ArtifactContentPolicy, ArtifactStamp};
use crate::domain::session::ReplaySession;

/// Seals the run into an artifact set.
pub trait ExportArtifactSet {
    /// Export.
    fn execute(&self, session: &mut ReplaySession) -> Result<ArtifactSet, SealError>;
}

/// The shipped implementation.
pub struct ExportArtifactSetService<S: ArtifactStamp, P: ArtifactContentPolicy> {
    stamp: S,
    policy: P,
}

impl<S: ArtifactStamp, P: ArtifactContentPolicy> ExportArtifactSetService<S, P> {
    /// Construct the service with its stamp and content policy.
    pub const fn new(stamp: S, policy: P) -> Self {
        Self { stamp, policy }
    }
}

impl<S: ArtifactStamp, P: ArtifactContentPolicy> ExportArtifactSet
    for ExportArtifactSetService<S, P>
{
    fn execute(&self, session: &mut ReplaySession) -> Result<ArtifactSet, SealError> {
        session.seal(&self.stamp, &self.policy)
    }
}
