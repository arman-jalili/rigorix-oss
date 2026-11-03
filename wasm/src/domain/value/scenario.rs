//! `ScenarioRef` — a pointer to an engine scenario, never a re-typed policy.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — ScenarioRef, ScenarioFormatVersion
//! Issue: #1294 (contract freeze)
//!
//! The scenario is authored in the **engine's** template and sequence-policy format.
//! This crate references it by digest and format version so that (a) it cannot drift
//! into a second policy language and (b) an export can name exactly which scenario
//! produced it.

use serde::{Deserialize, Serialize};

use super::Digest;

/// Version of the engine scenario format this crate was built against.
///
/// # Contract (Frozen)
/// - Serialised as an integer; an unknown value is a refusal, never a guess
///   (criterion 12, FR-010)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScenarioFormatVersion(pub u16);

impl std::fmt::Display for ScenarioFormatVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A digest-bearing reference to an engine scenario.
///
/// # Contract (Frozen)
/// - Equal by `digest` **and** `format_version`
/// - Carries no policy content: the crate never parses rules of its own
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioRef {
    /// Digest of the scenario as parsed by the engine.
    pub digest: Digest,
    /// Declared engine format version.
    pub format_version: ScenarioFormatVersion,
}

impl ScenarioRef {
    /// Build a reference from a digest and a declared format version.
    ///
    /// TODO(#1294): reject a format version the seam does not support.
    pub fn new(digest: Digest, format_version: ScenarioFormatVersion) -> Self {
        Self {
            digest,
            format_version,
        }
    }
}
