//! PolicyConfig domain entity.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#config
//! Implements: Contract Freeze — PolicyConfig struct
//! Issue: issue-contract-freeze
//!
//! Represents user-configurable policy rule definitions that can be
//! loaded from a TOML file (`.rigorix/policy.toml`). Rules are
//! defined as a list of serializable rule descriptors that are
//! compiled into `PolicyRule` instances by the policy engine.
//!
//! # Contract (Frozen)
//! - `PolicyConfig` is the top-level TOML structure
//! - Each rule definition has a name, condition, action, and priority
//! - Conditions use serde tagged enums for composable trees
//! - Actions use serde tagged enums for dispatch

use serde::{Deserialize, Serialize};

use super::action::PolicyAction;
use super::condition::PolicyCondition;

/// User-configurable policy rule definitions.
///
/// Loaded from `.rigorix/policy.toml` and converted to a list of
/// `PolicyRule` instances for evaluation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// The list of rule definitions.
    pub rules: Vec<RuleDefinition>,
}

/// A single rule definition in the configuration.
///
/// Maps directly to a `PolicyRule` entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleDefinition {
    /// Unique name for this rule.
    pub name: String,

    /// The composable condition for this rule.
    pub condition: PolicyCondition,

    /// The action to take when the condition matches.
    pub action: PolicyAction,

    /// Evaluation priority (lower = higher priority).
    pub priority: u32,
}

impl PolicyConfig {
    /// Create an empty policy configuration.
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a policy configuration with a single rule.
    pub fn single(rule: RuleDefinition) -> Self {
        Self { rules: vec![rule] }
    }

    /// Convert this configuration into a list of `PolicyRule` domain entities.
    pub fn into_rules(self) -> Vec<super::rule::PolicyRule> {
        self.rules
            .into_iter()
            .map(|def| super::rule::PolicyRule {
                name: def.name,
                condition: def.condition,
                action: def.action,
                priority: def.priority,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::PolicyAction;

    #[test]
    fn test_empty_config() {
        let config = PolicyConfig::empty();
        assert!(config.rules.is_empty());
    }

    #[test]
    fn test_single_rule_config() {
        let config = PolicyConfig::single(RuleDefinition {
            name: "test".to_string(),
            condition: PolicyCondition::LaneCompleted,
            action: PolicyAction::CloseoutLane,
            priority: 10,
        });
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].name, "test");
    }

    #[test]
    fn test_convert_to_rules() {
        let config = PolicyConfig {
            rules: vec![
                RuleDefinition {
                    name: "rule-1".to_string(),
                    condition: PolicyCondition::LaneCompleted,
                    action: PolicyAction::CloseoutLane,
                    priority: 10,
                },
                RuleDefinition {
                    name: "rule-2".to_string(),
                    condition: PolicyCondition::ScopedDiff,
                    action: PolicyAction::Notify {
                        channel: "slack".to_string(),
                    },
                    priority: 20,
                },
            ],
        };
        let rules = config.into_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].name, "rule-1");
        assert_eq!(rules[1].name, "rule-2");
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = PolicyConfig::single(RuleDefinition {
            name: "closeout-completed".to_string(),
            condition: PolicyCondition::And {
                conditions: vec![
                    PolicyCondition::LaneCompleted,
                    PolicyCondition::GreenAt { level: 3 },
                ],
            },
            action: PolicyAction::CloseoutLane,
            priority: 10,
        });
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PolicyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
