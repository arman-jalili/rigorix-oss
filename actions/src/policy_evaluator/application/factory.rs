//! Factory interfaces for constructing Policy Evaluator domain objects.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Implements: Contract Freeze — PolicyDocumentFactory, RulesFactory,
//! CompiledRulesFactory, PolicyResultFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::policy_evaluator::domain::{
    CompiledRules, DenyRule, FlagRule, PolicyDocument, PolicyError, PolicyLimits, PolicyResult,
    PolicyRules, ReviewRule, Severity,
};

/// Factory for constructing `PolicyDocument` instances.
///
/// Handles creation of policy documents from various sources:
/// - Parsed TOML content
/// - Default policies with no rules
/// - Policy documents with specific configurations
#[async_trait]
pub trait PolicyDocumentFactory: Send + Sync {
    /// Build a `PolicyDocument` from raw TOML content.
    ///
    /// Parses the TOML string, validates the structure,
    /// and returns a fully configured `PolicyDocument`.
    async fn build_from_toml(&self, content: &str) -> Result<PolicyDocument, PolicyError>;

    /// Create a default policy document with no rules.
    ///
    /// Version set to latest supported, empty rules, default limits.
    async fn default(&self) -> PolicyDocument;

    /// Create a `PolicyDocument` with the given version and rules.
    async fn with_rules(
        &self,
        version: &str,
        rules: PolicyRules,
        limits: Option<PolicyLimits>,
    ) -> PolicyDocument;

    /// Validate a `PolicyDocument` structure.
    ///
    /// Checks version support, rule consistency, and limit validity.
    async fn validate(&self, policy: &PolicyDocument) -> Result<(), PolicyError>;

    /// Clone a `PolicyDocument` with overridden limits.
    async fn with_limits(&self, policy: &PolicyDocument, limits: PolicyLimits) -> PolicyDocument;
}

/// Factory for constructing rule instances.
///
/// Handles creation of deny, review, and flag rules with proper
/// default values and validation.
#[async_trait]
pub trait RulesFactory: Send + Sync {
    /// Build a `DenyRule` with validated fields.
    async fn build_deny_rule(
        &self,
        name: &str,
        description: &str,
        pattern: &str,
        severity: Severity,
        exclude_users: Vec<String>,
    ) -> Result<DenyRule, PolicyError>;

    /// Build a `ReviewRule` with validated fields.
    async fn build_review_rule(
        &self,
        name: &str,
        description: &str,
        pattern: &str,
        required_reviewers: u8,
    ) -> Result<ReviewRule, PolicyError>;

    /// Build a `FlagRule` with validated fields.
    async fn build_flag_rule(
        &self,
        name: &str,
        description: &str,
        pattern: &str,
        message: Option<String>,
    ) -> Result<FlagRule, PolicyError>;

    /// Parse severity from a string.
    ///
    /// Valid values: "critical", "high", "medium", "low" (case-insensitive).
    async fn parse_severity(&self, severity: &str) -> Result<Severity, PolicyError>;

    /// Validate a glob pattern string.
    ///
    /// Returns an error if the pattern is invalid.
    async fn validate_pattern(&self, pattern: &str) -> Result<(), PolicyError>;

    /// Validate that all patterns in a rule set are valid.
    async fn validate_rule_patterns(&self, rules: &PolicyRules) -> Result<(), PolicyError>;
}

/// Factory for constructing `CompiledRules` from `PolicyRules`.
///
/// Handles compilation of glob patterns for efficient matching
/// during policy evaluation.
#[async_trait]
pub trait CompiledRulesFactory: Send + Sync {
    /// Build `CompiledRules` from a `PolicyDocument`.
    ///
    /// Compiles all glob patterns from deny, review, and flag rules.
    async fn build_from_policy(
        &self,
        policy: &PolicyDocument,
    ) -> Result<CompiledRules, PolicyError>;

    /// Compile a single deny rule pattern.
    async fn compile_deny_rule(
        &self,
        rule: &DenyRule,
    ) -> Result<crate::policy_evaluator::domain::CompiledDenyRule, PolicyError>;

    /// Compile a single review rule pattern.
    async fn compile_review_rule(
        &self,
        rule: &ReviewRule,
    ) -> Result<crate::policy_evaluator::domain::CompiledReviewRule, PolicyError>;

    /// Compile a single flag rule pattern.
    async fn compile_flag_rule(
        &self,
        rule: &FlagRule,
    ) -> Result<crate::policy_evaluator::domain::CompiledFlagRule, PolicyError>;

    /// Create empty compiled rules (no rules).
    fn empty(&self) -> CompiledRules;
}

/// Factory for constructing `PolicyResult` instances.
///
/// Handles creation of evaluation results with computed
/// aggregate statistics.
#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait PolicyResultFactory: Send + Sync {
    /// Build a `PolicyResult` from a list of violations.
    async fn build(
        &self,
        violations: Vec<crate::policy_evaluator::domain::PolicyViolation>,
        policy_tamper_detected: bool,
        policy_version: &str,
        deny_rule_count: usize,
        review_rule_count: usize,
        flag_rule_count: usize,
        org_policy_merged: bool,
    ) -> PolicyResult;

    /// Build an empty `PolicyResult` with no violations.
    async fn empty(&self, policy_version: &str, org_policy_merged: bool) -> PolicyResult;
}
