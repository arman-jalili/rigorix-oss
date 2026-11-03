//! `VersionStamp` — which engine, which format, which scenario.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! Implements: Contract Freeze — VersionStamp
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

use crate::domain::value::Digest;

/// The version identity stamped into every export.
///
/// # Contract (Frozen)
/// - `artifact_format_version` is this crate's container version; a change to the
///   container bumps it
/// - `engine_version` is copied verbatim from `rigorix_engine::ENGINE_VERSION`; the
///   crate holds no local copy (criterion 11)
/// - `scenario_digest` names the scenario, so a record cannot be re-read against a
///   different policy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionStamp {
    /// Container format version.
    pub artifact_format_version: u32,
    /// Engine version that produced the envelopes.
    pub engine_version: String,
    /// Digest of the scenario the run used.
    pub scenario_digest: Digest,
}
