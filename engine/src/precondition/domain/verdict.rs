//! PreconditionVerdict — the dispatch-gate decision for one about-to-dispatch
//! node (ADR-017 §R1).
//!
//! @canonical .pi/architecture/modules/precondition.md#dispatch-integration
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — PreconditionVerdict (Dispatch | Deny)
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-4 (PreconditionService)
//!
//! `PreconditionService` returns exactly one of two verdicts:
//!
//! - `Dispatch` — every matching precondition passed; the choke point may call
//!   `spawn_concurrent_node`
//! - `Deny { precondition_id, outcome }` — a matching precondition refused; the
//!   tool is **never** called and the node is marked failed
//!
//! # Contract (Frozen)
//! - A `Deny` always names the precondition that refused and the distinct
//!   outcome (`failed` | `error`)
//! - The verdict is the **only** thing the dispatch gate branches on: it is
//!   deterministic, serializable, and carries no parameter values

use serde::{Deserialize, Serialize};

use super::finding::PreconditionOutcome;

/// The decision produced by precondition evaluation for one node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum PreconditionVerdict {
    /// All matching preconditions passed (or none matched) — dispatch.
    Dispatch,
    /// A matching precondition refused — do not call the tool; mark the node
    /// failed.
    Deny {
        /// Stable id of the precondition that refused.
        precondition_id: String,
        /// The distinct outcome that caused the refusal.
        outcome: PreconditionOutcome,
    },
}

impl PreconditionVerdict {
    /// Whether the node may dispatch.
    pub fn allows_dispatch(&self) -> bool {
        matches!(self, PreconditionVerdict::Dispatch)
    }

    /// The refusing precondition id, when the verdict refuses.
    pub fn denying_precondition_id(&self) -> Option<&str> {
        match self {
            PreconditionVerdict::Dispatch => None,
            PreconditionVerdict::Deny {
                precondition_id, ..
            } => Some(precondition_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_allows_and_deny_refuses() {
        assert!(PreconditionVerdict::Dispatch.allows_dispatch());
        let deny = PreconditionVerdict::Deny {
            precondition_id: "beneficiary-eligible".to_string(),
            outcome: PreconditionOutcome::Failed,
        };
        assert!(!deny.allows_dispatch());
        assert_eq!(deny.denying_precondition_id(), Some("beneficiary-eligible"));
    }

    #[test]
    fn verdict_is_serializable() {
        let verdict = PreconditionVerdict::Deny {
            precondition_id: "beneficiary-eligible".to_string(),
            outcome: PreconditionOutcome::Error,
        };
        let encoded = serde_json::to_string(&verdict).expect("serialize");
        let decoded: PreconditionVerdict = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, verdict);
    }
}
