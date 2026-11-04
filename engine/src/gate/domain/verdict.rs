//! The verdict: dispatch, wait for a human, or do not happen.
//!
//! Three outcomes, not two — `Promote` folded into `Passed` was #1392.
//! @canonical .pi/architecture/modules/gate.md

use serde::{Deserialize, Serialize};

use super::finding::GateId;

/// The verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    /// Every configured gate passed; the step may dispatch.
    Dispatch,
    /// A gate promoted the step: it may not dispatch until a human approves it.
    ///
    /// The gate reports; the dispatcher acts. Setting `requires_approval` on the plan from
    /// here would write a policy read into the plan, and a plan reused under a changed
    /// policy would carry a stale flag — so the verdict is consulted per dispatch attempt
    /// instead, which is also what makes it correct across a resume.
    Promote {
        /// Which gate promoted it.
        gate: GateId,
        /// The promoting rule's identifier.
        rule_id: String,
        /// Redacted reason.
        reason: String,
    },
    /// A gate refused; the step must not dispatch.
    Refuse {
        /// Which gate refused.
        gate: GateId,
        /// The refusing rule's identifier, when it has one.
        rule_id: Option<String>,
        /// Redacted reason.
        reason: String,
    },
}

impl GateVerdict {
    /// Whether the step may dispatch now.
    ///
    /// A promotion is *not* dispatch: the step is waiting for a human, and a caller that
    /// treats "not refused" as "may proceed" reintroduces the defect this variant removed.
    #[must_use]
    pub fn permits_dispatch(&self) -> bool {
        matches!(self, Self::Dispatch)
    }

    /// Whether the step is waiting for a human.
    #[must_use]
    pub fn awaits_approval(&self) -> bool {
        matches!(self, Self::Promote { .. })
    }
}
