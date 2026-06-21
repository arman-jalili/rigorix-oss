//! Domain types for policy evaluation.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#types
//! Implements: Contract Freeze — PolicyDocument, PolicyRules, DenyRule, ReviewRule,
//! FlagRule, Severity, PolicyLimits, AuditConfig, PolicyViolation, PolicyResult,
//! ViolationCounts, CompiledRules, OrgPolicyConfig
//! Issue: issue-contract-freeze
//!
//! These are the core domain types that represent policy documents, rule definitions,
//! evaluation results, and configuration. They serve as the frozen contract that all
//! implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All types are serializable (Serialize + Deserialize) where applicable

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PolicyDocument
// ---------------------------------------------------------------------------

/// Complete policy document loaded from `.rigorix/policy.toml`.
///
/// Policies are versioned and include rules for deny, review, and flag
/// classifications, plus resource limits and audit configuration.
/// Policies MUST be loaded from the BASE BRANCH (not the PR branch) to
/// prevent tampering.
///
/// ## Source
///
/// Read from `.rigorix/policy.toml` in the repository root by default,
/// or from a custom path specified by the `policy_file` action input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDocument {
    /// Policy schema version (semver).
    pub version: String,

    /// Classification rules.
    #[serde(default)]
    pub rules: PolicyRules,

    /// Resource limits (max diff size, max files).
    #[serde(default)]
    pub limits: PolicyLimits,

    /// Audit configuration (HMAC key path, backend URL).
    #[serde(default)]
    pub audit: AuditConfig,
}

// ---------------------------------------------------------------------------
// PolicyRules
// ---------------------------------------------------------------------------

/// Container for all classification rules in a policy document.
///
/// Rules are grouped by enforcement action: deny (blocking), require_review
/// (must be reviewed before merge), and flag (warning only).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyRules {
    /// Rules that block the PR (commit status: failure).
    #[serde(rename = "deny", default)]
    pub deny_rules: Vec<DenyRule>,

    /// Rules that require human review (commit status: neutral).
    #[serde(rename = "require_review", default)]
    pub require_review_rules: Vec<ReviewRule>,

    /// Rules that warn without blocking (commit status: success with annotations).
    #[serde(rename = "flag", default)]
    pub flag_rules: Vec<FlagRule>,
}

impl PolicyRules {
    /// Total number of rules across all categories.
    pub fn total_rules(&self) -> usize {
        self.deny_rules.len() + self.require_review_rules.len() + self.flag_rules.len()
    }

    /// Whether there are any rules defined.
    pub fn is_empty(&self) -> bool {
        self.total_rules() == 0
    }
}

// ---------------------------------------------------------------------------
// DenyRule
// ---------------------------------------------------------------------------

/// A deny rule: if a changed file matches this pattern, the PR is blocked.
///
/// Deny rules produce blocking violations that prevent the PR from being merged
/// until resolved. These are the strongest enforcement level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenyRule {
    /// Human-readable name for the rule (e.g., "no-raw-sql-in-migrations").
    pub name: String,

    /// Description of what the rule enforces and why.
    pub description: String,

    /// Glob pattern matching file paths (e.g., "migrations/**", "*.sql").
    pub pattern: String,

    /// Severity level: critical (always block), high (block unless overridden).
    pub severity: Severity,

    /// Optional: only apply when the committing user is NOT in the exclude list.
    #[serde(default)]
    pub exclude_users: Vec<String>,
}

impl DenyRule {
    /// Whether this deny rule applies to a given user.
    ///
    /// Returns `false` if the user is in the `exclude_users` list.
    pub fn applies_to_user(&self, username: &str) -> bool {
        !self.exclude_users.iter().any(|u| u == username)
    }

    /// Whether this rule is a critical severity (always blocks).
    pub fn is_critical(&self) -> bool {
        self.severity == Severity::Critical
    }

    /// Whether this rule is a high severity (blocks unless overridden).
    pub fn is_high_severity(&self) -> bool {
        self.severity == Severity::High
    }
}

// ---------------------------------------------------------------------------
// ReviewRule
// ---------------------------------------------------------------------------

/// A review rule: matching files require manual review before merge.
///
/// Review rules produce non-blocking violations that set the commit status
/// to `neutral` and require a minimum number of reviewers to approve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRule {
    /// Human-readable name for the rule.
    pub name: String,

    /// Description of what the rule enforces and why.
    pub description: String,

    /// Glob pattern matching file paths (e.g., "src/auth/**", "*.key").
    pub pattern: String,

    /// Number of required reviewers for matching files.
    #[serde(default = "default_reviewers")]
    pub required_reviewers: u8,
}

fn default_reviewers() -> u8 {
    1
}

impl ReviewRule {
    /// Create a new review rule with default required reviewers (1).
    pub fn new(name: String, description: String, pattern: String) -> Self {
        Self {
            name,
            description,
            pattern,
            required_reviewers: default_reviewers(),
        }
    }
}

// ---------------------------------------------------------------------------
// FlagRule
// ---------------------------------------------------------------------------

/// A flag rule: matching files generate a warning annotation but don't block.
///
/// Flag rules produce violations that generate GitHub workflow annotations
/// (`::warning`) but do not block the PR or change the commit status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagRule {
    /// Human-readable name for the rule.
    pub name: String,

    /// Description of what the rule enforces and why.
    pub description: String,

    /// Glob pattern matching file paths.
    pub pattern: String,

    /// Optional custom message for the warning annotation.
    /// Defaults to a generated message including the rule name and file path.
    #[serde(default)]
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Severity level for deny rules.
///
/// Determines whether a rule always blocks (`Critical`) or can be
/// overridden (`High`). `Medium` and `Low` are informational for
/// reporting but do not affect blocking behavior.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Always blocks the PR. Cannot be overridden.
    Critical,
    /// Blocks the PR unless explicitly overridden.
    High,
    /// Informational severity — warning only, does not block.
    Medium,
    /// Lowest severity — informational only.
    Low,
}

impl Severity {
    /// Whether this severity level is blocking.
    pub fn is_blocking(&self) -> bool {
        matches!(self, Severity::Critical | Severity::High)
    }

    /// Get the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
        }
    }
}

// ---------------------------------------------------------------------------
// PolicyLimits
// ---------------------------------------------------------------------------

/// Resource limits enforced during policy evaluation.
///
/// These limits protect the action from resource exhaustion caused by
/// exceptionally large PRs. When limits are exceeded, the evaluator
/// processes what fits within limits and flags the rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyLimits {
    /// Maximum PR diff size in bytes (default: 1MB).
    pub max_diff_size: Option<u64>,

    /// Maximum number of changed files (default: 500).
    pub max_files: Option<usize>,

    /// Maximum changed lines per file (default: 2000).
    pub max_lines_per_file: Option<usize>,
}

impl Default for PolicyLimits {
    fn default() -> Self {
        Self {
            max_diff_size: Some(1_048_576), // 1 MB
            max_files: Some(500),
            max_lines_per_file: Some(2000),
        }
    }
}

impl PolicyLimits {
    /// Whether all limit fields are set (not None).
    pub fn is_fully_defined(&self) -> bool {
        self.max_diff_size.is_some()
            && self.max_files.is_some()
            && self.max_lines_per_file.is_some()
    }

    /// Apply a stricter limit from another `PolicyLimits` instance.
    ///
    /// For merge purposes: the tighter (smaller) value wins.
    pub fn apply_stricter(&mut self, other: &PolicyLimits) {
        self.max_diff_size = min_option(self.max_diff_size, other.max_diff_size, |a, b| a.min(b));
        self.max_files = min_option(self.max_files, other.max_files, |a, b| a.min(b));
        self.max_lines_per_file =
            min_option(self.max_lines_per_file, other.max_lines_per_file, |a, b| {
                a.min(b)
            });
    }
}

fn min_option<T: Ord>(a: Option<T>, b: Option<T>, f: fn(T, T) -> T) -> Option<T> {
    match (a, b) {
        (Some(a_val), Some(b_val)) => Some(f(a_val, b_val)),
        (Some(val), None) | (None, Some(val)) => Some(val),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// AuditConfig
// ---------------------------------------------------------------------------

/// Audit configuration for policy evaluation events.
///
/// When configured, policy evaluation results are signed with HMAC and
/// posted to an external audit backend for compliance tracking.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditConfig {
    /// Path to the HMAC signing key file.
    #[serde(default)]
    pub hmac_key_path: Option<String>,

    /// URL of the audit backend for posting evaluation records.
    #[serde(default)]
    pub backend_url: Option<String>,

    /// Whether to include full diff context in audit records.
    /// Default: false (only violations and metadata).
    #[serde(default = "default_include_diff")]
    pub include_diff: bool,
}

fn default_include_diff() -> bool {
    false
}

// ---------------------------------------------------------------------------
// PolicyViolation
// ---------------------------------------------------------------------------

/// A single policy violation detected during evaluation.
///
/// Each violation represents a file-change that matched a policy rule.
/// The violation carries the rule context, affected file, severity, and
/// a human-readable message suitable for GitHub workflow annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyViolation {
    /// The file matched a deny rule — this is a blocking violation.
    Deny {
        /// Name of the matching rule.
        rule: String,
        /// Description of the matching rule.
        description: String,
        /// Severity of the matching rule.
        severity: Severity,
        /// File path that triggered the violation.
        file: String,
        /// Human-readable violation message.
        message: String,
    },

    /// The file matched a review rule — human review is required.
    RequireReview {
        /// Name of the matching rule.
        rule: String,
        /// Description of the matching rule.
        description: String,
        /// File path that triggered the violation.
        file: String,
        /// Number of reviewers required.
        required_reviewers: u8,
    },

    /// The file matched a flag rule — warning only, no blocking.
    Flag {
        /// Name of the matching rule.
        rule: String,
        /// Description of the matching rule.
        description: String,
        /// File path that triggered the violation.
        file: String,
        /// Warning message for the annotation.
        message: String,
    },
}

impl PolicyViolation {
    /// The file path associated with this violation.
    pub fn file(&self) -> &str {
        match self {
            PolicyViolation::Deny { file, .. }
            | PolicyViolation::RequireReview { file, .. }
            | PolicyViolation::Flag { file, .. } => file,
        }
    }

    /// The rule name associated with this violation.
    pub fn rule_name(&self) -> &str {
        match self {
            PolicyViolation::Deny { rule, .. }
            | PolicyViolation::RequireReview { rule, .. }
            | PolicyViolation::Flag { rule, .. } => rule,
        }
    }

    /// Whether this violation is blocking (deny rule).
    pub fn is_blocking(&self) -> bool {
        matches!(self, PolicyViolation::Deny { .. })
    }

    /// Whether this violation requires human review.
    pub fn requires_review(&self) -> bool {
        matches!(self, PolicyViolation::RequireReview { .. })
    }

    /// The violation type as a string.
    pub fn violation_type(&self) -> &'static str {
        match self {
            PolicyViolation::Deny { .. } => "deny",
            PolicyViolation::RequireReview { .. } => "require_review",
            PolicyViolation::Flag { .. } => "flag",
        }
    }

    /// Format this violation as a GitHub workflow annotation.
    ///
    /// Returns `(annotation_type, message)` where annotation_type is
    /// "error" for deny, "warning" for flag, and "notice" for review.
    pub fn to_annotation(&self) -> (&'static str, String) {
        match self {
            PolicyViolation::Deny { message, .. } => ("error", message.clone()),
            PolicyViolation::Flag { message, .. } => ("warning", message.clone()),
            PolicyViolation::RequireReview { file, rule, .. } => (
                "notice",
                format!("File '{}' matches review rule '{}'", file, rule),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// PolicyResult
// ---------------------------------------------------------------------------

/// Aggregate result of evaluating a PR diff against policy rules.
///
/// Contains all violations grouped by type, plus summary flags for
/// quick decision-making by the action entrypoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResult {
    /// All violations found during evaluation.
    pub violations: Vec<PolicyViolation>,

    /// Whether any deny violations would block the PR.
    pub has_blocking_violations: bool,

    /// Whether there are review-required or flag violations.
    pub has_warnings: bool,

    /// Count of violations by type.
    pub counts: ViolationCounts,

    /// Whether the policy file itself was modified in this diff.
    pub policy_tamper_detected: bool,

    /// Metadata about the policy that was evaluated.
    pub policy_metadata: PolicyMetadata,
}

/// Metadata about the policy version and source for a policy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyMetadata {
    /// The policy document version string.
    pub policy_version: String,
    /// Whether an org policy was merged.
    pub org_policy_merged: bool,
    /// Number of deny rules evaluated.
    pub deny_rule_count: usize,
    /// Number of review rules evaluated.
    pub review_rule_count: usize,
    /// Number of flag rules evaluated.
    pub flag_rule_count: usize,
}

/// Counts of violations by enforcement type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ViolationCounts {
    /// Number of deny violations.
    pub deny: usize,
    /// Number of require_review violations.
    pub review: usize,
    /// Number of flag violations.
    pub flag: usize,
}

impl ViolationCounts {
    /// Total number of violations across all types.
    pub fn total(&self) -> usize {
        self.deny + self.review + self.flag
    }
}

impl PolicyResult {
    /// Create a new `PolicyResult` from a list of violations.
    pub fn new(
        violations: Vec<PolicyViolation>,
        policy_tamper_detected: bool,
        policy_metadata: PolicyMetadata,
    ) -> Self {
        let has_blocking = violations.iter().any(|v| v.is_blocking());
        let has_warnings = violations
            .iter()
            .any(|v| v.requires_review() || matches!(v, PolicyViolation::Flag { .. }));

        let counts = ViolationCounts {
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
        };

        Self {
            violations,
            has_blocking_violations: has_blocking,
            has_warnings,
            counts,
            policy_tamper_detected,
            policy_metadata,
        }
    }

    /// Returns true if the PR should be blocked (deny violations with blocking severity).
    pub fn should_block(&self, fail_on_violation: bool) -> bool {
        if fail_on_violation {
            self.has_blocking_violations || self.has_warnings
        } else {
            self.has_blocking_violations
        }
    }
}

// ---------------------------------------------------------------------------
// CompiledRules
// ---------------------------------------------------------------------------

/// Pre-compiled rule patterns for efficient evaluation.
///
/// Contains compiled glob patterns that are prepared during policy loading
/// and reused across all file evaluations for performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRules {
    /// Compiled deny rules with their glob matchers.
    pub deny: Vec<CompiledDenyRule>,

    /// Compiled review rules with their glob matchers.
    pub review: Vec<CompiledReviewRule>,

    /// Compiled flag rules with their glob matchers.
    pub flag: Vec<CompiledFlagRule>,
}

/// A deny rule with a compiled glob pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledDenyRule {
    pub name: String,
    pub description: String,
    pub severity: Severity,
    pub exclude_users: Vec<String>,
    /// The compiled glob pattern.
    pub pattern: String,
}

/// A review rule with a compiled glob pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledReviewRule {
    pub name: String,
    pub description: String,
    pub required_reviewers: u8,
    pub pattern: String,
}

/// A flag rule with a compiled glob pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledFlagRule {
    pub name: String,
    pub description: String,
    pub message: Option<String>,
    pub pattern: String,
}

impl CompiledRules {
    /// Create empty compiled rules.
    pub fn empty() -> Self {
        Self {
            deny: Vec::new(),
            review: Vec::new(),
            flag: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// OrgPolicyConfig
// ---------------------------------------------------------------------------

/// Configuration for loading an organization-level policy.
///
/// Organization policies are loaded from a separate location and merged
/// with repository-level policies to enforce minimum governance standards
/// across all repositories in an organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgPolicyConfig {
    /// URL or path to the organization policy file.
    pub org_policy_source: String,

    /// Whether to require the org policy (fail if missing).
    /// Default: false (warn-only if org policy not found).
    #[serde(default)]
    pub require_org_policy: bool,

    /// Override the org policy merge strategy.
    /// Default: "restrictive" (stricter rule wins).
    #[serde(default = "default_merge_strategy")]
    pub merge_strategy: MergeStrategy,
}

fn default_merge_strategy() -> MergeStrategy {
    MergeStrategy::Restrictive
}

/// Strategy for merging organization and repository policies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum MergeStrategy {
    /// The stricter/more restrictive rule wins. This is the default.
    #[default]
    Restrictive,
    /// Repository policy takes precedence over org policy.
    RepoPreferred,
    /// Organization policy takes precedence over repo policy.
    OrgPreferred,
}
