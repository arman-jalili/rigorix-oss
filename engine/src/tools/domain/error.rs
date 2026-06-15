//! ToolError — structured error type for tool execution failures.
//!
//! @canonical .pi/architecture/modules/tool-system.md#errors
//! Implements: Contract Freeze — ToolError enum
//! Issue: #124
//!
//! Defines the structured error types that tools can return. Each variant
//! carries sufficient context for the execution engine to make retry,
//! fallback, and reporting decisions.
//!
//! # Contract (Frozen)
//! - All public variants, their fields, and Display implementations are frozen
//! - New variants require ADR approval and interface review
//! - Serialization is stable for API responses

use serde::{Deserialize, Serialize};

/// Structured error type for tool execution failures.
///
/// Emitted when a tool fails during execution. The error variant carries
/// machine-readable context for programmatic handling (retry, fallback,
/// reporting) and a human-readable message via `Display`/`Error`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum ToolError {
    /// The input parameters were malformed, missing required fields, or
    /// failed type validation.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Tool execution failed at runtime (e.g., file not found, command
    /// returned non-zero exit code, network error).
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// The requested tool was not found in the registry.
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// The tool attempted to access a path that is denied by policy
    /// (e.g., writing outside the workspace root).
    #[error("Path denied: {0}")]
    PathDenied(String),

    /// The tool requires explicit user confirmation before proceeding.
    /// This is typically returned for Medium-risk tools when the gating
    /// policy requires it.
    #[error("Requires confirmation")]
    RequiresConfirmation,
}

impl ToolError {
    /// Check if the error is retriable (transient execution failures).
    pub fn is_retriable(&self) -> bool {
        matches!(self, ToolError::ExecutionFailed(_))
    }

    /// Get a machine-readable error code for API responses.
    pub fn error_code(&self) -> &'static str {
        match self {
            ToolError::InvalidInput(_) => "TOOL_INVALID_INPUT",
            ToolError::ExecutionFailed(_) => "TOOL_EXECUTION_FAILED",
            ToolError::NotFound(_) => "TOOL_NOT_FOUND",
            ToolError::PathDenied(_) => "TOOL_PATH_DENIED",
            ToolError::RequiresConfirmation => "TOOL_REQUIRES_CONFIRMATION",
        }
    }

    /// Get the HTTP status code mapping for this error type.
    pub fn http_status(&self) -> u16 {
        match self {
            ToolError::InvalidInput(_) => 400,
            ToolError::ExecutionFailed(_) => 500,
            ToolError::NotFound(_) => 404,
            ToolError::PathDenied(_) => 403,
            ToolError::RequiresConfirmation => 403,
        }
    }
}

