//! Data Transfer Objects for the Hook System module.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: Contract Freeze — DTO schemas for hook execution operations
//! Issue: #410
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::hooks::domain::event::HookEvent;
use crate::hooks::domain::result::HookRunResult;

// ---------------------------------------------------------------------------
// Run PreToolUse Hooks DTOs
// ---------------------------------------------------------------------------

/// Input for executing PreToolUse hooks for a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPreToolUseInput {
    /// Name of the tool being invoked.
    pub tool_name: String,

    /// The original tool input as a JSON value.
    pub tool_input: serde_json::Value,

    /// The session/execution ID for correlation.
    pub session_id: String,

    /// The workspace root directory path.
    pub workspace_root: String,
}

/// Output from executing PreToolUse hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPreToolUseOutput {
    /// The aggregated result from all PreToolUse hooks.
    pub result: HookRunResult,
}

// ---------------------------------------------------------------------------
// Run PostToolUse Hooks DTOs
// ---------------------------------------------------------------------------

/// Input for executing PostToolUse hooks after successful tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPostToolUseInput {
    /// Name of the tool that was executed.
    pub tool_name: String,

    /// The original tool input used for execution.
    pub tool_input: serde_json::Value,

    /// The output produced by the tool (stdout + stderr combined).
    pub tool_output: String,

    /// The session/execution ID for correlation.
    pub session_id: String,

    /// The workspace root directory path.
    pub workspace_root: String,
}

/// Output from executing PostToolUse hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPostToolUseOutput {
    /// The aggregated result from all PostToolUse hooks.
    pub result: HookRunResult,
}

// ---------------------------------------------------------------------------
// Run PostToolUseFailure Hooks DTOs
// ---------------------------------------------------------------------------

/// Input for executing PostToolUseFailure hooks after failed tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPostToolUseFailureInput {
    /// Name of the tool that failed.
    pub tool_name: String,

    /// The original tool input used for execution.
    pub tool_input: serde_json::Value,

    /// The error output from the failed tool execution.
    pub error_output: String,

    /// The session/execution ID for correlation.
    pub session_id: String,

    /// The workspace root directory path.
    pub workspace_root: String,
}

/// Output from executing PostToolUseFailure hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPostToolUseFailureOutput {
    /// The aggregated result from all PostToolUseFailure hooks.
    pub result: HookRunResult,
}

// ---------------------------------------------------------------------------
// General Run Hooks DTO
// ---------------------------------------------------------------------------

/// Input for running hooks for any lifecycle event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHooksInput {
    /// The lifecycle event to run hooks for.
    pub event: HookEvent,

    /// Name of the tool being intercepted.
    pub tool_name: String,

    /// The tool input as a JSON value.
    pub tool_input: serde_json::Value,

    /// The tool output or error output (for Post* events).
    /// Empty for PreToolUse.
    #[serde(default)]
    pub tool_output: String,

    /// The session/execution ID for correlation.
    pub session_id: String,

    /// The workspace root directory path.
    pub workspace_root: String,
}

/// Output from running hooks for any lifecycle event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHooksOutput {
    /// The aggregated result from all hooks for the event.
    pub result: HookRunResult,
}

// ---------------------------------------------------------------------------
// Hook Runner Configuration Input DTO
// ---------------------------------------------------------------------------

/// Input for configuring a HookRunner instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRunnerConfigInput {
    /// Comma-separated hook command lists.
    /// These override any existing commands for their respective events.
    pub pre_tool_use: Vec<String>,

    pub post_tool_use: Vec<String>,

    pub post_tool_use_failure: Vec<String>,
}

// ---------------------------------------------------------------------------
// Hook Runner Status DTO
// ---------------------------------------------------------------------------

/// Status information about the HookRunner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRunnerStatus {
    /// Number of registered PreToolUse hooks.
    pub pre_tool_use_count: usize,

    /// Number of registered PostToolUse hooks.
    pub post_tool_use_count: usize,

    /// Number of registered PostToolUseFailure hooks.
    pub post_tool_use_failure_count: usize,

    /// Total number of registered hooks across all events.
    pub total_hook_count: usize,

    /// Whether the runner is actively processing hooks.
    pub is_running: bool,

    /// Current timeout setting in seconds.
    pub timeout_secs: u64,
}
