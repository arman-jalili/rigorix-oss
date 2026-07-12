//! Error types for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#errors
//! Implements: Contract Freeze — EngineFacadeError, HandlerError
//!
//! Structured error types for execution tools operations. Each variant carries
//! sufficient context for programmatic error handling (retry, fallback,
//! client-facing error messages).
//!
//! # Contract (Frozen)
//!
//! - All public variants and their fields are frozen
//! - New variants require ADR approval and interface review
//! - All errors derive Serialize + Deserialize for API transmission
//! - Every error has a user-readable Display message

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::value::PlanTemplateError;

// ---------------------------------------------------------------------------
// EngineFacadeError — root error type for EngineFacade operations
// ---------------------------------------------------------------------------

/// Root error type for all EngineFacade operations.
///
/// Wraps errors from the rigorix-engine, enforcement checks,
/// timeouts, and plan validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum EngineFacadeError {
    /// The engine encountered an internal error.
    #[error("Engine error: {0}")]
    EngineError(String),

    /// The requested plan could not be validated.
    #[error("Plan validation failed: {0}")]
    PlanValidationFailed(String),

    /// Budget exceeded — execution blocked by enforcement.
    #[error(
        "Budget exceeded: {tool_calls_remaining} calls remaining, {tokens_remaining} tokens remaining"
    )]
    BudgetExceeded {
        /// Remaining tool calls in budget.
        tool_calls_remaining: u64,
        /// Remaining tokens in budget.
        tokens_remaining: u64,
    },

    /// Enforcement policy blocked the operation.
    #[error("Enforcement blocked: {0}")]
    EnforcementBlocked(String),

    /// The operation timed out.
    #[error("Operation timed out after {duration_secs}s: {operation}")]
    Timeout {
        /// Name of the operation that timed out.
        operation: String,
        /// Timeout duration in seconds.
        duration_secs: u64,
    },

    /// Invalid plan template.
    #[error("Invalid plan: {0}")]
    InvalidPlan(#[from] PlanTemplateError),

    /// Execution not found for the given ID.
    #[error("Execution not found: {0}")]
    ExecutionNotFound(Uuid),

    /// The engine is not available (not started or disconnected).
    #[error("Engine not available: {0}")]
    EngineNotAvailable(String),

    /// Internal synchronization or state error.
    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// HandlerError — error type for MCP tool handlers
// ---------------------------------------------------------------------------

/// Error type returned by execution tool handlers.
///
/// Wraps EngineFacade errors and adds handler-specific errors
/// like input validation failures and timeouts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum HandlerError {
    /// The input arguments did not match the expected schema.
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// The engine returned an error.
    #[error("Engine error: {0}")]
    EngineError(#[from] EngineFacadeError),

    /// Plan validation returned blocking errors.
    #[error("Plan validation errors: {0}")]
    ValidationErrors(String),

    /// The handler timed out (engines's timeout or self-imposed).
    #[error("Handler timed out")]
    Timeout,

    /// An internal error occurred in the handler.
    #[error("Internal handler error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// ToolCallResult — standard MCP tool call result
// ---------------------------------------------------------------------------

/// Standard MCP tool call result for execution tool handlers.
///
/// Mirrors the MCP ToolResult schema for consistent responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Content items to return to the client.
    pub content: Vec<ToolContentItem>,

    /// Whether the result represents an error.
    #[serde(default)]
    pub is_error: bool,
}

/// A single content item in a tool call result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolContentItem {
    /// Content type (e.g., "text", "json").
    pub r#type: String,

    /// Text or JSON content.
    pub text: String,
}
