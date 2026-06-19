//! PolicyAction domain entity.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#action
//! Implements: Contract Freeze — PolicyAction enum
//! Issue: issue-contract-freeze
//!
//! Actions define what the orchestrator does when a policy rule matches.
//! Actions are flat (single action) or compound via `Chain`.
//! The orchestrator iterates the collected action list and dispatches
//! each action to the appropriate handler.
//!
//! # Contract (Frozen)
//! - `Chain` is the only compound action — all others are leaf actions
//! - `Reconcile` carries a `ReconcileReason` enum
//! - Actions carry only the data needed for dispatch, no behavior

use serde::{Deserialize, Serialize};

/// Actions that the orchestrator performs when a policy rule matches.
///
/// # Dispatch
///
/// The orchestrator receives a flat list of `PolicyAction`s (after
/// flattening `Chain`). Actions are executed in order. If an action
/// fails, the orchestrator may retry, skip, or abort depending on
/// the action type and severity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    /// Merge the lane branch into the development/main branch.
    ///
    /// The specific branch is determined by the lane configuration
    /// (e.g., `dev` or `main` target).
    MergeToDev,

    /// Merge changes forward (e.g., from dev → main or staging → production).
    ///
    /// Used in multi-tier branching strategies where changes flow
    /// through multiple merge hops.
    MergeForward,

    /// Attempt recovery of a failed lane execution once.
    ///
    /// Re-triggers the lane execution with the same configuration.
    /// The retry count is tracked by the orchestrator.
    RecoverOnce,

    /// Escalate the lane to a human operator with a reason.
    Escalate {
        /// Human-readable reason for escalation.
        reason: String,
    },

    /// Close out the lane, cleaning up task resources and state.
    CloseoutLane,

    /// Clean up session state (temporary files, locks, etc.).
    CleanupSession,

    /// Reconcile the lane — no merge needed because the lane is
    /// already merged, superseded, has an empty diff, or was
    /// manually closed.
    Reconcile {
        /// The reason the lane can be reconciled.
        reason: ReconcileReason,
    },

    /// Send a notification to a specified channel (e.g., "discord", "slack").
    Notify {
        /// The target channel identifier.
        channel: String,
    },

    /// Block the lane with a reason, preventing further execution.
    Block {
        /// Human-readable reason for the block.
        reason: String,
    },

    /// Execute multiple actions in sequence.
    ///
    /// This is the only compound action. When the engine encounters
    /// a `Chain`, it flattens the contained actions into the output
    /// list in order.
    Chain(Vec<PolicyAction>),
}

impl PolicyAction {
    /// Flatten this action into the provided vector.
    ///
    /// For leaf actions, appends `self` to the vector.
    /// For `Chain` actions, recursively flattens each contained action.
    pub fn flatten_into(self, actions: &mut Vec<PolicyAction>) {
        match self {
            PolicyAction::Chain(inner) => {
                for action in inner {
                    action.flatten_into(actions);
                }
            }
            other => actions.push(other),
        }
    }

    /// Return true if this action is a leaf (not Chain).
    pub fn is_leaf(&self) -> bool {
        !matches!(self, PolicyAction::Chain(_))
    }
}

/// Reasons why a lane can be reconciled (no merge needed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileReason {
    /// The lane's branch was already merged by another process.
    AlreadyMerged,

    /// The lane's work was superseded by a more recent lane.
    Superseded,

    /// The lane has an empty diff (no changes to merge).
    EmptyDiff,

    /// The lane was manually closed by a user.
    ManualClose,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_leaf_action() {
        let mut actions = Vec::new();
        PolicyAction::CloseoutLane.flatten_into(&mut actions);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], PolicyAction::CloseoutLane);
    }

    #[test]
    fn test_flatten_chain() {
        let mut actions = Vec::new();
        let chain = PolicyAction::Chain(vec![
            PolicyAction::CloseoutLane,
            PolicyAction::CleanupSession,
        ]);
        chain.flatten_into(&mut actions);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0], PolicyAction::CloseoutLane);
        assert_eq!(actions[1], PolicyAction::CleanupSession);
    }

    #[test]
    fn test_flatten_nested_chain() {
        let mut actions = Vec::new();
        let chain = PolicyAction::Chain(vec![
            PolicyAction::CloseoutLane,
            PolicyAction::Chain(vec![
                PolicyAction::CleanupSession,
                PolicyAction::Notify {
                    channel: "discord".to_string(),
                },
            ]),
        ]);
        chain.flatten_into(&mut actions);
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn test_is_leaf() {
        assert!(PolicyAction::CloseoutLane.is_leaf());
        assert!(!PolicyAction::Chain(vec![]).is_leaf());
    }

    #[test]
    fn test_reconcile_reason_serde() {
        let reason = ReconcileReason::EmptyDiff;
        let json = serde_json::to_string(&reason).unwrap();
        let deserialized: ReconcileReason = serde_json::from_str(&json).unwrap();
        assert_eq!(reason, deserialized);
    }

    #[test]
    fn test_action_serde_roundtrip() {
        let action = PolicyAction::Escalate {
            reason: "Startup blocked".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: PolicyAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }
}
