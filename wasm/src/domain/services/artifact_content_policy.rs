//! `ArtifactContentPolicy` — refuse an export that would leak.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! Implements: Contract Freeze — ArtifactContentPolicy, ContentRule
//! Issue: #1294 (contract freeze)
//!
//! The check runs **before** bytes leave and it **refuses**; it never sanitises. A
//! scrubbed record is a silently altered record, and an altered record is not evidence.
//!
//! # Contract (Frozen)
//! - Runs on the assembled artifact set, not on a sample
//! - Returns the first violated rule; the caller turns it into `SealError::ContentRejected`
//! - Detects at least: key material, absolute host paths, recognised personal data
//!   (criterion 7, NFR-003)

use crate::domain::artifact::ArtifactSet;

/// Which rule an export violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentRule {
    /// Key material or a private key block.
    KeyMaterial,
    /// An absolute filesystem path.
    AbsolutePath,
    /// Recognised personal data (email, phone, national id shape).
    PersonalData,
    /// A credential-shaped token.
    Credential,
}

impl ContentRule {
    /// Stable label for logs and tests. Never contains the offending value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KeyMaterial => "key_material",
            Self::AbsolutePath => "absolute_path",
            Self::PersonalData => "personal_data",
            Self::Credential => "credential",
        }
    }
}

impl std::fmt::Display for ContentRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Decides whether a sealed set may leave the crate.
pub trait ArtifactContentPolicy {
    /// Check the whole set.
    fn check(&self, artifact: &ArtifactSet) -> Result<(), ContentRule>;
}
