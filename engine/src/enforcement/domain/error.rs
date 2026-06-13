//! Enforcement error types.
//!
//! @canonical .pi/architecture/modules/enforcement.md#errors
//! Implements: Contract Freeze — EnforcementError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `EnforcementError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during enforcement operations.
#[derive(Debug, Error)]
pub enum EnforcementError {
    /// A tool call was blocked by enforcement policy.
    #[error("Tool call blocked by enforcement policy: {tool}. Reason: {reason}")]
    ToolBlocked {
        /// The name of the tool that was blocked.
        tool: String,
        /// Human-readable reason why the tool was blocked.
        reason: String,
    },

    /// A resource budget hard limit was exceeded.
    #[error("Resource budget exceeded: {resource} used {used}, limit {limit}")]
    BudgetExceeded {
        /// The resource that exceeded its budget.
        resource: String,
        /// Current usage value.
        used: u64,
        /// The hard limit that was exceeded.
        limit: u64,
    },

    /// An execution limit was reached (e.g., max tool calls, max time).
    #[error("Execution limit reached: {limit_type} ({current}/{max})")]
    ExecutionLimitReached {
        /// Type of limit that was reached (e.g., "max_tool_calls", "max_tokens").
        limit_type: String,
        /// Current value of the limited resource.
        current: u64,
        /// The maximum allowed value.
        max: u64,
    },

    /// A tool policy was not found for the requested tool.
    #[error("No policy found for tool: {tool}")]
    PolicyNotFound {
        /// The name of the tool with no policy.
        tool: String,
    },

    /// A budget was not found for the requested resource.
    #[error("Budget not found for resource: {resource}")]
    BudgetNotFound {
        /// The name of the resource with no budget.
        resource: String,
    },

    /// Invalid configuration for enforcement.
    #[error("Invalid enforcement configuration: {detail}")]
    InvalidConfiguration {
        /// Details about the configuration error.
        detail: String,
    },

    /// The enforcer is in an invalid state for the requested operation.
    #[error("Invalid enforcer state: {detail}")]
    InvalidState {
        /// Details about the state error.
        detail: String,
    },
}
