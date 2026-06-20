//! Implementation of `RulesFactory`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#rule
//! Issue: issue-policyrule-types

use async_trait::async_trait;
use globset::Glob;

use crate::policy_evaluator::domain::{
    DenyRule, FlagRule, PolicyError, PolicyRules, ReviewRule, Severity,
};

use super::factory::RulesFactory;

/// Default implementation of `RulesFactory`.
pub struct RulesFactoryImpl;

#[async_trait]
impl RulesFactory for RulesFactoryImpl {
    async fn build_deny_rule(
        &self,
        name: &str,
        description: &str,
        pattern: &str,
        severity: Severity,
        exclude_users: Vec<String>,
    ) -> Result<DenyRule, PolicyError> {
        self.validate_pattern(pattern).await?;
        Ok(DenyRule {
            name: name.to_string(),
            description: description.to_string(),
            pattern: pattern.to_string(),
            severity,
            exclude_users,
        })
    }

    async fn build_review_rule(
        &self,
        name: &str,
        description: &str,
        pattern: &str,
        required_reviewers: u8,
    ) -> Result<ReviewRule, PolicyError> {
        self.validate_pattern(pattern).await?;
        Ok(ReviewRule {
            name: name.to_string(),
            description: description.to_string(),
            pattern: pattern.to_string(),
            required_reviewers,
        })
    }

    async fn build_flag_rule(
        &self,
        name: &str,
        description: &str,
        pattern: &str,
        message: Option<String>,
    ) -> Result<FlagRule, PolicyError> {
        self.validate_pattern(pattern).await?;
        Ok(FlagRule {
            name: name.to_string(),
            description: description.to_string(),
            pattern: pattern.to_string(),
            message,
        })
    }

    async fn parse_severity(&self, severity: &str) -> Result<Severity, PolicyError> {
        match severity.to_lowercase().as_str() {
            "critical" => Ok(Severity::Critical),
            "high" => Ok(Severity::High),
            "medium" => Ok(Severity::Medium),
            "low" => Ok(Severity::Low),
            other => Err(PolicyError::InvalidGlobPattern {
                rule: "<unknown>".to_string(),
                pattern: other.to_string(),
                detail: format!("Unknown severity level: '{}'. Valid: critical, high, medium, low", other),
            }),
        }
    }

    async fn validate_pattern(&self, pattern: &str) -> Result<(), PolicyError> {
        Glob::new(pattern).map_err(|e| PolicyError::InvalidGlobPattern {
            rule: "<validation>".to_string(),
            pattern: pattern.to_string(),
            detail: e.to_string(),
        })?;
        Ok(())
    }

    async fn validate_rule_patterns(&self, rules: &PolicyRules) -> Result<(), PolicyError> {
        for rule in &rules.deny_rules {
            self.validate_pattern(&rule.pattern).await?;
        }
        for rule in &rules.require_review_rules {
            self.validate_pattern(&rule.pattern).await?;
        }
        for rule in &rules.flag_rules {
            self.validate_pattern(&rule.pattern).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_valid_deny_rule() {
        let factory = RulesFactoryImpl;
        let result = factory
            .build_deny_rule("no-sql", "No raw SQL", "*.sql", Severity::Critical, vec![])
            .await;
        assert!(result.is_ok());
        let rule = result.unwrap();
        assert_eq!(rule.name, "no-sql");
        assert_eq!(rule.severity, Severity::Critical);
    }

    #[tokio::test]
    async fn test_build_invalid_pattern() {
        let factory = RulesFactoryImpl;
        let result = factory
            .build_deny_rule("bad", "Bad pattern", "[invalid", Severity::High, vec![])
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PolicyError::InvalidGlobPattern { .. }));
    }

    #[tokio::test]
    async fn test_parse_severity() {
        let factory = RulesFactoryImpl;
        assert_eq!(
            factory.parse_severity("critical").await.unwrap(),
            Severity::Critical
        );
        assert_eq!(factory.parse_severity("high").await.unwrap(), Severity::High);
        assert_eq!(factory.parse_severity("medium").await.unwrap(), Severity::Medium);
        assert_eq!(factory.parse_severity("low").await.unwrap(), Severity::Low);
        assert!(factory.parse_severity("unknown").await.is_err());
    }

    #[tokio::test]
    async fn test_build_review_rule() {
        let factory = RulesFactoryImpl;
        let result = factory
            .build_review_rule("auth-check", "Auth review", "src/auth/**", 2)
            .await;
        assert!(result.is_ok());
        let rule = result.unwrap();
        assert_eq!(rule.required_reviewers, 2);
    }

    #[tokio::test]
    async fn test_build_flag_rule_with_message() {
        let factory = RulesFactoryImpl;
        let result = factory
            .build_flag_rule(
                "big-migration",
                "Large migration",
                "migrations/**/*.sql",
                Some("Check performance impact".to_string()),
            )
            .await;
        assert!(result.is_ok());
        let rule = result.unwrap();
        assert_eq!(rule.message.as_deref(), Some("Check performance impact"));
    }

    #[tokio::test]
    async fn test_validate_rule_patterns() {
        let factory = RulesFactoryImpl;
        let rules = PolicyRules {
            deny_rules: vec![DenyRule {
                name: "valid".to_string(),
                description: "Desc".to_string(),
                pattern: "*.rs".to_string(),
                severity: Severity::Critical,
                exclude_users: vec![],
            }],
            require_review_rules: vec![],
            flag_rules: vec![],
        };
        assert!(factory.validate_rule_patterns(&rules).await.is_ok());
    }
}
