//! `ScanningContentPolicy` — the pre-export scan (NFR-003, criterion 7).
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! Implements: Contract Freeze — ScanningContentPolicy
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Refuses, never sanitises: a scrubbed record is an altered record
//! - Reports the violated rule, never the offending value (the value would be the leak)
//! - Runs on every export, including a re-export of a previously sealed set

use crate::domain::artifact::ArtifactSet;
use crate::domain::services::{ArtifactContentPolicy, ContentRule};

/// Scans the assembled bytes for secrets, absolute paths and personal data.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScanningContentPolicy;

impl ScanningContentPolicy {
    /// Construct the policy.
    pub const fn new() -> Self {
        Self
    }
}

impl ArtifactContentPolicy for ScanningContentPolicy {
    fn check(&self, artifact: &ArtifactSet) -> Result<(), ContentRule> {
        let _ = artifact;
        todo!("contract freeze: ScanningContentPolicy::check lands with #1297")
    }
}
