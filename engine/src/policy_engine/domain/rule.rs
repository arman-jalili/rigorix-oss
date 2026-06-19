//! PolicyRule domain entity.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#rule
//! Implements: Contract Freeze — PolicyRule struct
//! Issue: issue-contract-freeze
//!
//! A single policy rule with a name, composable condition, action, and
//! priority. Rules are evaluated in ascending priority order — lower
//! numbers have higher priority and are evaluated first.
//!
//! # Contract (Frozen)
//! - `name` is a unique identifier for the rule
//! - `condition` must evaluate to a boolean when tested against `LaneContext`
//! - `action` is the executable action taken when the condition matches
//! - `priority` determines evaluation order (lower = higher priority)

use serde::{Deserialize, Serialize};

use super::{action::PolicyAction, condition::PolicyCondition};

/// A single policy rule with a name, composable condition, action, and priority.
///
/// # Priority Ordering
///
/// Rules are evaluated in ascending priority order. A rule with `priority: 1`
/// is evaluated before a rule with `priority: 10`. If multiple rules match,
/// all matching actions are collected and returned in priority order.
///
/// # Design Notes
///
/// - `PolicyRule` is the atomic building block of the policy engine
/// - Rules are designed to be serializable from `.rigorix/policy.toml`
/// - The priority field allows users to control rule ordering
/// - No implicit rule ordering — priority is always explicit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Unique human-readable name for this rule (e.g., "closeout-completed-lane").
    pub name: String,

    /// The composable condition that determines if this rule matches.
    ///
    /// Evaluated against a `LaneContext`. May be a single condition or a
    /// tree of And/Or conditions.
    pub condition: PolicyCondition,

    /// The action to take when the condition matches.
    ///
    /// Supports singular actions (e.g., `MergeToDev`, `CloseoutLane`) or
    /// compound actions via `Chain(Vec<PolicyAction>)`.
    pub action: PolicyAction,

    /// Evaluation priority. Lower numbers = higher priority.
    ///
    /// Must be a non-negative integer. Rules with priority 0 are evaluated
    /// first. Typical ranges: 1-10 (critical), 11-50 (normal), 51-100 (fallback).
    pub priority: u32,
}

impl PolicyRule {
    /// Create a new PolicyRule.
    pub fn new(name: String, condition: PolicyCondition, action: PolicyAction, priority: u32) -> Self {
        Self {
            name,
            condition,
            action,
            priority,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::{PolicyAction, PolicyCondition};

    #[test]
    fn test_policy_rule_creation() {
        let rule = PolicyRule::new(
            "closeout-completed-lane".to_string(),
            PolicyCondition::LaneCompleted,
            PolicyAction::CloseoutLane,
            10,
        );
        assert_eq!(rule.name, "closeout-completed-lane");
        assert_eq!(rule.priority, 10);
    }

    #[test]
    fn test_policy_rule_serde_roundtrip() {
        let rule = PolicyRule::new(
            "test-rule".to_string(),
            PolicyCondition::GreenAt { level: 3 },
            PolicyAction::Notify {
                channel: "discord".to_string(),
            },
            5,
        );
        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: PolicyRule = serde_json::from_str(&json).unwrap();
        assert_eq!(rule, deserialized);
    }
}
