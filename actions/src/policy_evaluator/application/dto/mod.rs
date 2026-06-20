//! Data Transfer Objects for the Policy Evaluator module.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Implements: Contract Freeze — DTO schemas for policy loading, evaluation,
//! org policy merging, and tamper detection
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for event processing)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};

use crate::diff_analyzer::domain::PrDiff;
use crate::policy_evaluator::domain::{
    CompiledRules, OrgPolicyConfig, PolicyDocument, PolicyResult,
};

// ---------------------------------------------------------------------------
// Policy Loading DTOs
// ---------------------------------------------------------------------------

/// Input for loading a policy document from a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPolicyInput {
    /// Path to the policy file (e.g., ".rigorix/policy.toml").
    pub policy_path: String,

    /// The git ref (branch/commit) to load from.
    /// Security: MUST be the base branch, NOT the PR branch.
    pub base_ref: String,

    /// Owner and repository name in "owner/repo" format, if using GitHub API.
    pub repo: Option<String>,

    /// Whether to log the loaded policy content (default: false).
    /// Security: set to false in production to avoid leaking policy rules.
    pub log_content: Option<bool>,
}

/// Output from loading a policy document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPolicyOutput {
    /// The loaded policy document.
    pub policy: PolicyDocument,

    /// The source reference from which the policy was loaded.
    pub source_ref: String,

    /// The raw TOML content of the policy file (if requested).
    pub raw_content: Option<String>,

    /// Whether the policy was loaded from the base branch.
    pub from_base_branch: bool,

    /// Path to the loaded policy file.
    pub path: String,

    /// Compiled rule patterns ready for evaluation.
    pub compiled_rules: CompiledRules,
}

// ---------------------------------------------------------------------------
// Policy Tamper Detection DTOs
// ---------------------------------------------------------------------------

/// Input for detecting policy file tampering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectTamperInput {
    /// The parsed PR diff to check.
    pub diff: PrDiff,

    /// The expected path of the policy file.
    pub policy_path: String,
}

/// Output from policy tamper detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectTamperOutput {
    /// Whether the PR modifies the policy file.
    pub tamper_detected: bool,

    /// The policy file path that was modified (if detected).
    pub tampered_path: Option<String>,

    /// The change status of the policy file (added, modified, deleted).
    pub change_status: Option<String>,

    /// Whether to proceed with evaluation despite tampering.
    /// Set to false to block evaluation when the policy itself is modified.
    pub proceed: bool,
}

// ---------------------------------------------------------------------------
// Policy Evaluation DTOs
// ---------------------------------------------------------------------------

/// Input for evaluating a PR diff against policy rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyInput {
    /// The parsed PR diff to evaluate.
    pub diff: PrDiff,

    /// The loaded policy document with rules.
    pub policy: PolicyDocument,

    /// Pre-compiled rule patterns for efficient matching.
    pub compiled_rules: CompiledRules,

    /// Whether to fail the action on violations.
    /// When false, violations are reported as warnings.
    pub fail_on_violation: bool,

    /// Whether to include detailed file-level matching in results.
    pub include_details: Option<bool>,
}

/// A single file match result during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatchResult {
    /// The file path that was evaluated.
    pub file: String,

    /// Number of deny rules that matched this file.
    pub deny_matches: usize,

    /// Number of review rules that matched this file.
    pub review_matches: usize,

    /// Number of flag rules that matched this file.
    pub flag_matches: usize,

    /// Whether any deny match was a blocking severity.
    pub blocking: bool,
}

/// Output from evaluating a PR diff against policy rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyOutput {
    /// The aggregate policy result.
    pub result: PolicyResult,

    /// Per-file match results (if detailed output was requested).
    pub file_matches: Option<Vec<FileMatchResult>>,

    /// Whether the policy file itself was modified in this diff.
    pub policy_tamper_detected: bool,

    /// Number of files that were evaluated.
    pub files_evaluated: usize,

    /// Evaluation duration in milliseconds.
    pub evaluation_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Org Policy Merging DTOs
// ---------------------------------------------------------------------------

/// Input for loading the organization-level policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadOrgPolicyInput {
    /// Configuration for loading the org policy.
    pub org_config: OrgPolicyConfig,

    /// The base reference for loading (same as repo policy).
    pub base_ref: String,

    /// Owner and repository name in "owner/repo" format.
    pub repo: Option<String>,

    /// Whether to fail if the org policy is missing.
    /// Defaults to the value in OrgPolicyConfig.
    pub require_org_policy: Option<bool>,
}

/// Output from loading the organization-level policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadOrgPolicyOutput {
    /// The loaded organization policy document (if found).
    pub org_policy: Option<PolicyDocument>,

    /// The source from which the org policy was loaded.
    pub source: Option<String>,

    /// Whether the org policy was found and loaded.
    pub loaded: bool,

    /// Warning message if the org policy was expected but not found.
    pub warning: Option<String>,
}

/// Input for merging organization and repository policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePoliciesInput {
    /// The repository-level policy document.
    pub repo_policy: PolicyDocument,

    /// The organization-level policy document (if available).
    pub org_policy: Option<PolicyDocument>,

    /// The merge strategy to use.
    pub merge_strategy: String,
}

/// Output from merging organization and repository policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePoliciesOutput {
    /// The merged policy document.
    pub merged_policy: PolicyDocument,

    /// Whether any org rules were added.
    pub org_rules_added: bool,

    /// Number of deny rules contributed by org policy.
    pub org_deny_rules_added: usize,

    /// Number of review rules contributed by org policy.
    pub org_review_rules_added: usize,

    /// Number of flag rules contributed by org policy.
    pub org_flag_rules_added: usize,

    /// Whether limits were tightened by the org policy.
    pub limits_tightened: bool,
}

// ---------------------------------------------------------------------------
// Full Pipeline DTOs
// ---------------------------------------------------------------------------

/// Input for running the full policy evaluation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPolicyEvaluationInput {
    /// The parsed PR diff to evaluate.
    pub diff: PrDiff,

    /// Path to the policy file (e.g., ".rigorix/policy.toml").
    pub policy_path: String,

    /// The base git ref to load the policy from.
    pub base_ref: String,

    /// Owner and repository name in "owner/repo" format.
    pub repo: Option<String>,

    /// Configuration for organization-level policy (optional).
    pub org_policy_config: Option<OrgPolicyConfig>,

    /// Whether to fail the action on violations.
    pub fail_on_violation: bool,

    /// Whether to include detailed results.
    pub include_details: Option<bool>,
}

/// Output from the full policy evaluation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPolicyEvaluationOutput {
    /// The final evaluation result.
    pub result: PolicyResult,

    /// Whether the policy was loaded successfully.
    pub policy_loaded: bool,

    /// Whether org policy was merged.
    pub org_policy_merged: bool,

    /// Total processing time in milliseconds.
    pub processing_time_ms: u64,

    /// Summary of the pipeline execution.
    pub summary: PolicyPipelineSummary,
}

/// Summary of a full policy evaluation pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyPipelineSummary {
    /// Whether the policy file was loaded successfully.
    pub policy_loaded: bool,
    /// Whether organization policy was merged.
    pub org_policy_merged: bool,
    /// Whether tampering was detected.
    pub tamper_detected: bool,
    /// Number of files evaluated.
    pub files_evaluated: usize,
    /// Number of violations found.
    pub violation_count: usize,
    /// Number of blocking violations found.
    pub blocking_count: usize,
    /// Whether the evaluation resulted in a blocking action.
    pub is_blocking: bool,
}

// ---------------------------------------------------------------------------
// Violation Reporting DTOs
// ---------------------------------------------------------------------------

/// Input for generating a violation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateReportInput {
    /// The policy evaluation result.
    pub result: PolicyResult,

    /// The PR diff that was evaluated.
    pub diff: PrDiff,

    /// Whether to format for GitHub workflow annotations.
    pub github_format: Option<bool>,

    /// Whether to include the full violation list.
    pub include_violations: Option<bool>,
}

/// A formatted violation entry for reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationReportEntry {
    /// The violation type (deny, require_review, flag).
    pub violation_type: String,
    /// The rule name.
    pub rule: String,
    /// The file path.
    pub file: String,
    /// The violation message.
    pub message: String,
    /// The annotation type for GitHub (error, warning, notice).
    pub annotation_type: String,
}

/// Output from generating a violation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateReportOutput {
    /// Formatted GitHub workflow annotations.
    pub annotations: Vec<String>,

    /// Structured violation entries.
    pub entries: Vec<ViolationReportEntry>,

    /// Markdown summary suitable for PR comments.
    pub markdown_summary: String,

    /// Whether the action should fail based on violations + fail_on_violation.
    pub should_fail: bool,
}
