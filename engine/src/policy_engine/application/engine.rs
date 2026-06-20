//! PolicyEngineService — application service trait for policy evaluation.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#engine
//! Implements: Contract Freeze — PolicyEngineService trait
//! Issue: issue-contract-freeze
//!
//! Defines the application service interface for the policy engine.
//! The PolicyEngineService evaluates declarative PolicyRules against
//! LaneContext and returns ordered action lists. It also manages
//! rule lifecycle (load, reload, query).
//!
//! # Contract (Frozen)
//! - All methods are async (use `async-trait` for trait object safety)
//! - All public methods return `Result<_, PolicyEngineError>`
//! - Input/output types are DTOs defined in `dto/`
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::policy_engine::domain::{LaneContext, PolicyEngineError, PolicyRule};

use super::dto::{
    EvaluatePolicyInput, EvaluatePolicyOutput, GetActiveRulesOutput, LoadRulesOutput,
    ReloadRulesOutput,
};

/// Central policy evaluation service.
///
/// The PolicyEngineService sits between the orchestrator (which provides
/// the LaneContext after execution) and the rule repository (which provides
/// the PolicyRules). It evaluates rules against the context and produces
/// actionable results.
///
/// # Integration
///
/// The orchestrator calls `evaluate()` after execution completes, before
/// the closeout phase. The resulting action list is dispatched by the
/// orchestrator in order.
#[async_trait]
pub trait PolicyEngineService: Send + Sync {
    /// Evaluate all loaded rules against the given LaneContext.
    ///
    /// Returns a flat list of actions from all matching rules, ordered
    /// by rule priority. Rules are evaluated in ascending priority order
    /// (lower priority number = evaluated first). If multiple rules match,
    /// all matching actions are collected and flattened.
    ///
    /// # Errors
    ///
    /// Returns `PolicyEngineError::NoMatchingRule` if no rules match the
    /// given context (configurable — some orchestrators may treat this
    /// as informational rather than an error).
    async fn evaluate(
        &self,
        input: EvaluatePolicyInput,
    ) -> Result<EvaluatePolicyOutput, PolicyEngineError>;

    /// Load rules from a policy configuration.
    ///
    /// Replaces all currently loaded rules with those from the provided
    /// configuration. Rules are validated during loading — duplicate names
    /// or invalid configurations return an error.
    async fn load_rules(
        &self,
        config: super::dto::LoadRulesInput,
    ) -> Result<LoadRulesOutput, PolicyEngineError>;

    /// Get all currently active (loaded) rules.
    ///
    /// Returns the list of rules sorted by priority.
    async fn get_active_rules(&self) -> Result<GetActiveRulesOutput, PolicyEngineError>;

    /// Reload rules from the last loaded source.
    ///
    /// Re-reads the policy configuration from the source that was used
    /// in the last `load_rules()` call. If no source was previously loaded,
    /// returns `PolicyEngineError::InvalidState`.
    async fn reload_rules(&self) -> Result<ReloadRulesOutput, PolicyEngineError>;

    /// Check whether any rules are currently loaded.
    fn has_rules(&self) -> bool;

    /// Get the number of currently loaded rules.
    fn rule_count(&self) -> u32;
}

/// Convenience function: evaluate rules in memory against a context.
///
/// This is a pure function that doesn't go through the service trait,
/// useful for testing and simple in-process evaluations. It sorts rules
/// by priority (ascending), evaluates each against the context, and
/// returns a flat list of matching actions.
///
/// # Panics
///
/// This function does not panic. If no rules match, it returns an empty
/// vector (callers should check for this condition).
pub fn evaluate_rules(
    rules: &[PolicyRule],
    context: &LaneContext,
) -> Vec<super::dto::ActionOutput> {
    let mut sorted_rules: Vec<&PolicyRule> = rules.iter().collect();
    sorted_rules.sort_by_key(|r| r.priority);

    let mut actions = Vec::new();
    for rule in sorted_rules {
        if rule.condition.matches(context) {
            let mut flattened = Vec::new();
            rule.action.clone().flatten_into(&mut flattened);
            for action in flattened {
                actions.push(super::dto::ActionOutput {
                    rule_name: rule.name.clone(),
                    priority: rule.priority,
                    action,
                });
            }
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::{
        DiffScope, LaneBlocker, PolicyAction, PolicyCondition, PolicyRule, ReviewStatus,
    };

    #[test]
    fn test_evaluate_rules_in_priority_order() {
        let rules = vec![
            PolicyRule::new(
                "low".to_string(),
                PolicyCondition::LaneCompleted,
                PolicyAction::Notify {
                    channel: "slack".to_string(),
                },
                20,
            ),
            PolicyRule::new(
                "high".to_string(),
                PolicyCondition::LaneCompleted,
                PolicyAction::CloseoutLane,
                10,
            ),
        ];

        let ctx = LaneContext {
            lane_id: "test".to_string(),
            green_level: 3,
            branch_freshness_secs: 100,
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Pending,
            diff_scope: DiffScope::Scoped,
            completed: true,
            reconciled: false,
        };

        let actions = evaluate_rules(&rules, &ctx);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].rule_name, "high");
        assert_eq!(actions[1].rule_name, "low");
    }

    #[test]
    fn test_evaluate_rules_no_match() {
        let rules = vec![PolicyRule::new(
            "stale".to_string(),
            PolicyCondition::StaleBranch,
            PolicyAction::Block {
                reason: "branch is stale".to_string(),
            },
            10,
        )];

        let ctx = LaneContext {
            lane_id: "test".to_string(),
            green_level: 3,
            branch_freshness_secs: 100, // fresh, not stale
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Pending,
            diff_scope: DiffScope::Scoped,
            completed: true,
            reconciled: false,
        };

        let actions = evaluate_rules(&rules, &ctx);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_evaluate_rules_chain_flattening() {
        let rules = vec![PolicyRule::new(
            "chain-rule".to_string(),
            PolicyCondition::LaneCompleted,
            PolicyAction::Chain(vec![
                PolicyAction::CloseoutLane,
                PolicyAction::CleanupSession,
            ]),
            10,
        )];

        let ctx = LaneContext {
            lane_id: "test".to_string(),
            green_level: 3,
            branch_freshness_secs: 100,
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Pending,
            diff_scope: DiffScope::Scoped,
            completed: true,
            reconciled: false,
        };

        let actions = evaluate_rules(&rules, &ctx);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action, PolicyAction::CloseoutLane);
        assert_eq!(actions[1].action, PolicyAction::CleanupSession);
    }
}
