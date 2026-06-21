//! Error types for the CI Integration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md
//! Implements: Contract Freeze — CiIntegrationError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `CiIntegrationError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

use crate::shared::github_client::GitHubClientError;

/// Errors that can occur during CI integration operations.
#[derive(Debug, Error)]
pub enum CiIntegrationError {
    /// Failed to create or update a commit status check.
    #[error("Failed to create status check for commit '{commit_sha}': {detail}")]
    StatusCheckFailed {
        /// The commit SHA.
        commit_sha: String,
        /// Error details.
        detail: String,
    },

    /// Failed to post or update a PR comment.
    #[error("Failed to post PR comment on #{issue_number}: {detail}")]
    PrCommentFailed {
        /// The issue or PR number.
        issue_number: u64,
        /// Error details.
        detail: String,
    },

    /// Failed to find an existing bot comment to update.
    #[error("Bot comment not found on #{issue_number}")]
    BotCommentNotFound {
        /// The issue or PR number.
        issue_number: u64,
    },

    /// Failed to add or remove issue labels.
    #[error("Failed to update labels on #{issue_number}: {detail}")]
    LabelUpdateFailed {
        /// The issue or PR number.
        issue_number: u64,
        /// Error details.
        detail: String,
    },

    /// The execution ID was not found for tracking.
    #[error("Execution not found: {execution_id}")]
    ExecutionNotFound {
        /// The execution UUID.
        execution_id: String,
    },

    /// Duplicate execution detected (idempotency check).
    #[error("Duplicate execution '{execution_id}' already recorded for PR #{pr_number}")]
    DuplicateExecution {
        /// The execution UUID.
        execution_id: String,
        /// The PR number.
        pr_number: u64,
    },

    /// GitHub API rate limit would be exceeded.
    #[error("GitHub API rate limit exceeded. Retry after {retry_after_secs}s")]
    RateLimitExceeded {
        /// Seconds to wait before retrying.
        retry_after_secs: u64,
    },

    /// Invalid arguments passed to a CI integration method.
    #[error("Invalid argument: {detail}")]
    InvalidArgument {
        /// Error details.
        detail: String,
    },

    /// GitHub API client error (wraps `shared::GitHubClientError`).
    #[error("GitHub API error: {0}")]
    GitHubApi(#[from] GitHubClientError),

    /// IO error (file system, environment).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Internal invariant violation (should not happen).
    #[error("Internal error: {detail}")]
    Internal {
        /// Error description.
        detail: String,
    },
}

impl CiIntegrationError {
    /// Whether the error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            CiIntegrationError::GitHubApi(_)
                | CiIntegrationError::RateLimitExceeded { .. }
                | CiIntegrationError::Io(_)
        )
    }
}
