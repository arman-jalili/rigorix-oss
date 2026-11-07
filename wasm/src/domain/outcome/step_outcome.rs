//! `StepOutcome` — one gated step's decision.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — StepOutcome, StepDecision
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

use super::{EvidenceRef, RefusalKind};

/// The decision for one step.
///
/// # Contract (Frozen)
/// - Three values; `NotReached` is **not** `Refused` — the gate never evaluated the
///   step, and saying so is the difference between a record and a summary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepDecision {
    /// The step was permitted.
    Allowed,
    /// The step was promoted: it may not proceed until a human approves it.
    ///
    /// Distinct from `Allowed` (which proceeds) and from `Refused` (which is final). A run
    /// carrying one must not be reported as green — nothing was decided yet.
    Promoted,
    /// The step was evaluated and denied.
    Refused,
    /// The step was not evaluated because an earlier step refused.
    NotReached,
}

/// The decision for one gated step, in engine evaluation order.
///
/// # Contract (Frozen)
/// - Ordered exactly as the engine evaluated (criterion 9)
/// - A `Refused` carries its `kind`; `Allowed` and `NotReached` do not
/// - `reason` is business wording from [`RefusalVocabulary`](crate::domain::services::RefusalVocabulary),
///   never a raw engine string and never a parameter value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepOutcome {
    /// Stable identifier of the gate that produced this decision.
    pub gate_id: String,
    /// The decision.
    pub decision: StepDecision,
    /// Why, when refused.
    pub kind: Option<RefusalKind>,
    /// Business-worded reason, when refused.
    pub reason: Option<String>,
    /// The evidence this decision produced, when it produced any.
    pub evidence: Option<EvidenceRef>,
    /// The operands the engine computed, when the refusal has any (#1462).
    ///
    /// **Not evidence.** An [`EvidenceRef`] is a pointer to bytes and stays that way; this is
    /// the decision record saying *what the numbers were*, which is the difference between
    /// "capacity exceeded" and the sentence a form validator produces. Absent on `Allowed` and
    /// on `NotReached`: a step that was never evaluated has no operands to report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<RefusalDetail>,
}

/// The operands a refusal can name, typed rather than free JSON (#1462).
///
/// A host renders these; it must not have to parse the engine's payload to find them, and it
/// must never be handed a `0` in place of a number the engine did not compute — a zero overage
/// reads as a pass. The projection therefore produces `None` rather than a default when an
/// operand is missing, and says so by refusing with a port-failure kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RefusalDetail {
    /// What the declared cap refused, and by how much.
    Capacity {
        /// The amount this attempt asked for.
        requested: u64,
        /// What the resource would total if it were admitted (`committed + outstanding + requested`).
        claimed: u64,
        /// The configured cap.
        hard_limit: u64,
        /// `claimed - hard_limit`: how much over the request goes.
        overage: u64,
    },
    /// Which prior commitment a repeated effect collided with.
    DuplicateEffect {
        /// The provider's record identity.
        record_id: String,
        /// RFC 3339 instant the earlier commitment happened.
        instant: String,
    },
}

impl StepOutcome {
    /// Whether this step stopped the run.
    pub fn is_refusal(&self) -> bool {
        matches!(self.decision, StepDecision::Refused)
    }

    /// Whether this step is waiting for a human.
    ///
    /// `is_refusal()` is false for a promotion and `is_allowed()`-style checks are false too:
    /// a caller asking "did anything stop?" must ask this as well, or a paused run reads as
    /// a completed one.
    pub fn awaits_approval(&self) -> bool {
        matches!(self.decision, StepDecision::Promoted)
    }
}
