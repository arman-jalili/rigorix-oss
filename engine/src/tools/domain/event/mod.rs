//! Event payload schemas for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md#events
//! Implements: Contract Freeze — ToolEvent payload schemas
//! Issue: #124
//!
//! These events are emitted whenever a tool is executed, encounters an error,
//! or produces side effects. Consumers (audit trail, console printer, TUI)
//! subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::tools::domain::error::ToolError;

/// Events emitted by the Tool System module.
///
/// Wrapped in `ExecutionEvent::ToolSystem(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolEvent {
    /// A tool was registered in the ToolRegistry.
    ToolRegistered {
        /// The tool's unique name.
        tool_name: String,
        /// Total number of registered tools after this registration.
        total_tools: usize,
    },

    /// A tool execution was requested.
    ToolExecutionRequested {
        /// The tool's unique name.
        tool_name: String,
        /// Execution ID for tracing.
        execution_id: uuid::Uuid,
        /// Input parameters used for execution.
        input_params: HashMap<String, serde_json::Value>,
        /// Risk level assigned to this execution.
        risk_level: String,
    },

    /// A tool executed successfully.
    ToolExecuted {
        /// The tool's unique name.
        tool_name: String,
        /// Execution ID for tracing.
        execution_id: uuid::Uuid,
        /// Duration of execution in milliseconds.
        duration_ms: u64,
        /// Exit code from the tool execution.
        exit_code: i32,
        /// Whether side effects were produced.
        has_side_effects: bool,
        /// Output truncated to first 200 characters for logging.
        output_preview: String,
    },

    /// A tool was executed via the risk gate with the gating result.
    ToolExecutionGated {
        /// The tool's unique name.
        tool_name: String,
        /// Risk level assigned to this tool.
        risk_level: String,
        /// The gating action applied (auto_execute, require_confirmation, dry_run).
        gating_action: String,
        /// Whether execution was allowed through the gate.
        allowed: bool,
    },

    /// Tool execution failed with an error.
    ToolExecutionFailed {
        /// The tool's unique name.
        tool_name: String,
        /// Execution ID for tracing.
        execution_id: uuid::Uuid,
        /// Duration of execution in milliseconds (if execution started).
        duration_ms: Option<u64>,
        /// The error that occurred.
        error: ToolError,
    },

    /// A side effect was produced by a tool execution.
    ToolSideEffect {
        /// The tool's unique name.
        tool_name: String,
        /// Execution ID for tracing.
        execution_id: uuid::Uuid,
        /// Path affected by the side effect (file path, git ref, etc.).
        path: String,
        /// Type of side effect (e.g., "file_write", "git_commit").
        effect_type: String,
        /// Description of the side effect for audit.
        description: String,
    },

    /// A tool path was denied by security policy.
    ToolPathDenied {
        /// The tool's unique name.
        tool_name: String,
        /// The path that was denied.
        path: String,
        /// Reason for denial.
        reason: String,
    },
}
