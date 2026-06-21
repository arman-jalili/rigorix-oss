//! Error types for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: Contract Freeze — ActionError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `ActionError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Every variant maps to a GitHub Action annotation level (error/warning/notice)
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

use crate::shared::github_client::GitHubClientError;

/// Errors that can occur during action entrypoint dispatch.
///
/// Each variant includes enough context to produce a GitHub Action
/// annotation (`::error file=...::message`) or a structured error
/// response.
#[derive(Debug, Error)]
pub enum ActionError {
    /// No execution mode could be resolved from inputs/event.
    #[error("Cannot determine execution mode: {detail}")]
    ModeResolutionError {
        /// Explanation of why mode resolution failed.
        detail: String,
        /// The value of `INPUT_MODE` if set.
        input_mode: Option<String>,
        /// The event name from `GITHUB_EVENT_NAME`.
        event_name: Option<String>,
    },

    /// The GitHub event type is not supported for routing.
    #[error("Unsupported event type '{event_name}': {detail}")]
    UnsupportedEvent {
        /// The event name from `GITHUB_EVENT_NAME`.
        event_name: String,
        /// Explanation of why the event is not supported.
        detail: String,
    },

    /// Missing required context (workspace, token, etc.).
    #[error("Missing required context: {detail}")]
    MissingContext {
        /// Description of what context is missing.
        detail: String,
        /// The environment variable name that was checked.
        env_var: Option<String>,
    },

    /// Engine orchestrator call failed.
    #[error("Engine orchestrator error: {detail}")]
    EngineError {
        /// Description of the engine error.
        detail: String,
        /// Optional engine-side error code.
        code: Option<String>,
    },

    /// Engine validation loop call failed.
    #[error("Validation loop error: {detail}")]
    ValidationLoopError {
        /// Description of the validation loop error.
        detail: String,
        /// Number of completed iterations before failure.
        iterations_completed: Option<u32>,
    },

    /// The workspace root path is invalid or inaccessible.
    #[error("Invalid workspace root '{path}': {detail}")]
    InvalidWorkspaceRoot {
        /// The resolved workspace root path.
        path: String,
        /// Details about the validation failure.
        detail: String,
    },

    /// Context repository read/write failure.
    #[error("Context repository error: {detail}")]
    ContextRepositoryError {
        /// Description of the repository error.
        detail: String,
    },

    /// Error formatting or writing action output.
    #[error("Output formatting error: {detail}")]
    OutputError {
        /// Description of the output error.
        detail: String,
    },

    /// GitHub API client error (PR comments, status checks).
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

impl ActionError {
    /// Whether the error is retriable (transient infrastructure failures).
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ActionError::Io(_)
                | ActionError::GitHubApi(_)
                | ActionError::EngineError { .. }
                | ActionError::ValidationLoopError { .. }
        )
    }

    /// The GitHub Action annotation level for this error.
    ///
    /// - `Error` → `::error` — fails the step
    /// - `Warning` → `::warning` — visible but non-fatal
    pub fn annotation_level(&self) -> &'static str {
        match self {
            ActionError::ModeResolutionError { .. }
            | ActionError::MissingContext { .. }
            | ActionError::InvalidWorkspaceRoot { .. }
            | ActionError::Internal { .. } => "error",
            ActionError::UnsupportedEvent { .. } | ActionError::OutputError { .. } => "warning",
            ActionError::EngineError { .. }
            | ActionError::ValidationLoopError { .. }
            | ActionError::ContextRepositoryError { .. }
            | ActionError::GitHubApi(_)
            | ActionError::Io(_)
            | ActionError::Json(_) => "error",
        }
    }

    /// The exit code to use when this error terminates the action.
    ///
    /// - 1: Fatal error (action failed)
    /// - 0: Non-fatal (action completed with warnings)
    pub fn exit_code(&self) -> i32 {
        if self.annotation_level() == "warning" {
            0
        } else {
            1
        }
    }
}
