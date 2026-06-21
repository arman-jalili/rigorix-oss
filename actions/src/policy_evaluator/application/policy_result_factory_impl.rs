//! Implementation of `PolicyResultFactory`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#result
//! Issue: issue-policyevaluator

use async_trait::async_trait;

use crate::policy_evaluator::domain::{PolicyMetadata, PolicyResult, PolicyViolation};

use super::factory::PolicyResultFactory;

/// Default implementation of `PolicyResultFactory`.
pub struct PolicyResultFactoryImpl;

#[async_trait]
impl PolicyResultFactory for PolicyResultFactoryImpl {
    async fn build(
        &self,
        violations: Vec<PolicyViolation>,
        policy_tamper_detected: bool,
        policy_version: &str,
        deny_rule_count: usize,
        review_rule_count: usize,
        flag_rule_count: usize,
        org_policy_merged: bool,
    ) -> PolicyResult {
        PolicyResult::new(
            violations,
            policy_tamper_detected,
            PolicyMetadata {
                policy_version: policy_version.to_string(),
                org_policy_merged,
                deny_rule_count,
                review_rule_count,
                flag_rule_count,
            },
        )
    }

    async fn empty(&self, policy_version: &str, org_policy_merged: bool) -> PolicyResult {
        PolicyResult::new(
            vec![],
            false,
            PolicyMetadata {
                policy_version: policy_version.to_string(),
                org_policy_merged,
                deny_rule_count: 0,
                review_rule_count: 0,
                flag_rule_count: 0,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_evaluator::domain::Severity;

    #[tokio::test]
    async fn test_build_empty_result() {
        let factory = PolicyResultFactoryImpl;
        let result = factory.empty("1.0.0", false).await;
        assert!(!result.has_blocking_violations);
        assert!(!result.has_warnings);
        assert_eq!(result.counts.total(), 0);
    }

    #[tokio::test]
    async fn test_build_with_violations() {
        let factory = PolicyResultFactoryImpl;
        let violations = vec![
            PolicyViolation::Deny {
                rule: "no-sql".to_string(),
                description: "No raw SQL".to_string(),
                severity: Severity::Critical,
                file: "db/migrate.sql".to_string(),
                message: "Blocked by no-sql".to_string(),
            },
            PolicyViolation::Flag {
                rule: "big-file".to_string(),
                description: "Big file".to_string(),
                file: "src/main.rs".to_string(),
                message: "Flagged".to_string(),
            },
        ];
        let result = factory
            .build(violations, false, "1.0.0", 1, 0, 1, false)
            .await;
        assert!(result.has_blocking_violations);
        assert!(result.has_warnings);
        assert_eq!(result.counts.deny, 1);
        assert_eq!(result.counts.flag, 1);
    }
}
