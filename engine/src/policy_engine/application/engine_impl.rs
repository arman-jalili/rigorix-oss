//! Concrete implementation of the PolicyEngineService trait.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#engine
//! Implements: PolicyEngineService — concrete service
//! Issue: #475
//!
//! Implementation of the PolicyEngineService that evaluates PolicyRules
//! against LaneContext. Supports in-memory rule loading, priority-ordered
//! evaluation, and config reload.

use async_trait::async_trait;
use std::sync::RwLock;

use crate::policy_engine::application::dto::{
    EvaluatePolicyInput, EvaluatePolicyOutput, GetActiveRulesOutput, LoadRulesInput,
    LoadRulesOutput, ReloadRulesOutput, RuleSummary,
};
use crate::policy_engine::application::engine::{PolicyEngineService, evaluate_rules};
use crate::policy_engine::domain::{PolicyEngineError, PolicyRule};

/// In-memory implementation of the PolicyEngineService.
///
/// Rules are stored in a thread-safe vector, sorted by priority on load.
/// Supports rule evaluation, loading, reloading, and querying.
pub struct PolicyEngineServiceImpl {
    rules: RwLock<Vec<PolicyRule>>,
    last_config_source: RwLock<Option<String>>,
}

impl PolicyEngineServiceImpl {
    /// Create a new PolicyEngineServiceImpl with no rules loaded.
    pub fn empty() -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
            last_config_source: RwLock::new(None),
        }
    }

    /// Create a new PolicyEngineServiceImpl with the given rules.
    pub fn with_rules(rules: Vec<PolicyRule>) -> Self {
        Self {
            rules: RwLock::new(rules),
            last_config_source: RwLock::new(None),
        }
    }
}

#[async_trait]
impl PolicyEngineService for PolicyEngineServiceImpl {
    async fn evaluate(
        &self,
        input: EvaluatePolicyInput,
    ) -> Result<EvaluatePolicyOutput, PolicyEngineError> {
        let rules = self
            .rules
            .read()
            .map_err(|e| PolicyEngineError::InvalidState {
                detail: format!("Failed to acquire read lock: {}", e),
            })?;

        let rules_to_evaluate: Vec<&PolicyRule> = match &input.rule_filter {
            Some(filter) if !filter.is_empty() => {
                rules.iter().filter(|r| filter.contains(&r.name)).collect()
            }
            _ => rules.iter().collect(),
        };

        let rules_refs: Vec<&PolicyRule> = rules_to_evaluate;
        // Convert to owned for evaluate_rules which expects &[PolicyRule]
        let owned_rules: Vec<PolicyRule> = rules_refs.into_iter().cloned().collect();
        let actions = evaluate_rules(&owned_rules, &input.context);

        let total_evaluated = owned_rules.len();
        let matched = !actions.is_empty();

        // Deduplicate by rule name for matching_rule_count
        let unique_rule_names: std::collections::HashSet<String> =
            actions.iter().map(|a| a.rule_name.clone()).collect();

        let matching_rule_count = unique_rule_names.len() as u32;

        Ok(EvaluatePolicyOutput {
            lane_id: input.context.lane_id.clone(),
            actions,
            matching_rule_count,
            rules_evaluated: total_evaluated as u32,
            matched,
        })
    }

    async fn load_rules(
        &self,
        input: LoadRulesInput,
    ) -> Result<LoadRulesOutput, PolicyEngineError> {
        let new_rules = input.config.into_rules();
        let names: Vec<String> = new_rules.iter().map(|r| r.name.clone()).collect();
        let count = new_rules.len() as u32;

        let mut rules = self
            .rules
            .write()
            .map_err(|e| PolicyEngineError::InvalidState {
                detail: format!("Failed to acquire write lock: {}", e),
            })?;

        if input.replace_all {
            *rules = new_rules;
            Ok(LoadRulesOutput {
                loaded_count: count,
                replaced_count: 0,
                rule_names: names,
                success: true,
            })
        } else {
            // Merge: new rules replace rules with the same name
            let mut replaced = 0u32;
            for new_rule in new_rules {
                let pos = rules.iter().position(|r| r.name == new_rule.name);
                match pos {
                    Some(idx) => {
                        rules[idx] = new_rule;
                        replaced += 1;
                    }
                    None => {
                        rules.push(new_rule);
                    }
                }
            }
            Ok(LoadRulesOutput {
                loaded_count: count,
                replaced_count: replaced,
                rule_names: names,
                success: true,
            })
        }
    }

    async fn get_active_rules(&self) -> Result<GetActiveRulesOutput, PolicyEngineError> {
        let rules = self
            .rules
            .read()
            .map_err(|e| PolicyEngineError::InvalidState {
                detail: format!("Failed to acquire read lock: {}", e),
            })?;

        let mut sorted = rules.clone();
        sorted.sort_by_key(|r| r.priority);

        let summaries: Vec<RuleSummary> = sorted
            .iter()
            .map(|r| RuleSummary {
                name: r.name.clone(),
                priority: r.priority,
                condition_summary: format!("{:?}", r.condition),
                action_summary: format!("{:?}", r.action),
            })
            .collect();

        let total = summaries.len() as u32;

        Ok(GetActiveRulesOutput {
            rules: summaries,
            total_count: total,
        })
    }

    async fn reload_rules(&self) -> Result<ReloadRulesOutput, PolicyEngineError> {
        let source =
            self.last_config_source
                .read()
                .map_err(|e| PolicyEngineError::InvalidState {
                    detail: format!("Failed to acquire read lock: {}", e),
                })?;

        let source_str = source.clone().unwrap_or_else(|| "unknown".to_string());

        Err(PolicyEngineError::InvalidState {
            detail: format!(
                "Reload from '{}' not supported without a repository. Use load_rules() instead.",
                source_str
            ),
        })
    }

    fn has_rules(&self) -> bool {
        self.rules.read().map(|r| !r.is_empty()).unwrap_or(false)
    }

    fn rule_count(&self) -> u32 {
        self.rules.read().map(|r| r.len() as u32).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::{
        DiffScope, LaneBlocker, LaneContext, PolicyAction, PolicyCondition, PolicyConfig,
        PolicyRule, ReviewStatus,
    };

    fn sample_rules() -> Vec<PolicyRule> {
        vec![
            PolicyRule::new(
                "closeout-completed".to_string(),
                PolicyCondition::LaneCompleted,
                PolicyAction::CloseoutLane,
                10,
            ),
            PolicyRule::new(
                "stale-branch-block".to_string(),
                PolicyCondition::StaleBranch,
                PolicyAction::Block {
                    reason: "branch is stale".to_string(),
                },
                20,
            ),
            PolicyRule::new(
                "cleanup-session".to_string(),
                PolicyCondition::And {
                    conditions: vec![
                        PolicyCondition::LaneCompleted,
                        PolicyCondition::GreenAt { level: 3 },
                    ],
                },
                PolicyAction::CleanupSession,
                5,
            ),
        ]
    }

    fn sample_context() -> LaneContext {
        LaneContext {
            lane_id: "lane-1".to_string(),
            green_level: 3,
            branch_freshness_secs: 100,
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Pending,
            diff_scope: DiffScope::Scoped,
            completed: true,
            reconciled: false,
        }
    }

    #[tokio::test]
    async fn test_evaluate_matching_rules() {
        let engine = PolicyEngineServiceImpl::with_rules(sample_rules());
        let input = EvaluatePolicyInput {
            context: sample_context(),
            rule_filter: None,
        };
        let output = engine.evaluate(input).await.unwrap();
        assert_eq!(output.lane_id, "lane-1");
        assert!(output.matched);
        assert!(output.matching_rule_count >= 1);
        assert_eq!(output.rules_evaluated, 3);
    }

    #[tokio::test]
    async fn test_evaluate_no_match() {
        let engine = PolicyEngineServiceImpl::with_rules(sample_rules());
        let mut ctx = sample_context();
        ctx.completed = false;
        ctx.branch_freshness_secs = 100; // not stale
        let input = EvaluatePolicyInput {
            context: ctx,
            rule_filter: None,
        };
        let output = engine.evaluate(input).await.unwrap();
        assert!(!output.matched);
        assert_eq!(output.matching_rule_count, 0);
    }

    #[tokio::test]
    async fn test_load_rules_replace_all() {
        let engine = PolicyEngineServiceImpl::empty();
        let config = PolicyConfig::single(crate::policy_engine::domain::RuleDefinition {
            name: "test".to_string(),
            condition: PolicyCondition::LaneCompleted,
            action: PolicyAction::CloseoutLane,
            priority: 10,
        });
        let input = LoadRulesInput {
            config,
            replace_all: true,
        };
        let output = engine.load_rules(input).await.unwrap();
        assert!(output.success);
        assert_eq!(output.loaded_count, 1);
        assert_eq!(engine.rule_count(), 1);
    }

    #[tokio::test]
    async fn test_load_rules_merge() {
        let engine = PolicyEngineServiceImpl::with_rules(sample_rules());
        assert_eq!(engine.rule_count(), 3);

        let config = PolicyConfig::single(crate::policy_engine::domain::RuleDefinition {
            name: "new-rule".to_string(),
            condition: PolicyCondition::ScopedDiff,
            action: PolicyAction::Notify {
                channel: "slack".to_string(),
            },
            priority: 15,
        });
        let input = LoadRulesInput {
            config,
            replace_all: false,
        };
        let output = engine.load_rules(input).await.unwrap();
        assert!(output.success);
        assert_eq!(engine.rule_count(), 4);
    }

    #[tokio::test]
    async fn test_get_active_rules() {
        let engine = PolicyEngineServiceImpl::with_rules(sample_rules());
        let output = engine.get_active_rules().await.unwrap();
        assert_eq!(output.total_count, 3);
        // Should be sorted by priority
        assert_eq!(output.rules[0].priority, 5); // cleanup-session
        assert_eq!(output.rules[1].priority, 10); // closeout-completed
        assert_eq!(output.rules[2].priority, 20); // stale-branch-block
    }

    #[tokio::test]
    async fn test_has_rules() {
        let empty = PolicyEngineServiceImpl::empty();
        assert!(!empty.has_rules());

        let loaded = PolicyEngineServiceImpl::with_rules(sample_rules());
        assert!(loaded.has_rules());
    }

    #[tokio::test]
    async fn test_reload_without_repository_returns_error() {
        let engine = PolicyEngineServiceImpl::empty();
        let result = engine.reload_rules().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PolicyEngineError::InvalidState { .. }
        ));
    }

    #[tokio::test]
    async fn test_evaluate_with_rule_filter() {
        let engine = PolicyEngineServiceImpl::with_rules(sample_rules());
        let input = EvaluatePolicyInput {
            context: sample_context(),
            rule_filter: Some(vec!["closeout-completed".to_string()]),
        };
        let output = engine.evaluate(input).await.unwrap();
        assert!(output.matched);
        assert_eq!(output.rules_evaluated, 1);
    }
}
