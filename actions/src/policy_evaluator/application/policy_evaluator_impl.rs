//! Implementation of `PolicyEvaluationService`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#evaluator
//! Issue: issue-policyevaluator

use async_trait::async_trait;
use globset::Glob;

use crate::diff_analyzer::domain::PrDiff;
use crate::policy_evaluator::domain::{
    CompiledDenyRule, CompiledFlagRule, CompiledReviewRule, CompiledRules, PolicyDocument,
    PolicyError, PolicyViolation, ViolationCounts,
};
use crate::policy_evaluator::domain::{PolicyMetadata, PolicyResult};

use super::dto::{EvaluatePolicyInput, EvaluatePolicyOutput, FileMatchResult};
use super::service::PolicyEvaluationService;

/// Default implementation of `PolicyEvaluationService`.
///
/// Matches each changed file in a PR diff against compiled deny, review,
/// and flag rules. Returns structured violations grouped by type.
pub struct PolicyEvaluationServiceImpl;

impl PolicyEvaluationServiceImpl {
    fn glob_matches(pattern: &str, file_path: &str) -> bool {
        Glob::new(pattern)
            .ok()
            .map(|g| g.compile_matcher().is_match(file_path))
            .unwrap_or(false)
    }
}

#[async_trait]
impl PolicyEvaluationService for PolicyEvaluationServiceImpl {
    async fn evaluate(
        &self,
        input: EvaluatePolicyInput,
    ) -> Result<EvaluatePolicyOutput, PolicyError> {
        let start = std::time::Instant::now();
        let mut violations = Vec::new();
        let mut file_matches = Vec::new();

        for file in input.diff.changed_files() {
            let file_violations = self
                .evaluate_file(&file.path, &input.compiled_rules, None)
                .await?;

            let deny_count = file_violations
                .iter()
                .filter(|v| matches!(v, PolicyViolation::Deny { .. }))
                .count();
            let review_count = file_violations
                .iter()
                .filter(|v| matches!(v, PolicyViolation::RequireReview { .. }))
                .count();
            let flag_count = file_violations
                .iter()
                .filter(|v| matches!(v, PolicyViolation::Flag { .. }))
                .count();

            file_matches.push(FileMatchResult {
                file: file.path.clone(),
                deny_matches: deny_count,
                review_matches: review_count,
                flag_matches: flag_count,
                blocking: deny_count > 0,
            });

            violations.extend(file_violations);
        }

        let policy_metadata = PolicyMetadata {
            policy_version: input.policy.version.clone(),
            org_policy_merged: false,
            deny_rule_count: input.compiled_rules.deny.len(),
            review_rule_count: input.compiled_rules.review.len(),
            flag_rule_count: input.compiled_rules.flag.len(),
        };

        let result = PolicyResult::new(violations, false, policy_metadata);
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(EvaluatePolicyOutput {
            result,
            file_matches: if input.include_details.unwrap_or(false) {
                Some(file_matches)
            } else {
                None
            },
            policy_tamper_detected: false,
            files_evaluated: input.diff.changed_file_count(),
            evaluation_time_ms: elapsed,
        })
    }

    async fn evaluate_file(
        &self,
        file_path: &str,
        compiled_rules: &CompiledRules,
        username: Option<&str>,
    ) -> Result<Vec<PolicyViolation>, PolicyError> {
        let mut violations = Vec::new();

        // Check deny rules
        for rule in &compiled_rules.deny {
            if self.matches_deny_rule(rule, file_path, username).await {
                violations.push(PolicyViolation::Deny {
                    rule: rule.name.clone(),
                    description: rule.description.clone(),
                    severity: rule.severity.clone(),
                    file: file_path.to_string(),
                    message: format!(
                        "File '{}' matches deny rule '{}': {}",
                        file_path, rule.name, rule.description
                    ),
                });
            }
        }

        // Check review rules
        for rule in &compiled_rules.review {
            if self.matches_review_rule(rule, file_path).await {
                violations.push(PolicyViolation::RequireReview {
                    rule: rule.name.clone(),
                    description: rule.description.clone(),
                    file: file_path.to_string(),
                    required_reviewers: rule.required_reviewers,
                });
            }
        }

        // Check flag rules
        for rule in &compiled_rules.flag {
            if self.matches_flag_rule(rule, file_path).await {
                let message = rule
                    .message
                    .clone()
                    .unwrap_or_else(|| {
                        format!("File '{}' flagged by rule '{}'", file_path, rule.name)
                    });
                violations.push(PolicyViolation::Flag {
                    rule: rule.name.clone(),
                    description: rule.description.clone(),
                    file: file_path.to_string(),
                    message,
                });
            }
        }

        Ok(violations)
    }

    async fn matches_deny_rule(
        &self,
        rule: &CompiledDenyRule,
        file_path: &str,
        username: Option<&str>,
    ) -> bool {
        // Check user exclusion
        if let Some(user) = username {
            if rule.exclude_users.iter().any(|u| u == user) {
                return false;
            }
        }
        Self::glob_matches(&rule.pattern, file_path)
    }

    async fn matches_review_rule(
        &self,
        rule: &CompiledReviewRule,
        file_path: &str,
    ) -> bool {
        Self::glob_matches(&rule.pattern, file_path)
    }

    async fn matches_flag_rule(
        &self,
        rule: &CompiledFlagRule,
        file_path: &str,
    ) -> bool {
        Self::glob_matches(&rule.pattern, file_path)
    }

    async fn count_violations(
        &self,
        violations: &[PolicyViolation],
    ) -> ViolationCounts {
        ViolationCounts {
            deny: violations
                .iter()
                .filter(|v| matches!(v, PolicyViolation::Deny { .. }))
                .count(),
            review: violations
                .iter()
                .filter(|v| matches!(v, PolicyViolation::RequireReview { .. }))
                .count(),
            flag: violations
                .iter()
                .filter(|v| matches!(v, PolicyViolation::Flag { .. }))
                .count(),
        }
    }

    async fn should_block(&self, result: &PolicyResult, fail_on_violation: bool) -> bool {
        result.should_block(fail_on_violation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::domain::{
        ChangedFile, FileRisk, FileStatus, PrDiff,
    };
    use crate::policy_evaluator::domain::{
        AuditConfig, PolicyLimits, PolicyRules, Severity,
    };

    fn make_diff(files: Vec<(&str, FileStatus)>) -> PrDiff {
        PrDiff {
            files: files
                .into_iter()
                .map(|(path, status)| ChangedFile {
                    path: path.to_string(),
                    status,
                    additions: 1,
                    deletions: 0,
                    is_binary: false,
                    hunks: vec![],
                    risk: FileRisk::Low,
                    raw_diff: None,
                })
                .collect(),
            total_size_bytes: 100,
            excluded_files: vec![],
            limits_exceeded: false,
            policy_modified: false,
            ai_signals: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_evaluate_no_matches() {
        let evaluator = PolicyEvaluationServiceImpl;
        let diff = make_diff(vec![("src/main.rs", FileStatus::Modified)]);
        let compiled = CompiledRules::empty();
        let policy = PolicyDocument {
            version: "1.0.0".to_string(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        };

        let input = EvaluatePolicyInput {
            diff,
            policy,
            compiled_rules: compiled,
            fail_on_violation: false,
            include_details: Some(false),
        };
        let output = evaluator.evaluate(input).await.unwrap();
        assert_eq!(output.result.violations.len(), 0);
        assert!(!output.result.has_blocking_violations);
    }

    #[tokio::test]
    async fn test_evaluate_deny_match() {
        let evaluator = PolicyEvaluationServiceImpl;
        let diff = make_diff(vec![("db/migrate.sql", FileStatus::Added)]);
        let compiled = CompiledRules {
            deny: vec![CompiledDenyRule {
                name: "no-sql".to_string(),
                description: "No raw SQL".to_string(),
                severity: Severity::Critical,
                exclude_users: vec![],
                pattern: "*.sql".to_string(),
            }],
            review: vec![],
            flag: vec![],
        };
        let policy = PolicyDocument {
            version: "1.0.0".to_string(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        };

        let input = EvaluatePolicyInput {
            diff,
            policy,
            compiled_rules: compiled,
            fail_on_violation: false,
            include_details: Some(false),
        };
        let output = evaluator.evaluate(input).await.unwrap();
        assert_eq!(output.result.violations.len(), 1);
        assert!(output.result.has_blocking_violations);
        assert!(matches!(
            output.result.violations[0],
            PolicyViolation::Deny { ref rule, .. } if rule == "no-sql"
        ));
    }

    #[tokio::test]
    async fn test_evaluate_review_match() {
        let evaluator = PolicyEvaluationServiceImpl;
        let diff = make_diff(vec![("src/auth/login.rs", FileStatus::Modified)]);
        let compiled = CompiledRules {
            deny: vec![],
            review: vec![CompiledReviewRule {
                name: "auth-check".to_string(),
                description: "Auth review".to_string(),
                required_reviewers: 2,
                pattern: "src/auth/**".to_string(),
            }],
            flag: vec![],
        };
        let policy = PolicyDocument {
            version: "1.0.0".to_string(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        };

        let input = EvaluatePolicyInput {
            diff,
            policy,
            compiled_rules: compiled,
            fail_on_violation: false,
            include_details: Some(false),
        };
        let output = evaluator.evaluate(input).await.unwrap();
        assert_eq!(output.result.violations.len(), 1);
        assert!(matches!(
            output.result.violations[0],
            PolicyViolation::RequireReview { ref rule, .. } if rule == "auth-check"
        ));
    }

    #[tokio::test]
    async fn test_evaluate_flag_match() {
        let evaluator = PolicyEvaluationServiceImpl;
        let diff = make_diff(vec![("docs/readme.md", FileStatus::Modified)]);
        let compiled = CompiledRules {
            deny: vec![],
            review: vec![],
            flag: vec![CompiledFlagRule {
                name: "doc-flag".to_string(),
                description: "Doc flag".to_string(),
                message: Some("Check docs".to_string()),
                pattern: "*.md".to_string(),
            }],
        };
        let policy = PolicyDocument {
            version: "1.0.0".to_string(),
            rules: PolicyRules::default(),
            limits: PolicyLimits::default(),
            audit: AuditConfig::default(),
        };

        let input = EvaluatePolicyInput {
            diff,
            policy,
            compiled_rules: compiled,
            fail_on_violation: false,
            include_details: Some(false),
        };
        let output = evaluator.evaluate(input).await.unwrap();
        assert_eq!(output.result.violations.len(), 1);
        assert!(matches!(
            output.result.violations[0],
            PolicyViolation::Flag { ref rule, .. } if rule == "doc-flag"
        ));
    }

    #[tokio::test]
    async fn test_glob_match() {
        assert!(PolicyEvaluationServiceImpl::glob_matches("*.sql", "db/migrate.sql"));
        assert!(PolicyEvaluationServiceImpl::glob_matches("src/auth/**", "src/auth/login.rs"));
        assert!(!PolicyEvaluationServiceImpl::glob_matches("*.sql", "src/main.rs"));
        assert!(PolicyEvaluationServiceImpl::glob_matches("migrations/**/*.sql", "migrations/001.sql"));
    }

    #[tokio::test]
    async fn test_deny_rule_user_exclusion() {
        let evaluator = PolicyEvaluationServiceImpl;
        let rule = CompiledDenyRule {
            name: "no-sql".to_string(),
            description: "No raw SQL".to_string(),
            severity: Severity::Critical,
            exclude_users: vec!["admin".to_string()],
            pattern: "*.sql".to_string(),
        };
        // Admin user should be excluded
        assert!(!evaluator
            .matches_deny_rule(&rule, "db/migrate.sql", Some("admin"))
            .await);
        // Other user should NOT be excluded
        assert!(evaluator
            .matches_deny_rule(&rule, "db/migrate.sql", Some("developer"))
            .await);
    }
}
