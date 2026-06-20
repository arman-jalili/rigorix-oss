//! Event payload schemas for the Action Input bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
//! Implements: Contract Freeze — ActionInputEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the EventBus whenever inputs are parsed,
//! commands are detected, or CI environment is resolved. Consumers
//! (audit, console printer, action-entrypoint) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - Events are serializable for audit logging

use serde::{Deserialize, Serialize};

/// Events emitted by the Action Input module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionInputEvent {
    /// Inputs were successfully parsed from the environment.
    InputsParsed {
        /// Which mode was resolved (or None if ambiguous).
        mode: Option<String>,
        /// Whether intent was provided.
        has_intent: bool,
        /// Number of non-default input fields set.
        inputs_count: u32,
    },

    /// A `/rigorix` slash command was detected in a comment.
    CommentCommandDetected {
        /// The issue or PR number where the comment was posted.
        issue_number: u64,
        /// The command type (run, validate, plan, status, retry, help).
        command_type: String,
        /// The commenter's username.
        commenter: String,
    },

    /// CI environment was detected.
    CiEnvironmentDetected {
        /// Whether we're running in CI.
        is_ci: bool,
        /// The CI type (GitHubActions or Local).
        ci_type: String,
        /// The resolved permission mode.
        permission_mode: String,
    },

    /// Configuration was loaded and merged.
    ConfigLoaded {
        /// Whether environment overrides were applied.
        has_env_overrides: bool,
        /// Whether action.yml defaults were found.
        has_yml_defaults: bool,
        /// Whether CLI overrides were applied.
        has_cli_overrides: bool,
    },

    /// An input parsing error occurred (non-fatal warning).
    InputWarning {
        /// The field that produced the warning.
        field: String,
        /// Warning message.
        message: String,
    },

    /// GitHub event payload was parsed.
    EventParsed {
        /// The event type (workflow_dispatch, issue_comment, pull_request, push, unknown).
        event_type: String,
        /// Whether parsing succeeded.
        success: bool,
    },
}
