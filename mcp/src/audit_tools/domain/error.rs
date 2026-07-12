//! Error types for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#errors
//! Implements: Contract Freeze — AuditError, AuditHandlerError
//!
//! Structured error types for audit operations. Each variant carries sufficient
//! context for programmatic error handling (retry, fallback, client-facing messages).
//!
//! # Contract (Frozen)
//!
//! - All public variants and their fields are frozen
//! - New variants require ADR approval and interface review
//! - All errors derive Serialize + Deserialize for API transmission
//! - Every error has a user-readable Display message

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// AuditError — root error type for AuditQueryService operations
// ---------------------------------------------------------------------------

/// Root error type for all AuditQueryService operations.
///
/// Wraps errors from rigorix-engine, filter validation, and not-found conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum AuditError {
    /// The requested execution was not found.
    #[error("Execution not found: {0}")]
    NotFound(Uuid),

    /// The audit query filter parameters are invalid.
    #[error("Invalid audit filter: {0}")]
    InvalidFilter(String),

    /// The engine encountered an internal error.
    #[error("Engine error: {0}")]
    EngineError(String),

    /// The engine is not available (not started or disconnected).
    #[error("Engine not available: {0}")]
    EngineNotAvailable(String),

    /// The operation timed out.
    #[error("Operation timed out after {duration_secs}s: {operation}")]
    Timeout {
        /// Name of the operation that timed out.
        operation: String,
        /// Timeout duration in seconds.
        duration_secs: u64,
    },

    /// Internal state or configuration error.
    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// AuditHandlerError — error type for MCP audit tool handlers
// ---------------------------------------------------------------------------

/// Error type returned by audit tool handlers (ReadAuditHandler, ListAuditsHandler,
/// AuditSummaryHandler).
///
/// Wraps AuditError and adds handler-specific errors like input validation failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum AuditHandlerError {
    /// The input arguments did not match the expected schema.
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// The audit query service returned an error.
    #[error("Audit query error: {0}")]
    AuditError(#[from] AuditError),

    /// The handler timed out.
    #[error("Handler timed out")]
    Timeout,

    /// An internal error occurred in the handler.
    #[error("Internal handler error: {0}")]
    Internal(String),
}
