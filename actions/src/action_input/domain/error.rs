//! Error types for the Action Input bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
//! Implements: Contract Freeze — ActionInputError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `ActionInputError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

use crate::shared::github_client::GitHubClientError;

/// Errors that can occur during GitHub Action input parsing.
#[derive(Debug, Error)]
pub enum ActionInputError {
    /// A required input variable was not provided.
    #[error("Missing required input: {0}")]
    MissingRequiredInput(String),

    /// An input value could not be parsed into the expected type.
    #[error("Invalid input value for '{field}': expected {expected_type}, got '{value}'")]
    InvalidInputValue {
        /// The input field name (e.g. "INPUT_MAX_LLM_CALLS").
        field: String,
        /// The expected Rust type.
        expected_type: &'static str,
        /// The raw string value that failed to parse.
        value: String,
    },

    /// The `GITHUB_EVENT_PATH` file was not found or unreadable.
    #[error("GitHub event payload not found at '{path}': {detail}")]
    EventPayloadNotFound {
        /// The expected path to the event payload file.
        path: String,
        /// Additional details about the failure.
        detail: String,
    },

    /// Failed to parse the GitHub event JSON payload.
    #[error("Failed to parse GitHub event payload: {detail}")]
    EventPayloadParseError {
        /// Parse error description.
        detail: String,
        /// Source line number if available.
        line: Option<u32>,
    },

    /// Unknown or unsupported GitHub event type.
    #[error("Unknown or unsupported GitHub event type: {event_name}")]
    UnsupportedEventType {
        /// The value of `GITHUB_EVENT_NAME`.
        event_name: String,
    },

    /// The `action.yml` file was not found or unreadable.
    #[error("action.yml not found: {detail}")]
    ActionYmlNotFound {
        /// Additional details about the failure.
        detail: String,
    },

    /// Failed to parse `action.yml`.
    #[error("Failed to parse action.yml: {detail}")]
    ActionYmlParseError {
        /// Parse error description.
        detail: String,
    },

    /// IO error (file system, environment).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML serialization/deserialization error.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// GitHub API client error.
    #[error("GitHub API error: {0}")]
    GitHubApi(#[from] GitHubClientError),

    /// Environment variable error (e.g., missing GITHUB_WORKSPACE).
    #[error("Environment error: {detail}")]
    EnvironmentError {
        /// Error description.
        detail: String,
    },

    /// Internal invariant violation (should not happen).
    #[error("Internal error: {detail}")]
    Internal {
        /// Error description.
        detail: String,
    },
}

impl ActionInputError {
    /// Whether the error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ActionInputError::Io(_)
                | ActionInputError::GitHubApi(_)
                | ActionInputError::EventPayloadNotFound { .. }
        )
    }
}
