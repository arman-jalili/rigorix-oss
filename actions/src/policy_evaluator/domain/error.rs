//! Error types for the Policy Evaluator bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#error
//! Implements: Contract Freeze — PolicyError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `PolicyError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

use crate::shared::github_client::GitHubClientError;

/// Errors that can occur during policy evaluation.
#[derive(Debug, Error)]
pub enum PolicyError {
    /// The policy file was not found at the expected path.
    #[error("Policy file not found: '{path}' (ref: {reference})")]
    FileNotFound {
        /// The policy file path that was searched.
        path: String,
        /// The git ref (branch/commit) that was searched.
        reference: String,
    },

    /// The policy file had invalid TOML syntax or structure.
    #[error("Invalid policy syntax: {detail}")]
    InvalidSyntax {
        /// Description of the syntax error.
        detail: String,
        /// Line number where the error occurred (if known).
        line: Option<usize>,
    },

    /// The policy version is not supported.
    #[error("Unsupported policy version: '{version}' (supported: {supported})")]
    UnsupportedVersion {
        /// The version found in the policy file.
        version: String,
        /// The supported version range.
        supported: String,
    },

    /// A glob pattern in a rule failed to compile.
    #[error("Invalid glob pattern in rule '{rule}': '{pattern}' — {detail}")]
    InvalidGlobPattern {
        /// The name of the rule with the invalid pattern.
        rule: String,
        /// The invalid pattern string.
        pattern: String,
        /// Details about why the pattern is invalid.
        detail: String,
    },

    /// A rule has a duplicate name (name collision).
    #[error("Duplicate rule name: '{name}' appears in multiple rule categories")]
    DuplicateRuleName {
        /// The duplicate rule name.
        name: String,
        /// The rule categories where the name appears.
        categories: Vec<String>,
    },

    /// Evaluation encountered an internal error.
    #[error("Evaluation error: {detail}")]
    EvaluationError {
        /// Description of the evaluation error.
        detail: String,
    },

    /// The policy limit configuration is invalid.
    #[error("Invalid policy limits: {detail}")]
    InvalidLimits {
        /// Description of the configuration issue.
        detail: String,
    },

    /// Organization policy could not be loaded.
    #[error("Failed to load organization policy: {detail}")]
    OrgPolicyLoadError {
        /// Description of the load failure.
        detail: String,
    },

    /// Organization policy merge failed.
    #[error("Policy merge error: {detail}")]
    PolicyMergeError {
        /// Description of the merge failure.
        detail: String,
    },

    /// Policy tamper detected — the PR modifies the policy file.
    #[error("Policy tamper detected: PR modifies '{path}' — requires admin review")]
    PolicyTamperDetected {
        /// The policy file path that was modified.
        path: String,
    },

    /// IO error (file system, network).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML deserialization error.
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// GitHub API client error.
    #[error("GitHub API error: {0}")]
    GitHubApi(#[from] GitHubClientError),

    /// Internal invariant violation (should not happen).
    #[error("Internal error: {detail}")]
    Internal {
        /// Error description.
        detail: String,
    },
}

impl PolicyError {
    /// Whether the error is retriable (transient failure).
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            PolicyError::Io(_) | PolicyError::GitHubApi(_) | PolicyError::OrgPolicyLoadError { .. }
        )
    }

    /// Whether the error is a security concern (tamper, bad config).
    pub fn is_security_relevant(&self) -> bool {
        matches!(
            self,
            PolicyError::PolicyTamperDetected { .. } | PolicyError::InvalidGlobPattern { .. }
        )
    }

    /// Whether the error should block the action from proceeding.
    pub fn is_blocking(&self) -> bool {
        match self {
            PolicyError::FileNotFound { .. }
            | PolicyError::InvalidSyntax { .. }
            | PolicyError::UnsupportedVersion { .. }
            | PolicyError::PolicyTamperDetected { .. }
            | PolicyError::InvalidLimits { .. } => true,
            _ => false,
        }
    }
}
