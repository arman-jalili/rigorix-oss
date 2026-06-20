//! Implementation of `CompiledRulesFactory`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#rule
//! Issue: issue-policyrule-types

use async_trait::async_trait;
use globset::Glob;

use crate::policy_evaluator::domain::{
    CompiledDenyRule, CompiledFlagRule, CompiledReviewRule, CompiledRules, DenyRule, FlagRule,
    PolicyDocument, PolicyError, ReviewRule,
};

use super::factory::CompiledRulesFactory;

/// Default implementation of `CompiledRulesFactory`.
pub struct CompiledRulesFactoryImpl;

#[async_trait]
impl CompiledRulesFactory for CompiledRulesFactoryImpl {
    async fn build_from_policy(
        &self,
        policy: &PolicyDocument,
    ) -> Result<CompiledRules, PolicyError> {
        let mut deny = Vec::with_capacity(policy.rules.deny_rules.len());
        for rule in &policy.rules.deny_rules {
            deny.push(self.compile_deny_rule(rule).await?);
        }

        let mut review = Vec::with_capacity(policy.rules.require_review_rules.len());
        for rule in &policy.rules.require_review_rules {
            review.push(self.compile_review_rule(rule).await?);
        }

        let mut flag = Vec::with_capacity(policy.rules.flag_rules.len());
        for rule in &policy.rules.flag_rules {
            flag.push(self.compile_flag_rule(rule).await?);
        }

        Ok(CompiledRules { deny, review, flag })
    }

    async fn compile_deny_rule(&self, rule: &DenyRule) -> Result<CompiledDenyRule, PolicyError> {
        // Validate pattern compiles
        Glob::new(&rule.pattern).map_err(|e| PolicyError::InvalidGlobPattern {
            rule: rule.name.clone(),
            pattern: rule.pattern.clone(),
            detail: e.to_string(),
        })?;

        Ok(CompiledDenyRule {
            name: rule.name.clone(),
            description: rule.description.clone(),
            severity: rule.severity.clone(),
            exclude_users: rule.exclude_users.clone(),
            pattern: rule.pattern.clone(),
        })
    }

    async fn compile_review_rule(
        &self,
        rule: &ReviewRule,
    ) -> Result<CompiledReviewRule, PolicyError> {
        Glob::new(&rule.pattern).map_err(|e| PolicyError::InvalidGlobPattern {
            rule: rule.name.clone(),
            pattern: rule.pattern.clone(),
            detail: e.to_string(),
        })?;

        Ok(CompiledReviewRule {
            name: rule.name.clone(),
            description: rule.description.clone(),
            required_reviewers: rule.required_reviewers,
            pattern: rule.pattern.clone(),
        })
    }

    async fn compile_flag_rule(&self, rule: &FlagRule) -> Result<CompiledFlagRule, PolicyError> {
        Glob::new(&rule.pattern).map_err(|e| PolicyError::InvalidGlobPattern {
            rule: rule.name.clone(),
            pattern: rule.pattern.clone(),
            detail: e.to_string(),
        })?;

        Ok(CompiledFlagRule {
            name: rule.name.clone(),
            description: rule.description.clone(),
            message: rule.message.clone(),
            pattern: rule.pattern.clone(),
        })
    }

    fn empty(&self) -> CompiledRules {
        CompiledRules::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_evaluator::domain::{
        AuditConfig, PolicyDocument, PolicyLimits, PolicyRules, Severity,
    };

    #[tokio::test]
    async fn test_compile_empty_policy() {
        let factory = CompiledRulesFactoryImpl;
        let policy = PolicyDocument {
            version: "1.0.0".to_string(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        };
        let result = factory.build_from_policy(&policy).await;
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert!(compiled.deny.is_empty());
        assert!(compiled.review.is_empty());
        assert!(compiled.flag.is_empty());
    }

    #[tokio::test]
    async fn test_compile_deny_rule() {
        let factory = CompiledRulesFactoryImpl;
        let rule = DenyRule {
            name: "no-sql".to_string(),
            description: "No raw SQL".to_string(),
            pattern: "*.sql".to_string(),
            severity: Severity::Critical,
            exclude_users: vec![],
        };
        let result = factory.compile_deny_rule(&rule).await;
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert_eq!(compiled.name, "no-sql");
        assert_eq!(compiled.severity, Severity::Critical);
    }

    #[tokio::test]
    async fn test_compile_invalid_pattern() {
        let factory = CompiledRulesFactoryImpl;
        let rule = DenyRule {
            name: "bad".to_string(),
            description: "Bad".to_string(),
            pattern: "[invalid".to_string(),
            severity: Severity::High,
            exclude_users: vec![],
        };
        let result = factory.compile_deny_rule(&rule).await;
        assert!(result.is_err());
    }
}
