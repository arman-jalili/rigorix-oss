//! Implementation of `PolicyDocumentFactory`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#policy
//! Issue: issue-policydocument

use async_trait::async_trait;

use crate::policy_evaluator::domain::{
    AuditConfig, PolicyDocument, PolicyError, PolicyLimits, PolicyRules,
};

use super::factory::PolicyDocumentFactory;

/// Default implementation of `PolicyDocumentFactory`.
pub struct PolicyDocumentFactoryImpl;

#[async_trait]
impl PolicyDocumentFactory for PolicyDocumentFactoryImpl {
    async fn build_from_toml(&self, content: &str) -> Result<PolicyDocument, PolicyError> {
        let policy: PolicyDocument =
            toml::from_str(content).map_err(|e| PolicyError::InvalidSyntax {
                detail: e.to_string(),
                line: None,
            })?;
        self.validate(&policy).await?;
        Ok(policy)
    }

    async fn default(&self) -> PolicyDocument {
        PolicyDocument {
            version: "1.0.0".to_string(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        }
    }

    async fn with_rules(
        &self,
        version: &str,
        rules: PolicyRules,
        limits: Option<PolicyLimits>,
    ) -> PolicyDocument {
        PolicyDocument {
            version: version.to_string(),
            rules,
            limits: limits.unwrap_or_default(),
            audit: AuditConfig::default(),
        }
    }

    async fn validate(&self, policy: &PolicyDocument) -> Result<(), PolicyError> {
        // Validate version is semver-like
        if policy.version.is_empty() {
            return Err(PolicyError::InvalidSyntax {
                detail: "Policy version must not be empty".to_string(),
                line: None,
            });
        }

        // Version must start with a digit
        let first = policy.version.chars().next().unwrap_or('0');
        if !first.is_ascii_digit() {
            return Err(PolicyError::UnsupportedVersion {
                version: policy.version.clone(),
                supported: "semver (e.g., 1.0.0)".to_string(),
            });
        }

        // Validate no duplicate rule names
        let mut names: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for rule in &policy.rules.deny_rules {
            if !names.insert(&rule.name) {
                return Err(PolicyError::DuplicateRuleName {
                    name: rule.name.clone(),
                    categories: vec!["deny".to_string()],
                });
            }
        }
        for rule in &policy.rules.require_review_rules {
            if !names.insert(&rule.name) {
                return Err(PolicyError::DuplicateRuleName {
                    name: rule.name.clone(),
                    categories: vec!["require_review".to_string()],
                });
            }
        }
        for rule in &policy.rules.flag_rules {
            if !names.insert(&rule.name) {
                return Err(PolicyError::DuplicateRuleName {
                    name: rule.name.clone(),
                    categories: vec!["flag".to_string()],
                });
            }
        }

        Ok(())
    }

    async fn with_limits(
        &self,
        policy: &PolicyDocument,
        limits: PolicyLimits,
    ) -> PolicyDocument {
        let mut p = policy.clone();
        p.limits = limits;
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_from_toml_valid() {
        let factory = PolicyDocumentFactoryImpl;
        let toml = r#"
            version = "1.0.0"

            [[rules.deny]]
            name = "no-sql"
            description = "No raw SQL"
            pattern = "*.sql"
            severity = "critical"
        "#;
        let result = factory.build_from_toml(toml).await;
        assert!(result.is_ok());
        let policy = result.unwrap();
        assert_eq!(policy.version, "1.0.0");
        assert_eq!(policy.rules.deny_rules.len(), 1);
    }

    #[tokio::test]
    async fn test_build_from_toml_invalid_syntax() {
        let factory = PolicyDocumentFactoryImpl;
        let result = factory.build_from_toml("not valid toml {{").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PolicyError::InvalidSyntax { .. }));
    }

    #[tokio::test]
    async fn test_default_policy() {
        let factory = PolicyDocumentFactoryImpl;
        let policy = factory.default().await;
        assert_eq!(policy.version, "1.0.0");
        assert!(policy.rules.deny_rules.is_empty());
        assert!(policy.rules.require_review_rules.is_empty());
        assert!(policy.rules.flag_rules.is_empty());
    }

    #[tokio::test]
    async fn test_validate_empty_version() {
        let factory = PolicyDocumentFactoryImpl;
        let policy = PolicyDocument {
            version: String::new(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        };
        let result = factory.validate(&policy).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_duplicate_names() {
        let factory = PolicyDocumentFactoryImpl;
        let toml = r#"
            version = "1.0.0"

            [[rules.deny]]
            name = "duplicate"
            description = "First"
            pattern = "*.rs"
            severity = "high"

            [[rules.deny]]
            name = "duplicate"
            description = "Second"
            pattern = "*.toml"
            severity = "medium"
        "#;
        let result = factory.build_from_toml(toml).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PolicyError::DuplicateRuleName { .. }));
    }
}
