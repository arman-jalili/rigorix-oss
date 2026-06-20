//! Data Transfer Objects for the Policy Engine module.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — DTO schemas for policy engine operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::policy_engine::domain::{LaneContext, PolicyAction, PolicyConfig};

// ---------------------------------------------------------------------------
// Evaluate Policy DTOs
// ---------------------------------------------------------------------------

/// Input for evaluating policy rules against a LaneContext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyInput {
    /// The typed execution context to evaluate rules against.
    pub context: LaneContext,

    /// Optional filter to only evaluate specific rules by name.
    /// If `None` or empty, all loaded rules are evaluated.
    pub rule_filter: Option<Vec<String>>,
}

/// Output from evaluating policy rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyOutput {
    /// The lane ID that was evaluated.
    pub lane_id: String,

    /// Flat list of actions from all matching rules, in priority order.
    pub actions: Vec<ActionOutput>,

    /// Number of matching rules.
    pub matching_rule_count: u32,

    /// Total number of rules evaluated.
    pub rules_evaluated: u32,

    /// Whether any rules matched the given context.
    pub matched: bool,
}

/// A single action with its matching rule context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOutput {
    /// The name of the rule that produced this action.
    pub rule_name: String,

    /// The priority of the matching rule.
    pub priority: u32,

    /// The action to execute.
    pub action: PolicyAction,
}

// ---------------------------------------------------------------------------
// Load Rules DTOs
// ---------------------------------------------------------------------------

/// Input for loading policy rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRulesInput {
    /// The policy configuration to load.
    pub config: PolicyConfig,

    /// Whether to replace all existing rules (`true`) or merge (`false`).
    /// Merge behavior: rules from the config replace rules with the same name.
    pub replace_all: bool,
}

/// Output from loading policy rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRulesOutput {
    /// Number of rules successfully loaded.
    pub loaded_count: u32,

    /// Number of rules that replaced existing rules (only if `replace_all` is false).
    pub replaced_count: u32,

    /// Names of the loaded rules.
    pub rule_names: Vec<String>,

    /// Whether the load was successful.
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Get Active Rules DTOs
// ---------------------------------------------------------------------------

/// Output from querying active rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetActiveRulesOutput {
    /// List of currently loaded rules, sorted by priority.
    pub rules: Vec<RuleSummary>,

    /// Total number of loaded rules.
    pub total_count: u32,
}

/// Summary of a single loaded rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    pub name: String,
    pub priority: u32,
    pub condition_summary: String,
    pub action_summary: String,
}

// ---------------------------------------------------------------------------
// Reload Rules DTOs
// ---------------------------------------------------------------------------

/// Output from reloading policy rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadRulesOutput {
    /// Whether the reload was successful.
    pub success: bool,

    /// Number of rules loaded from the source.
    pub rule_count: u32,

    /// The source path/identifier from which rules were reloaded.
    pub source: String,

    /// Error details if the reload failed.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::config::RuleDefinition;
    use crate::policy_engine::domain::{
        DiffScope, LaneBlocker, LaneContext, PolicyAction, PolicyCondition, PolicyConfig,
        ReviewStatus,
    };

    #[test]
    fn test_evaluate_policy_input_serde() {
        let input = EvaluatePolicyInput {
            context: LaneContext {
                lane_id: "lane-1".to_string(),
                green_level: 3,
                branch_freshness_secs: 100,
                blocker: LaneBlocker::None,
                review_status: ReviewStatus::Pending,
                diff_scope: DiffScope::Scoped,
                completed: true,
                reconciled: false,
            },
            rule_filter: None,
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: EvaluatePolicyInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.context.lane_id, "lane-1");
    }

    #[test]
    fn test_evaluate_policy_output_serde() {
        let output = EvaluatePolicyOutput {
            lane_id: "lane-1".to_string(),
            actions: vec![ActionOutput {
                rule_name: "closeout".to_string(),
                priority: 10,
                action: PolicyAction::CloseoutLane,
            }],
            matching_rule_count: 1,
            rules_evaluated: 5,
            matched: true,
        };
        let json = serde_json::to_string(&output).unwrap();
        let deserialized: EvaluatePolicyOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.lane_id, "lane-1");
        assert_eq!(deserialized.actions.len(), 1);
    }

    #[test]
    fn test_load_rules_input_serde() {
        let input = LoadRulesInput {
            config: PolicyConfig::single(RuleDefinition {
                name: "test".to_string(),
                condition: PolicyCondition::LaneCompleted,
                action: PolicyAction::CloseoutLane,
                priority: 10,
            }),
            replace_all: true,
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: LoadRulesInput = serde_json::from_str(&json).unwrap();
        assert!(deserialized.replace_all);
        assert_eq!(deserialized.config.rules.len(), 1);
    }

    #[test]
    fn test_get_active_rules_output() {
        let output = GetActiveRulesOutput {
            rules: vec![RuleSummary {
                name: "rule-1".to_string(),
                priority: 10,
                condition_summary: "LaneCompleted".to_string(),
                action_summary: "CloseoutLane".to_string(),
            }],
            total_count: 1,
        };
        let json = serde_json::to_string(&output).unwrap();
        let deserialized: GetActiveRulesOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_count, 1);
    }

    #[test]
    fn test_reload_rules_output() {
        let output = ReloadRulesOutput {
            success: true,
            rule_count: 5,
            source: ".rigorix/policy.toml".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        let deserialized: ReloadRulesOutput = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.rule_count, 5);
    }
}
