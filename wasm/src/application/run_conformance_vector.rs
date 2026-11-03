//! `RunConformanceVector` — the proof obligation (FR-007, FR-008, FR-012).
//!
//! @canonical .pi/architecture/decisions/ADR-009-conformance-harness.md
//! Implements: Contract Freeze — RunConformanceVector, ConformanceVector, ConformanceReport
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Test-only: gated behind `cfg(any(test, feature = "conformance"))` and **never**
//!   part of the bundle (NFR-002)
//! - Comparison is **byte-exact**. `normalised_comparison` exists only to label an
//!   interim while FR-011 is unimplemented; a labelled normalised pass is still a
//!   failing claim (FR-012)
//! - A skipped or missing vector is a failure, never a pass (criterion 20)
//! - `SeededDivergence` must diverge; equality is a build failure (criterion 21)

use serde::{Deserialize, Serialize};

/// One committed vector: inputs plus the bytes they must produce.
///
/// # Contract (Frozen)
/// - Immutable once committed; changing a vector is a reviewed artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConformanceVector {
    /// Stable vector id, used in failure messages.
    pub id: String,
    /// Scenario JSON, in the engine's format.
    pub scenario: serde_json::Value,
    /// Fixture transcript JSON.
    pub fixtures: serde_json::Value,
    /// Clock readings JSON.
    pub clock: serde_json::Value,
    /// Seed JSON.
    pub seed: serde_json::Value,
    /// Expected artifact set bytes.
    pub expected: Vec<u8>,
    /// Whether this vector is the deliberately-diverging one (FR-008).
    pub seeded_divergence: bool,
}

/// What a replay concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// Vector id.
    pub vector_id: String,
    /// Whether the bytes matched.
    pub matched: bool,
    /// Whether the comparison was normalised rather than byte-exact (interim label).
    pub normalised_comparison: bool,
    /// Whether the vector was skipped. A skip is a failure.
    pub skipped: bool,
}

impl ConformanceReport {
    /// Whether this report satisfies the contract for its vector.
    ///
    /// A seeded-divergence vector passes only when it **did not** match; every other
    /// vector passes only when it did. A skip never passes.
    pub fn is_pass(&self, seeded_divergence: bool) -> bool {
        !self.skipped && self.matched != seeded_divergence
    }
}

/// Replays vectors and compares bytes.
pub trait RunConformanceVector {
    /// Replay one vector.
    fn execute(&self, vector: &ConformanceVector) -> ConformanceReport;
}
