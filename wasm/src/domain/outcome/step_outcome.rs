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
}

impl StepOutcome {
    /// Whether this step stopped the run.
    pub fn is_refusal(&self) -> bool {
        matches!(self.decision, StepDecision::Refused)
    }
}
