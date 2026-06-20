//! Service interfaces (use cases) for the Policy Evaluator bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Implements: Contract Freeze — PolicyLoadingService, PolicyEvaluationService,
//! OrgPolicyMergingService, PolicyTamperDetectionService,
//! PolicyReportGenerationService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for loading policy
//! documents, evaluating PR diffs against policies, merging organization-level
//! policies, detecting tampering, and generating violation reports.
//! All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::diff_analyzer::domain::PrDiff;
use crate::policy_evaluator::domain::{PolicyDocument, PolicyError, PolicyResult};

use super::dto::{
    DetectTamperInput, DetectTamperOutput, EvaluatePolicyInput, EvaluatePolicyOutput,
    GenerateReportInput, GenerateReportOutput, LoadOrgPolicyInput, LoadOrgPolicyOutput,
    LoadPolicyInput, LoadPolicyOutput, MergePoliciesInput, MergePoliciesOutput,
    RunPolicyEvaluationInput, RunPolicyEvaluationOutput,
};

/// Application service for loading policy documents from a repository.
///
/// Implements the contract defined in `PolicyLoader` from the architecture doc.
/// Reads the policy file from the BASE BRANCH (not the PR branch) to prevent
/// tampering. Validates TOML syntax, compiles glob patterns, and validates
/// rule consistency.
///
/// # Security
///
/// Policy MUST be loaded from the BASE BRANCH (e.g., `origin/main`),
/// not from the PR branch. This prevents attackers from modifying the policy
/// to bypass governance checks.
///
/// # Contract (Frozen)
/// - `load()` is the primary entry point
/// - Loads from base branch — never from PR branch
/// - Validates TOML syntax and structure
/// - Compiles glob patterns for efficient matching
/// - Detects unsupported policy versions
/// - Returns compiled rules ready for evaluation
#[async_trait]
pub trait PolicyLoadingService: Send + Sync {
    /// Load a policy document from a repository.
    ///
    /// Reads the policy file from the specified base branch reference,
    /// validates TOML syntax, and pre-compiles all glob patterns for
    /// efficient evaluation.
    ///
    /// # Security
    ///
    /// The `base_ref` MUST point to the base branch (e.g., `origin/main`),
    /// never the PR branch. Implementations should enforce this.
    ///
    /// # Returns
    ///
    /// `LoadPolicyOutput` containing:
    /// - The loaded `PolicyDocument`
    /// - Compiled glob patterns ready for evaluation
    /// - Source reference metadata
    async fn load(&self, input: LoadPolicyInput) -> Result<LoadPolicyOutput, PolicyError>;

    /// Validate the policy version against supported range.
    ///
    /// Returns an error if the version is not supported.
    async fn validate_version(&self, version: &str) -> Result<(), PolicyError>;

    /// Compile all glob patterns in the policy rules.
    ///
    /// Returns compiled patterns that can be reused across evaluations.
    /// Returns an error if any pattern fails to compile.
    async fn compile_patterns(
        &self,
        policy: &PolicyDocument,
    ) -> Result<crate::policy_evaluator::domain::CompiledRules, PolicyError>;

    /// Validate that there are no duplicate rule names across categories.
    async fn validate_no_duplicate_rules(&self, policy: &PolicyDocument)
    -> Result<(), PolicyError>;

    /// Parse a raw policy file content string into a `PolicyDocument`.
    ///
    /// Used by implementations to deserialize TOML content.
    async fn parse_content(&self, content: &str) -> Result<PolicyDocument, PolicyError>;

    /// Read the policy file content from the base branch.
    ///
    /// Returns the raw file content as a string.
    async fn read_policy_content(
        &self,
        policy_path: &str,
        base_ref: &str,
        repo: Option<&str>,
    ) -> Result<String, PolicyError>;
}

/// Application service for detecting policy file tampering in a PR diff.
///
/// Checks whether the PR being evaluated modifies the policy file itself.
/// If the policy file is modified in the PR, the evaluation should warn
/// that an admin review is required — the PR is changing the rules it
/// claims to satisfy.
///
/// # Contract (Frozen)
/// - `detect()` checks all changed files against the policy path
/// - Returns tamper status and whether to proceed
/// - Tampering does not block by default — it generates a warning
#[async_trait]
pub trait PolicyTamperDetectionService: Send + Sync {
    /// Detect if the PR modifies the policy file.
    ///
    /// Scans the changed files in the PR diff for the policy file path.
    /// If the policy file itself is modified, this is flagged as tampering.
    ///
    /// # Returns
    ///
    /// `DetectTamperOutput` with tamper status and whether to proceed.
    async fn detect(&self, input: DetectTamperInput) -> Result<DetectTamperOutput, PolicyError>;

    /// Check if a specific file path matches the policy file path.
    ///
    /// Performs exact match and common variations (.rigorix/policy.toml vs
    /// policy.toml, etc.).
    async fn is_policy_file(&self, file_path: &str, policy_path: &str) -> bool;

    /// Get the change status of a file in a diff.
    ///
    /// Returns "added", "modified", "deleted", or None if file not in diff.
    async fn get_change_status(&self, diff: &PrDiff, file_path: &str) -> Option<String>;

    /// Generate a tamper warning message.
    async fn tamper_warning(&self, policy_path: &str) -> String;
}

/// Application service for evaluating a PR diff against policy rules.
///
/// Implements the contract defined in `PolicyEvaluator` from the architecture doc.
/// Matches each changed file against compiled deny, review, and flag rules.
/// Produces structured violations grouped by type.
///
/// # Contract (Frozen)
/// - `evaluate()` is the primary entry point
/// - Evaluates files against compiled rules in order: deny → review → flag
/// - Returns aggregate `PolicyResult` with all violations
/// - May optionally return per-file match details
#[async_trait]
pub trait PolicyEvaluationService: Send + Sync {
    /// Evaluate a PR diff against policy rules.
    ///
    /// For each changed file in the diff:
    /// 1. Check against compiled deny rules → Deny violations
    /// 2. Check against compiled review rules → RequireReview violations
    /// 3. Check against compiled flag rules → Flag violations
    ///
    /// # Returns
    ///
    /// `EvaluatePolicyOutput` containing:
    /// - The aggregate `PolicyResult` with all violations
    /// - Optional per-file match details
    /// - Evaluation timing metadata
    async fn evaluate(
        &self,
        input: EvaluatePolicyInput,
    ) -> Result<EvaluatePolicyOutput, PolicyError>;

    /// Evaluate a single file against all compiled rules.
    ///
    /// Used internally by `evaluate()` for per-file matching.
    /// Returns violations for this file only.
    async fn evaluate_file(
        &self,
        file_path: &str,
        compiled_rules: &crate::policy_evaluator::domain::CompiledRules,
        username: Option<&str>,
    ) -> Result<Vec<crate::policy_evaluator::domain::PolicyViolation>, PolicyError>;

    /// Check if a single compiled deny rule matches a file path.
    ///
    /// Returns `true` if the glob pattern matches and the rule applies
    /// to the given user (if user is provided and not excluded).
    async fn matches_deny_rule(
        &self,
        rule: &crate::policy_evaluator::domain::CompiledDenyRule,
        file_path: &str,
        username: Option<&str>,
    ) -> bool;

    /// Check if a single compiled review rule matches a file path.
    async fn matches_review_rule(
        &self,
        rule: &crate::policy_evaluator::domain::CompiledReviewRule,
        file_path: &str,
    ) -> bool;

    /// Check if a single compiled flag rule matches a file path.
    async fn matches_flag_rule(
        &self,
        rule: &crate::policy_evaluator::domain::CompiledFlagRule,
        file_path: &str,
    ) -> bool;

    /// Count violations by type for a list of violations.
    async fn count_violations(
        &self,
        violations: &[crate::policy_evaluator::domain::PolicyViolation],
    ) -> crate::policy_evaluator::domain::ViolationCounts;

    /// Determine if the action should block based on violations and fail_on_violation setting.
    async fn should_block(&self, result: &PolicyResult, fail_on_violation: bool) -> bool;
}

/// Application service for loading and merging organization-level policies.
///
/// Implements the contract defined in `OrgPolicyMerger` from the architecture doc.
/// Loads organization-wide policy from a configurable source, then merges it with
/// the repository-level policy. The merge follows the "most restrictive wins" rule
/// by default: org deny rules are added, review minimums are maxed, and limits
/// are minimized.
///
/// # Contract (Frozen)
/// - `load_org_policy()` loads the organization policy from its source
/// - `merge()` merges org and repo policies with configurable strategy
/// - `restrictive` strategy: stricter rule wins (default)
/// - `repo_preferred` strategy: repo policy takes precedence
/// - `org_preferred` strategy: org policy overrides repo rules
#[async_trait]
pub trait OrgPolicyMergingService: Send + Sync {
    /// Load the organization-level policy from its configured source.
    ///
    /// Reads the org policy from the configured source path or URL.
    /// Returns `None` if the org policy is not found (non-fatal unless
    /// `require_org_policy` is set).
    async fn load_org_policy(
        &self,
        input: LoadOrgPolicyInput,
    ) -> Result<LoadOrgPolicyOutput, PolicyError>;

    /// Merge organization and repository policies.
    ///
    /// Applies the configured merge strategy:
    /// - `restrictive`: union of deny/review/flag rules, tighter limits
    /// - `repo_preferred`: repo rules take precedence
    /// - `org_preferred`: org rules override repo rules
    ///
    /// # Returns
    ///
    /// The merged policy and metadata about what was added.
    async fn merge(&self, input: MergePoliciesInput) -> Result<MergePoliciesOutput, PolicyError>;

    /// Merge deny rules: union of both policies.
    async fn merge_deny_rules(
        &self,
        repo_rules: &[crate::policy_evaluator::domain::DenyRule],
        org_rules: &[crate::policy_evaluator::domain::DenyRule],
        strategy: &str,
    ) -> Vec<crate::policy_evaluator::domain::DenyRule>;

    /// Merge review rules: union with max required_reviewers.
    async fn merge_review_rules(
        &self,
        repo_rules: &[crate::policy_evaluator::domain::ReviewRule],
        org_rules: &[crate::policy_evaluator::domain::ReviewRule],
        strategy: &str,
    ) -> Vec<crate::policy_evaluator::domain::ReviewRule>;

    /// Merge flag rules: union of both policies.
    async fn merge_flag_rules(
        &self,
        repo_rules: &[crate::policy_evaluator::domain::FlagRule],
        org_rules: &[crate::policy_evaluator::domain::FlagRule],
        strategy: &str,
    ) -> Vec<crate::policy_evaluator::domain::FlagRule>;

    /// Merge limits: tighter bounds win (min of both).
    async fn merge_limits(
        &self,
        repo_limits: &crate::policy_evaluator::domain::PolicyLimits,
        org_limits: &crate::policy_evaluator::domain::PolicyLimits,
    ) -> crate::policy_evaluator::domain::PolicyLimits;

    /// Get the default org policy source path.
    ///
    /// Default: `.rigorix/org-policy.toml` in the organization's
    /// `.github` repository or a configured URL.
    async fn default_org_policy_source(&self) -> String;
}

/// Application service for generating policy violation reports.
///
/// Formats policy evaluation results into:
/// - GitHub workflow annotations (`::error`, `::warning`, `::notice`)
/// - Structured violation entries for machine consumption
/// - Markdown summaries suitable for PR comments
///
/// # Contract (Frozen)
/// - `generate_report()` produces formatted output from evaluation results
/// - Annotations are formatted for GitHub Actions `::` syntax
/// - Markdown summary is suitable for PR comment posting
#[async_trait]
pub trait PolicyReportGenerationService: Send + Sync {
    /// Generate a complete violation report.
    ///
    /// Produces formatted annotations, structured entries, and a
    /// markdown summary from the policy evaluation result.
    async fn generate_report(
        &self,
        input: GenerateReportInput,
    ) -> Result<GenerateReportOutput, PolicyError>;

    /// Format a single violation as a GitHub workflow annotation.
    ///
    /// Returns a string like:
    /// `::error file=src/auth/login.rs,title=Deny rule 'no-secrets'::...`
    async fn format_annotation(
        &self,
        violation: &crate::policy_evaluator::domain::PolicyViolation,
    ) -> String;

    /// Generate a markdown summary of violations.
    ///
    /// Produces a formatted markdown string with violation counts,
    /// categorized lists, and action recommendations.
    async fn format_markdown_summary(&self, result: &PolicyResult) -> String;

    /// Generate a short status line summarizing the evaluation outcome.
    ///
    /// E.g., "✅ No violations found" or "❌ 3 blocking violations, 2 warnings"
    async fn format_status_line(&self, result: &PolicyResult) -> String;

    /// Escape a string for use in GitHub workflow annotation commands.
    ///
    /// Handles escaping of `%`, `\n`, and `\r` characters.
    async fn escape_annotation(&self, text: &str) -> String;
}

/// Application service for orchestrating the full policy evaluation pipeline.
///
/// Coordinates the end-to-end workflow:
/// 1. Load policy from base branch
/// 2. Detect policy tampering
/// 3. Load and merge org policy (if configured)
/// 4. Evaluate PR diff against merged policy
/// 5. Generate violation report
///
/// # Contract (Frozen)
/// - `run()` is the primary entry point for full pipeline execution
/// - Coordinates all sub-services in the correct order
/// - Returns comprehensive output with all results and metadata
#[async_trait]
pub trait PolicyEvaluationPipelineService: Send + Sync {
    /// Run the full policy evaluation pipeline.
    ///
    /// Orchestrates:
    /// 1. Policy loading from base branch
    /// 2. Tamper detection
    /// 3. Org policy merge (if configured)
    /// 4. PR diff evaluation
    /// 5. Report generation
    async fn run(
        &self,
        input: RunPolicyEvaluationInput,
    ) -> Result<RunPolicyEvaluationOutput, PolicyError>;

    /// Run the full pipeline and produce a formatted report.
    ///
    /// Like `run()`, but also generates the formatted report output
    /// for direct use by the action entrypoint.
    async fn run_with_report(
        &self,
        input: RunPolicyEvaluationInput,
    ) -> Result<(RunPolicyEvaluationOutput, GenerateReportOutput), PolicyError>;
}
