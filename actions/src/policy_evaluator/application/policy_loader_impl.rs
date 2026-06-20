//! Implementation of `PolicyLoadingService`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#loader
//! Issue: issue-policyloader

use async_trait::async_trait;
use globset::Glob;

use crate::policy_evaluator::domain::{
    CompiledRules, PolicyDocument, PolicyError, PolicyLimits,
};

use super::compiled_rules_factory_impl::CompiledRulesFactoryImpl;
use super::dto::{LoadPolicyInput, LoadPolicyOutput};
use super::factory::{CompiledRulesFactory, PolicyDocumentFactory};
use super::policy_document_factory_impl::PolicyDocumentFactoryImpl;
use super::service::PolicyLoadingService;

/// Default implementation of `PolicyLoadingService`.
///
/// Loads and validates policy TOML files. In production, the policy is
/// read from the repository's base branch via the GitHub API. For testing,
/// content can be provided directly.
pub struct PolicyLoadingServiceImpl;

#[async_trait]
impl PolicyLoadingService for PolicyLoadingServiceImpl {
    async fn load(&self, input: LoadPolicyInput) -> Result<LoadPolicyOutput, PolicyError> {
        let content = self
            .read_policy_content(&input.policy_path, &input.base_ref, input.repo.as_deref())
            .await?;

        let doc_factory = PolicyDocumentFactoryImpl;
        let policy = doc_factory.build_from_toml(&content).await?;
        let compiled_rules = self.compile_patterns(&policy).await?;

        Ok(LoadPolicyOutput {
            policy,
            source_ref: input.base_ref.clone(),
            raw_content: if input.log_content.unwrap_or(false) {
                Some(content)
            } else {
                None
            },
            from_base_branch: true,
            path: input.policy_path,
            compiled_rules,
        })
    }

    async fn validate_version(&self, version: &str) -> Result<(), PolicyError> {
        if version.is_empty() {
            return Err(PolicyError::InvalidSyntax {
                detail: "Version string is empty".to_string(),
                line: None,
            });
        }
        let first = version.chars().next().unwrap_or('0');
        if !first.is_ascii_digit() {
            return Err(PolicyError::UnsupportedVersion {
                version: version.to_string(),
                supported: "1.x".to_string(),
            });
        }
        Ok(())
    }

    async fn compile_patterns(&self, policy: &PolicyDocument) -> Result<CompiledRules, PolicyError> {
        let factory = CompiledRulesFactoryImpl;
        factory.build_from_policy(policy).await
    }

    async fn validate_no_duplicate_rules(
        &self,
        policy: &PolicyDocument,
    ) -> Result<(), PolicyError> {
        let doc_factory = PolicyDocumentFactoryImpl;
        doc_factory.validate(policy).await
    }

    async fn parse_content(&self, content: &str) -> Result<PolicyDocument, PolicyError> {
        let doc_factory = PolicyDocumentFactoryImpl;
        doc_factory.build_from_toml(content).await
    }

    async fn read_policy_content(
        &self,
        _policy_path: &str,
        _base_ref: &str,
        _repo: Option<&str>,
    ) -> Result<String, PolicyError> {
        // In production, this would use the GitHub API to fetch from base branch.
        // For now, return an error — concrete implementations will override this.
        Err(PolicyError::FileNotFound {
            path: _policy_path.to_string(),
            reference: _base_ref.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_evaluator::domain::{AuditConfig, PolicyRules};

    #[tokio::test]
    async fn test_validate_version() {
        let loader = PolicyLoadingServiceImpl;
        assert!(loader.validate_version("1.0.0").await.is_ok());
        assert!(loader.validate_version("").await.is_err());
        assert!(loader.validate_version("abc").await.is_err());
    }

    #[tokio::test]
    async fn test_compile_patterns_empty() {
        let loader = PolicyLoadingServiceImpl;
        let policy = PolicyDocument {
            version: "1.0.0".to_string(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        };
        let result = loader.compile_patterns(&policy).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().deny.len(), 0);
    }

    #[tokio::test]
    async fn test_compile_patterns_with_rules() {
        let loader = PolicyLoadingServiceImpl;
        let toml = r#"
            version = "1.0.0"

            [[rules.deny]]
            name = "no-sql"
            description = "No raw SQL"
            pattern = "*.sql"
            severity = "critical"

            [[rules.flag]]
            name = "big-file"
            description = "Large files"
            pattern = "*.md"
        "#;
        let policy = loader.parse_content(toml).await.unwrap();
        let compiled = loader.compile_patterns(&policy).await.unwrap();
        assert_eq!(compiled.deny.len(), 1);
        assert_eq!(compiled.flag.len(), 1);
    }

    #[tokio::test]
    async fn test_read_policy_content_not_available() {
        let loader = PolicyLoadingServiceImpl;
        let input = LoadPolicyInput {
            policy_path: ".rigorix/policy.toml".to_string(),
            base_ref: "origin/main".to_string(),
            repo: Some("org/repo".to_string()),
            log_content: Some(false),
        };
        let result = loader.load(input).await;
        // Should fail because no GitHub API is available
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PolicyError::FileNotFound { .. }
        ));
    }
}
