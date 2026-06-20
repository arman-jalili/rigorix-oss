//! Concrete implementation of the PolicyEngineFactory trait.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: PolicyEngineFactory — concrete factory
//! Issue: #475
//!
//! Builds PolicyEngineService instances from config, defaults, or
//! rule definitions.

use async_trait::async_trait;

use crate::policy_engine::application::engine::PolicyEngineService;
use crate::policy_engine::application::engine_impl::PolicyEngineServiceImpl;
use crate::policy_engine::application::factory::PolicyEngineFactory;
use crate::policy_engine::domain::{PolicyConfig, PolicyEngineError, PolicyRule, RuleDefinition};

/// Default implementation of PolicyEngineFactory.
///
/// Creates PolicyEngineServiceImpl instances with the requested
/// configuration or default rules.
pub struct PolicyEngineFactoryImpl;

impl PolicyEngineFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PolicyEngineFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default set of policy rules for standard operation.
fn default_rules() -> Vec<PolicyRule> {
    vec![
        PolicyRule::new(
            "closeout-completed-lane".to_string(),
            crate::policy_engine::domain::PolicyCondition::And {
                conditions: vec![
                    crate::policy_engine::domain::PolicyCondition::LaneCompleted,
                    crate::policy_engine::domain::PolicyCondition::GreenAt { level: 3 },
                ],
            },
            crate::policy_engine::domain::PolicyAction::CloseoutLane,
            10,
        ),
        PolicyRule::new(
            "cleanup-completed-session".to_string(),
            crate::policy_engine::domain::PolicyCondition::LaneCompleted,
            crate::policy_engine::domain::PolicyAction::CleanupSession,
            20,
        ),
        PolicyRule::new(
            "reconcile-empty-diff".to_string(),
            crate::policy_engine::domain::PolicyCondition::And {
                conditions: vec![
                    crate::policy_engine::domain::PolicyCondition::LaneCompleted,
                    crate::policy_engine::domain::PolicyCondition::ScopedDiff,
                ],
            },
            crate::policy_engine::domain::PolicyAction::Reconcile {
                reason: crate::policy_engine::domain::ReconcileReason::EmptyDiff,
            },
            15,
        ),
        PolicyRule::new(
            "escalate-startup-blocked".to_string(),
            crate::policy_engine::domain::PolicyCondition::StartupBlocked,
            crate::policy_engine::domain::PolicyAction::Escalate {
                reason: "Lane is blocked at startup".to_string(),
            },
            5,
        ),
    ]
}

#[async_trait]
impl PolicyEngineFactory for PolicyEngineFactoryImpl {
    async fn create_from_config(
        &self,
        config: PolicyConfig,
    ) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError> {
        let rules = config.into_rules();
        if rules.is_empty() {
            return Err(PolicyEngineError::InvalidConfiguration {
                detail: "Policy config contains no rules".to_string(),
            });
        }
        Ok(Box::new(PolicyEngineServiceImpl::with_rules(rules)))
    }

    async fn create_default(&self) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError> {
        Ok(Box::new(PolicyEngineServiceImpl::with_rules(
            default_rules(),
        )))
    }

    async fn create_with_rules(
        &self,
        rule_definitions: Vec<RuleDefinition>,
    ) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError> {
        if rule_definitions.is_empty() {
            return Err(PolicyEngineError::InvalidConfiguration {
                detail: "No rule definitions provided".to_string(),
            });
        }
        let rules: Vec<PolicyRule> = rule_definitions
            .into_iter()
            .map(|def| PolicyRule {
                name: def.name,
                condition: def.condition,
                action: def.action,
                priority: def.priority,
            })
            .collect();
        Ok(Box::new(PolicyEngineServiceImpl::with_rules(rules)))
    }

    async fn create_with_repository(
        &self,
        _repository: Box<dyn crate::policy_engine::infrastructure::repository::PolicyRepository>,
    ) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError> {
        // Load rules from repository and create engine
        let config = _repository.load_config().await?;
        self.create_from_config(config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::{PolicyAction, PolicyCondition, RuleDefinition};

    #[tokio::test]
    async fn test_create_default() {
        let factory = PolicyEngineFactoryImpl::new();
        let engine = factory.create_default().await.unwrap();
        assert!(engine.has_rules());
        assert_eq!(engine.rule_count(), 4);
    }

    #[tokio::test]
    async fn test_create_from_config() {
        let factory = PolicyEngineFactoryImpl::new();
        let config = PolicyConfig::single(RuleDefinition {
            name: "test".to_string(),
            condition: PolicyCondition::LaneCompleted,
            action: PolicyAction::CloseoutLane,
            priority: 10,
        });
        let engine = factory.create_from_config(config).await.unwrap();
        assert_eq!(engine.rule_count(), 1);
    }

    #[tokio::test]
    async fn test_create_from_empty_config_errors() {
        let factory = PolicyEngineFactoryImpl::new();
        let config = PolicyConfig::empty();
        let result = factory.create_from_config(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_with_rules() {
        let factory = PolicyEngineFactoryImpl::new();
        let defs = vec![RuleDefinition {
            name: "rule-1".to_string(),
            condition: PolicyCondition::LaneCompleted,
            action: PolicyAction::CloseoutLane,
            priority: 10,
        }];
        let engine = factory.create_with_rules(defs).await.unwrap();
        assert_eq!(engine.rule_count(), 1);
    }

    #[tokio::test]
    async fn test_create_with_empty_rules_errors() {
        let factory = PolicyEngineFactoryImpl::new();
        let result = factory.create_with_rules(vec![]).await;
        assert!(result.is_err());
    }
}
