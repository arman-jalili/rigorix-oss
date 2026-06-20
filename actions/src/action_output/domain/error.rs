//! Error types for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — ActionOutputError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `ActionOutputError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

/// Errors that can occur during GitHub Actions output formatting.
#[derive(Debug, Error)]
pub enum ActionOutputError {
    /// A required environment variable was not set.
    #[error("Missing required environment variable: {0}")]
    MissingEnv(String),

    /// Failed to write to the output/annotation/summary file.
    #[error("Failed to write output: {detail}")]
    WriteError {
        /// The destination that failed.
        destination: String,
        /// Additional error details.
        detail: String,
    },

    /// Failed to format output content.
    #[error("Failed to format output: {detail}")]
    FormatError {
        /// What was being formatted.
        context: String,
        /// Formatting error description.
        detail: String,
    },

    /// GitHub API error from posting comments.
    #[error("GitHub API error on {endpoint}: HTTP {status_code} — {response}")]
    GitHubApiError {
        /// The API endpoint that failed.
        endpoint: String,
        /// HTTP status code.
        status_code: u16,
        /// Error response body.
        response: String,
    },

    /// GitHub token is required but not available.
    #[error("GitHub token required but not available")]
    MissingToken,

    /// PR context is required but not available.
    #[error("PR context required — not running in a pull request")]
    MissingPrContext,

    /// Output variable value exceeds maximum allowed length.
    #[error("Output variable '{name}' exceeds maximum length of {max_length} characters")]
    VariableTooLong {
        /// The variable name.
        name: String,
        /// Current value length.
        actual_length: usize,
        /// Maximum allowed length.
        max_length: usize,
    },

    /// Invalid output variable name (contains invalid characters).
    #[error("Invalid output variable name: '{name}' — must match [a-z_][a-z0-9_]*")]
    InvalidVariableName {
        /// The variable name.
        name: String,
    },

    /// IO error (file system).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Template failure details cannot be formatted (non-fatal).
    #[error("Failed to format template failure: {detail}")]
    UnsupportedFailureType {
        /// The failure type that couldn't be formatted.
        failure_type: String,
        /// Additional details.
        detail: String,
    },

    /// Internal invariant violation (should not happen).
    #[error("Internal error: {detail}")]
    Internal {
        /// Error description.
        detail: String,
    },
}

impl ActionOutputError {
    /// Whether the error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ActionOutputError::Io(_)
                | ActionOutputError::GitHubApiError { .. }
                | ActionOutputError::WriteError { .. }
        )
    }

    /// Whether the error is fatal (should fail the step).
    pub fn is_fatal(&self) -> bool {
        !self.is_retriable()
    }
}
