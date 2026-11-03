//! `RunOutcomeDto` and `StepOutcomeDto` — what the host receives.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! Implements: Contract Freeze — RunOutcomeDto, StepOutcomeDto
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

/// The run's result, on the wire.
///
/// # Contract (Frozen)
/// - `status` is `allowed` or `refused`; there is no third value
/// - `steps` preserves engine order and includes `not_reached` entries
/// - `evidence` holds references (digests), never evidence bodies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunOutcomeDto {
    /// `allowed` or `refused`.
    pub status: String,
    /// Ordered step decisions.
    pub steps: Vec<StepOutcomeDto>,
    /// Evidence references.
    pub evidence: Vec<EvidenceRefDto>,
}

/// One step decision, on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepOutcomeDto {
    /// The gate that decided.
    pub gate_id: String,
    /// `allowed`, `refused` or `not_reached`.
    pub decision: String,
    /// The engine-aligned refusal kind, when refused.
    pub kind: Option<String>,
    /// Business-worded reason, when refused.
    pub reason: Option<String>,
    /// Evidence for this decision, when produced.
    pub evidence: Option<EvidenceRefDto>,
}

/// A reference to evidence, on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRefDto {
    /// `sha256:<hex>`.
    pub digest: String,
    /// Evidence kind.
    pub kind: String,
}
