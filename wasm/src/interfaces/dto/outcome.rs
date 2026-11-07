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
/// - `decision_canonical` / `decision_digest` describe **the engine's** decision, and a host
///   that signs, stores or displays a digest computes nothing (#1442)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunOutcomeDto {
    /// `allowed` or `refused`.
    pub status: String,
    /// Ordered step decisions.
    pub steps: Vec<StepOutcomeDto>,
    /// Evidence references.
    pub evidence: Vec<EvidenceRefDto>,
    /// Base64 of the engine's canonical decision bytes, byte for byte.
    ///
    /// `None` when no engine decided — a run refused before the gate. Absent rather than an
    /// empty string: a host must be able to tell "there is no record" from "the record is
    /// empty", and an empty string says the second.
    pub decision_canonical: Option<String>,
    /// `sha256:<64 hex>` over those same bytes, computed in-crate (#1442).
    ///
    /// Same condition as `decision_canonical`: the two fields describe one byte string, and
    /// a digest that could disagree with the bytes beside it would be a second claim.
    pub decision_digest: Option<String>,
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
    /// The operands a refusal names, when it names any (#1462).
    ///
    /// Absent on `allowed` and on `not_reached`, and absent when the engine's operands did not
    /// survive — in which case the refusal is a `port_unavailable`, never a zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<RefusalDetailDto>,
}

/// The operands of a refusal, on the wire.
///
/// Typed rather than free JSON: a host renders these, and a host that had to parse the
/// engine's payload would be re-deriving a decision the engine already made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RefusalDetailDto {
    /// A declared cap refused this consumption.
    Capacity {
        /// The amount this attempt asked for.
        requested: u64,
        /// What the resource would total if it were admitted.
        claimed: u64,
        /// The configured cap.
        hard_limit: u64,
        /// How much over the request goes.
        overage: u64,
    },
    /// The same effect has already committed.
    DuplicateEffect {
        /// The earlier commitment's record identity.
        record_id: String,
        /// When it happened, RFC 3339.
        instant: String,
    },
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
