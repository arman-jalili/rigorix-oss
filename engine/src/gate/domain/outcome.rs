//! What one gate concluded — four values, because a run's outcome is not binary.
//!
//! @canonical .pi/architecture/modules/gate.md

use serde::{Deserialize, Serialize};

/// What one gate concluded about the step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateOutcome {
    /// The gate ran and did not refuse.
    Passed,
    /// The gate ran and refused.
    Refused,
    /// The gate ran and requires a human before the step may proceed.
    ///
    /// Distinct from both neighbours on purpose: `Passed` would let the step dispatch,
    /// and `Refused` would say the decision is final. A promotion is a decision that is
    /// *not yet made* — and #1392 exists because this third value had no home in the type,
    /// so `matches!(action, Deny)` folded it into `Passed`.
    Promoted,
    /// The gate could not run. Always a refusal: an unavailable check must never
    /// read as a passed one (invariant 2).
    CouldNotEvaluate,
}
