//! Domain value objects for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md#domain
//! Implements: Contract Freeze — ToolInput, ToolResult, SideEffect value objects
//! Issue: #124
//!
//! These are the core value objects exchanged between the `Tool` trait and
//! its implementations. They live in the domain layer (GAP-A-21: they were
//! previously in application/dto, creating a domain -> application import in
//! tool_trait.rs). The application DTO module re-exports them for the service
//! traits, keeping the hexagonal dependency direction: application -> domain.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::tools::domain::error::ToolError;

// Tool Input/Output DTOs
// ---------------------------------------------------------------------------

/// Input parameters for a tool execution.
///
/// Each tool implementation deserializes the expected fields from
/// `params` and validates them. The `params` map is free-form JSON
/// to support different parameter schemas for different tools.
///
/// # Example
///
/// For a FileRead tool:
/// ```json
/// { "path": "src/main.rs" }
/// ```
///
/// For a RunCommand tool:
/// ```json
/// {
///   "command": "cargo build",
///   "timeout_secs": 60,
///   "cwd": "/workspace"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    /// Tool-specific parameters as a JSON object.
    /// The tool implementation is responsible for deserializing and
    /// validating the expected fields.
    pub params: HashMap<String, serde_json::Value>,

    /// Optional execution context for tracing and auditing.
    pub execution_id: Option<uuid::Uuid>,
}

impl ToolInput {
    /// Create a new ToolInput with the given parameters.
    pub fn new(params: HashMap<String, serde_json::Value>) -> Self {
        Self {
            params,
            execution_id: None,
        }
    }

    /// Create a new ToolInput with an execution ID for tracing.
    pub fn with_execution_id(
        params: HashMap<String, serde_json::Value>,
        execution_id: uuid::Uuid,
    ) -> Self {
        Self {
            params,
            execution_id: Some(execution_id),
        }
    }

    /// Extract a string parameter by key.
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.params
            .get(key)
            .and_then(|v| v.as_str().map(String::from))
    }

    /// Extract a u64 parameter by key.
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.params.get(key).and_then(|v| v.as_u64())
    }

    /// Extract a required string parameter, returning InvalidInput error if missing.
    pub fn require_string(&self, key: &str) -> Result<String, ToolError> {
        self.get_string(key)
            .ok_or_else(|| ToolError::InvalidInput(format!("Missing required parameter: {}", key)))
    }

    /// Extract a required u64 parameter, returning InvalidInput error if missing.
    pub fn require_u64(&self, key: &str) -> Result<u64, ToolError> {
        self.get_u64(key)
            .ok_or_else(|| ToolError::InvalidInput(format!("Missing required parameter: {}", key)))
    }
}

/// Output from a successful tool execution.
///
/// Contains the execution result including output text, exit code,
/// and any side effects produced (file writes, git commits, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Text output from the tool execution.
    pub output: String,

    /// Exit code (0 for success, non-zero for failure).
    pub exit_code: i32,

    /// Side effects produced by this execution.
    pub side_effects: Vec<SideEffect>,

    /// Duration of execution in milliseconds.
    pub duration_ms: u64,

    /// Whether the tool was executed in dry-run mode (preview, no side effects).
    pub dry_run: bool,
}

impl ToolResult {
    /// Create a successful ToolResult.
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            exit_code: 0,
            side_effects: Vec::new(),
            duration_ms: 0,
            dry_run: false,
        }
    }

    /// Check if execution was successful.
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }

    /// Check if any side effects were produced.
    pub fn has_side_effects(&self) -> bool {
        !self.side_effects.is_empty()
    }

    /// Get a preview of the output (truncated to N characters).
    pub fn output_preview(&self, max_chars: usize) -> &str {
        if self.output.len() > max_chars {
            &self.output[..max_chars]
        } else {
            &self.output
        }
    }
}

// ---------------------------------------------------------------------------
// SideEffect
// ---------------------------------------------------------------------------

/// A side effect produced by a tool execution.
///
/// Tracks all state changes (file writes, git operations, etc.)
/// for audit trail, undo operations, and observability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SideEffect {
    /// The path affected (file path, git ref, etc.).
    pub path: String,

    /// The type of side effect (e.g., "file_write", "git_commit").
    pub effect_type: String,

    /// Description of what changed for audit.
    pub description: String,

    /// The previous value/hash for undo support (optional).
    pub previous_hash: Option<String>,
}

impl SideEffect {
    /// Create a new side effect record.
    pub fn new(
        path: impl Into<String>,
        effect_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            effect_type: effect_type.into(),
            description: description.into(),
            previous_hash: None,
        }
    }

    /// Set the previous hash for undo support.
    pub fn with_previous_hash(mut self, hash: impl Into<String>) -> Self {
        self.previous_hash = Some(hash.into());
        self
    }
}
