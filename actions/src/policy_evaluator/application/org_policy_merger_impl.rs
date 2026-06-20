//! Implementation of `OrgPolicyMergingService`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#merger
//! Issue: issue-orgpolicymerger

use async_trait::async_trait;

use crate::policy_evaluator::domain::{
    DenyRule, FlagRule, PolicyDocument, PolicyError, PolicyLimits, ReviewRule,
};

use super::dto::{
    LoadOrgPolicyInput, LoadOrgPolicyOutput, MergePoliciesInput, MergePoliciesOutput,
};
use super::service::OrgPolicyMergingService;

/// Default implementation of `OrgPolicyMergingService`.
///
/// Merges organization-level policy with repository-level policy.
/// Default strategy is "restrictive" — stricter rules always win.
pub struct OrgPolicyMergingServiceImpl;

#[async_trait]
impl OrgPolicyMergingService for OrgPolicyMergingServiceImpl {
    async fn load_org_policy(
        &self,
        input: LoadOrgPolicyInput,
    ) -> Result<LoadOrgPolicyOutput, PolicyError> {
        // Org policy loading is not yet linked to a GitHub client implementation.
        // Return not-found with a warning by default.
        let require = input
            .require_org_policy
            .unwrap_or(input.org_config.require_org_policy);
        let source = input.org_config.org_policy_source.clone();

        if require {
            Err(PolicyError::OrgPolicyLoadError {
                detail: format!("Organization policy not found at source: {}", source),
            })
        } else {
            Ok(LoadOrgPolicyOutput {
                org_policy: None,
                source: Some(source.clone()),
                loaded: false,
                warning: Some(format!(
                    "Organization policy not found at '{}' — proceeding with repo policy only",
                    source
                )),
            })
        }
    }

    async fn merge(&self, input: MergePoliciesInput) -> Result<MergePoliciesOutput, PolicyError> {
        let org_policy = match &input.org_policy {
            Some(p) => p,
            None => {
                return Ok(MergePoliciesOutput {
                    merged_policy: input.repo_policy,
                    org_rules_added: false,
                    org_deny_rules_added: 0,
                    org_review_rules_added: 0,
                    org_flag_rules_added: 0,
                    limits_tightened: false,
                });
            }
        };

        let strategy = &input.merge_strategy;

        let merged_deny = self
            .merge_deny_rules(
                &input.repo_policy.rules.deny_rules,
                &org_policy.rules.deny_rules,
                strategy,
            )
            .await;
        let merged_review = self
            .merge_review_rules(
                &input.repo_policy.rules.require_review_rules,
                &org_policy.rules.require_review_rules,
                strategy,
            )
            .await;
        let merged_flag = self
            .merge_flag_rules(
                &input.repo_policy.rules.flag_rules,
                &org_policy.rules.flag_rules,
                strategy,
            )
            .await;
        let merged_limits = self
            .merge_limits(&input.repo_policy.limits, &org_policy.limits)
            .await;

        let org_deny_added = org_policy.rules.deny_rules.len();
        let org_review_added = org_policy.rules.require_review_rules.len();
        let org_flag_added = org_policy.rules.flag_rules.len();

        let merged = PolicyDocument {
            version: input.repo_policy.version.clone(),
            rules: crate::policy_evaluator::domain::PolicyRules {
                deny_rules: merged_deny,
                require_review_rules: merged_review,
                flag_rules: merged_flag,
            },
            limits: merged_limits,
            audit: input.repo_policy.audit.clone(),
        };

        Ok(MergePoliciesOutput {
            merged_policy: merged,
            org_rules_added: org_deny_added + org_review_added + org_flag_added > 0,
            org_deny_rules_added: org_deny_added,
            org_review_rules_added: org_review_added,
            org_flag_rules_added: org_flag_added,
            limits_tightened: self
                .are_limits_tightened(&input.repo_policy.limits, &org_policy.limits),
        })
    }

    async fn merge_deny_rules(
        &self,
        repo_rules: &[DenyRule],
        org_rules: &[DenyRule],
        _strategy: &str,
    ) -> Vec<DenyRule> {
        // Union of both: org rules are always added for restrictive strategy
        let mut merged = repo_rules.to_vec();
        merged.extend(org_rules.iter().cloned());
        merged
    }

    async fn merge_review_rules(
        &self,
        repo_rules: &[ReviewRule],
        org_rules: &[ReviewRule],
        _strategy: &str,
    ) -> Vec<ReviewRule> {
        // Union with max required_reviewers
        let mut merged: Vec<ReviewRule> = repo_rules.to_vec();

        for org_rule in org_rules {
            // Check if a rule with the same name exists in repo
            if let Some(existing) = merged.iter_mut().find(|r| r.name == org_rule.name) {
                // Take the higher reviewer count
                existing.required_reviewers =
                    existing.required_reviewers.max(org_rule.required_reviewers);
            } else {
                merged.push(org_rule.clone());
            }
        }

        merged
    }

    async fn merge_flag_rules(
        &self,
        repo_rules: &[FlagRule],
        org_rules: &[FlagRule],
        _strategy: &str,
    ) -> Vec<FlagRule> {
        let mut merged = repo_rules.to_vec();
        merged.extend(org_rules.iter().cloned());
        merged
    }

    async fn merge_limits(
        &self,
        repo_limits: &PolicyLimits,
        org_limits: &PolicyLimits,
    ) -> PolicyLimits {
        let mut merged = repo_limits.clone();
        merged.apply_stricter(org_limits);
        merged
    }

    async fn default_org_policy_source(&self) -> String {
        ".github/rigorix/org-policy.toml".to_string()
    }
}

impl OrgPolicyMergingServiceImpl {
    fn are_limits_tightened(&self, repo: &PolicyLimits, org: &PolicyLimits) -> bool {
        let tight_diff = |a: Option<u64>, b: Option<u64>| -> bool {
            match (a, b) {
                (Some(r), Some(o)) => o < r,
                (None, Some(_)) => true, // org introduced a limit where repo had none
                _ => false,
            }
        };

        tight_diff(repo.max_diff_size, org.max_diff_size)
            || tight_diff(
                repo.max_files.map(|x| x as u64),
                org.max_files.map(|x| x as u64),
            )
            || tight_diff(
                repo.max_lines_per_file.map(|x| x as u64),
                org.max_lines_per_file.map(|x| x as u64),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_evaluator::domain::{AuditConfig, PolicyDocument, PolicyRules, Severity};

    fn make_policy(rules: PolicyRules, limits: PolicyLimits) -> PolicyDocument {
        PolicyDocument {
            version: "1.0.0".to_string(),
            rules,
            limits,
            audit: AuditConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_merge_no_org_policy() {
        let merger = OrgPolicyMergingServiceImpl;
        let repo = make_policy(PolicyRules::default(), PolicyLimits::default());
        let input = MergePoliciesInput {
            repo_policy: repo.clone(),
            org_policy: None,
            merge_strategy: "restrictive".to_string(),
        };
        let output = merger.merge(input).await.unwrap();
        assert!(!output.org_rules_added);
        assert!(!output.limits_tightened);
    }

    #[tokio::test]
    async fn test_merge_deny_union() {
        let merger = OrgPolicyMergingServiceImpl;
        let repo = make_policy(
            PolicyRules {
                deny_rules: vec![DenyRule {
                    name: "repo-deny".to_string(),
                    description: "Repo deny".to_string(),
                    pattern: "*.rs".to_string(),
                    severity: Severity::High,
                    exclude_users: vec![],
                }],
                require_review_rules: vec![],
                flag_rules: vec![],
            },
            PolicyLimits::default(),
        );
        let org = make_policy(
            PolicyRules {
                deny_rules: vec![DenyRule {
                    name: "org-deny".to_string(),
                    description: "Org deny".to_string(),
                    pattern: "*.sql".to_string(),
                    severity: Severity::Critical,
                    exclude_users: vec![],
                }],
                require_review_rules: vec![],
                flag_rules: vec![],
            },
            PolicyLimits::default(),
        );

        let input = MergePoliciesInput {
            repo_policy: repo,
            org_policy: Some(org),
            merge_strategy: "restrictive".to_string(),
        };
        let output = merger.merge(input).await.unwrap();
        assert!(output.org_rules_added);
        assert_eq!(output.org_deny_rules_added, 1);
        assert_eq!(output.merged_policy.rules.deny_rules.len(), 2);
    }

    #[tokio::test]
    async fn test_merge_limits_tightened() {
        let merger = OrgPolicyMergingServiceImpl;
        let repo_limits = PolicyLimits {
            max_diff_size: Some(1_000_000),
            max_files: Some(500),
            max_lines_per_file: Some(2000),
        };
        let org_limits = PolicyLimits {
            max_diff_size: Some(500_000),
            max_files: Some(100),
            max_lines_per_file: Some(1000),
        };

        let merged = merger.merge_limits(&repo_limits, &org_limits).await;
        assert_eq!(merged.max_diff_size, Some(500_000));
        assert_eq!(merged.max_files, Some(100));
        assert_eq!(merged.max_lines_per_file, Some(1000));
    }

    #[tokio::test]
    async fn test_merge_review_rules_max_reviewers() {
        let merger = OrgPolicyMergingServiceImpl;
        let repo_reviews = vec![ReviewRule {
            name: "auth".to_string(),
            description: "Auth".to_string(),
            pattern: "src/auth/**".to_string(),
            required_reviewers: 1,
        }];
        let org_reviews = vec![ReviewRule {
            name: "auth".to_string(),
            description: "Auth".to_string(),
            pattern: "src/auth/**".to_string(),
            required_reviewers: 2,
        }];

        let merged = merger
            .merge_review_rules(&repo_reviews, &org_reviews, "restrictive")
            .await;
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].required_reviewers, 2);
    }

    #[tokio::test]
    async fn test_default_org_source() {
        let merger = OrgPolicyMergingServiceImpl;
        let source = merger.default_org_policy_source().await;
        assert_eq!(source, ".github/rigorix/org-policy.toml");
    }
}
