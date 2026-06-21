//! Data Transfer Objects for the Action Entrypoint module.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: Contract Freeze — DTO schemas for context building, mode resolution,
//! and dispatch operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for event processing and audit logging)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};

use crate::action_entrypoint::domain::{ActionContext, ActionMode, ActionOutput, GitHubEvent};

// ---------------------------------------------------------------------------
// Dispatch DTOs
// ---------------------------------------------------------------------------

/// Input for dispatching an action through the router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchInput {
    /// The fully built action context.
    pub context: ActionContext,

    /// Maximum time to wait for the dispatch in seconds.
    /// If `None`, uses default timeout.
    pub timeout_secs: Option<u64>,

    /// Whether to force dispatch even if the event type is not routable.
    pub force: bool,
}

/// Output from dispatching an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchOutput {
    /// The formatted action output.
    pub output: ActionOutput,
    /// The execution mode that was actually dispatched.
    pub mode: ActionMode,
    /// Duration of the dispatch in milliseconds.
    pub duration_ms: u64,
    /// Whether the dispatch was successful.
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Mode Resolution DTOs
// ---------------------------------------------------------------------------

/// Input for resolving the execution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveModeInput {
    /// The raw `INPUT_MODE` value from environment (if set).
    pub input_mode: Option<String>,
    /// The event name from `GITHUB_EVENT_NAME`.
    pub event_name: String,
    /// The event payload JSON value (for extracting intent from commands).
    pub event_payload: Option<serde_json::Value>,
    /// The raw `INPUT_INTENT` value from environment (if set).
    pub input_intent: Option<String>,
}

/// Output from resolving the execution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveModeOutput {
    /// The resolved execution mode.
    pub mode: ActionMode,
    /// The source of resolution (input, event_command, event_type, default).
    pub source: String,
    /// Whether the resolution was unambiguous.
    pub unambiguous: bool,
    /// Warnings from resolution (e.g., conflicting inputs).
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Context Building DTOs
// ---------------------------------------------------------------------------

/// Input for building an ActionContext from the environment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildContextInput {
    /// Override environment for testing (maps variable name → value).
    /// If `None`, reads from real `std::env::var`.
    pub env_override: Option<std::collections::HashMap<String, String>>,

    /// Override the workspace root path for testing.
    pub workspace_override: Option<String>,

    /// Override the event name for testing.
    pub event_name_override: Option<String>,

    /// Override the event payload path for testing.
    pub event_path_override: Option<String>,

    /// Override the event payload JSON content for testing.
    /// If set, skips reading from the event path file.
    pub event_payload_override: Option<String>,
}

/// Output from building an ActionContext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextOutput {
    /// The fully built action context.
    pub context: ActionContext,
    /// The raw event name from the environment.
    pub event_name: String,
    /// The raw input mode string (if present).
    pub input_mode: Option<String>,
    /// Warnings from context construction.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Event Parsing DTOs
// ---------------------------------------------------------------------------

/// Input for parsing a GitHub event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseEventInput {
    /// The event name (e.g., "workflow_dispatch", "issue_comment").
    pub event_name: String,
    /// Path to the event payload JSON file.
    pub event_path: String,
    /// Override JSON content for testing (if `None`, reads from file).
    pub content_override: Option<String>,
}

/// Output from parsing a GitHub event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseEventOutput {
    /// The parsed GitHub event.
    pub event: GitHubEvent,
    /// The raw event payload JSON (for further processing).
    pub raw_payload: serde_json::Value,
    /// File size of the event payload in bytes (if read from file).
    pub file_size_bytes: Option<u64>,
}

// ---------------------------------------------------------------------------
// Validation Loop DTOs
// ---------------------------------------------------------------------------

/// Configuration for the validation loop dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationLoopConfig {
    /// Maximum number of validation iterations.
    pub max_iterations: u32,
    /// Minimum quality score to accept (0.0 - 1.0).
    pub min_quality_score: Option<f64>,
    /// Whether to persist intermediate results.
    pub persist_intermediate: bool,
}

impl Default for ValidationLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 3,
            min_quality_score: None,
            persist_intermediate: false,
        }
    }
}
