//! CLI event payload schemas.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#events
//! Implements: Contract Freeze — CLI event schemas
//! Issue: issue-contract-freeze
//!
//! Events emitted by the CLI layer. These wrap or translate engine events
//! for CLI output rendering and session tracking.
//!
//! # Contract (Frozen)
//! - Each variant is a serializable struct with derived Debug
//! - Variants are additive only (no removal without architecture review)
//! - All events carry at least a session_id and timestamp

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::observability::domain::event::observability::ObservabilityEvent;

/// Events emitted by the CLI boundary during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CliEvent {
    /// A CLI command was parsed and dispatched.
    CommandDispatched(CommandDispatchedPayload),

    /// An execution session was started.
    SessionStarted(SessionStartedPayload),

    /// An execution session completed.
    SessionCompleted(SessionCompletedPayload),

    /// An error occurred during CLI processing.
    CliError(CliErrorPayload),

    /// TUI was enabled or disabled.
    TuiStatus(TuiStatusPayload),

    /// An observability event occurred (tracing init, health check, metrics).
    Observability(ObservabilityEvent),
}

/// Payload for command dispatch events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDispatchedPayload {
    /// The session identifier.
    pub session_id: String,
    /// The command that was dispatched (e.g., "run", "plan").
    pub command: String,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}

/// Payload for session start events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartedPayload {
    /// The session identifier.
    pub session_id: String,
    /// The command that started the session.
    pub command: String,
    /// The template ID being executed, if applicable.
    pub template_id: Option<String>,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}

/// The final status of a completed execution session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionOutcome {
    /// The session completed successfully.
    #[serde(rename = "completed")]
    Completed,
    /// The session failed with errors.
    #[serde(rename = "failed")]
    Failed,
    /// The session was cancelled by the user.
    #[serde(rename = "cancelled")]
    Cancelled,
    /// The session timed out.
    #[serde(rename = "timed_out")]
    TimedOut,
}

impl fmt::Display for SessionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionOutcome::Completed => write!(f, "completed"),
            SessionOutcome::Failed => write!(f, "failed"),
            SessionOutcome::Cancelled => write!(f, "cancelled"),
            SessionOutcome::TimedOut => write!(f, "timed_out"),
        }
    }
}

/// Payload for session completion events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCompletedPayload {
    /// The session identifier.
    pub session_id: String,
    /// The final outcome of the session.
    pub outcome: SessionOutcome,
    /// Duration of the session in milliseconds.
    pub duration_ms: u64,
    /// Number of nodes completed.
    pub nodes_completed: u32,
    /// Number of nodes that failed.
    pub nodes_failed: u32,
    /// Number of nodes that were skipped.
    pub nodes_skipped: u32,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}

/// Payload for CLI error events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliErrorPayload {
    /// The session identifier (may be empty if error occurred before session start).
    pub session_id: Option<String>,
    /// The error code.
    pub code: String,
    /// The error message.
    pub message: String,
    /// A suggestion for resolving the error, if applicable.
    pub suggestion: Option<String>,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}

/// Payload for TUI status events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiStatusPayload {
    /// The session identifier.
    pub session_id: String,
    /// Whether the TUI is active.
    pub active: bool,
    /// Reason if TUI is not active (e.g., "no_tty", "force_disabled").
    pub reason: Option<String>,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}
