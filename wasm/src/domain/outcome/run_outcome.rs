//! `RunOutcome` — the single result of an attempt.
//!
//! @canonical .pi/architecture/modules/portable-session.md#aggregates
//! Implements: Contract Freeze — RunOutcome, OutcomeStatus
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

use super::{EvidenceRef, StepOutcome};

/// The status of a run.
///
/// # Contract (Frozen)
/// - Exactly two values. There is no `Unknown`, no `Pending` and no
///   host-settable override (FR-002, FR-009)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStatus {
    /// The gate permitted every step.
    Allowed,
    /// The gate refused; at least one step carries the refusal.
    Refused,
}

/// The result of one attempt.
///
/// # Contract (Frozen)
/// - A `Refused` status always has a causing `StepOutcome`; a run-level refusal
///   without a step is unrepresentable (criterion 10, FR-002)
/// - `steps` preserves engine order, including `NotReached` entries
/// - `evidence` lists every reference the run produced, ordered
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunOutcome {
    /// Allowed or refused.
    pub status: OutcomeStatus,
    /// Ordered step decisions.
    pub steps: Vec<StepOutcome>,
    /// Every evidence reference the run produced.
    pub evidence: Vec<EvidenceRef>,
}

impl RunOutcome {
    /// The first step that refused, if any.
    pub fn causing_step(&self) -> Option<&StepOutcome> {
        self.steps.iter().find(|step| step.is_refusal())
    }
}
