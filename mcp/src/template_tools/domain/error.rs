//! Error types for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#errors
//! Implements: Contract Freeze — TemplateError, HandlerError
//!
//! Structured error types for template tools operations. Each variant carries
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

// ---------------------------------------------------------------------------
// TemplateError — root error type for TemplateRepository operations
// ---------------------------------------------------------------------------

/// Root error type for all TemplateRepository and TemplateConverter operations.
///
/// Covers filesystem errors, validation failures, serialization issues,
/// and concurrency concerns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum TemplateError {
    /// The requested template was not found.
    #[error("Template not found: {0}")]
    NotFound(String),

    /// The template name is invalid (empty, too long, or has invalid chars).
    #[error("Invalid template name: {0}")]
    InvalidName(String),

    /// A template with the given name already exists.
    #[error("Template already exists: {0}")]
    AlreadyExists(String),

    /// Template validation failed.
    #[error("Template validation failed: {0}")]
    ValidationError(String),

    /// Failed to deserialize template from TOML or JSON.
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    /// Failed to serialize template to TOML or JSON.
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    /// Filesystem repository error.
    #[error("Repository error: {0}")]
    RepositoryError(String),

    /// Concurrent write conflict — another write is in progress.
    #[error("Concurrent write conflict: {0}")]
    ConcurrentWriteConflict(String),

    /// Internal error (unexpected state, should not happen).
    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// HandlerError — error type for MCP tool handlers
// ---------------------------------------------------------------------------

/// Error type returned by template tool handlers.
///
/// Wraps TemplateError and adds handler-specific errors
/// like input validation failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum HandlerError {
    /// The input arguments did not match the expected schema.
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// The template repository returned an error.
    #[error("Template error: {0}")]
    TemplateError(#[from] TemplateError),

    /// The operation timed out.
    #[error("Operation timed out after {duration_secs}s: {operation}")]
    Timeout {
        /// Name of the operation that timed out.
        operation: String,
        /// Timeout duration in seconds.
        duration_secs: u64,
    },

    /// Internal synchronization or state error.
    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// ToolCallResult — MCP tool call result type
// ---------------------------------------------------------------------------

/// Result of an MCP tool call, matching the MCP protocol specification.
///
/// Used by all template tool handlers to format responses.
///
/// # Contract (Frozen)
///
/// - `success` creates a successful result with JSON content
/// - `error` creates an error result with a message
/// - `content` is a list of content items (text or resource)
/// - `is_error` indicates whether the call failed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Content items returned by the tool.
    #[serde(default)]
    pub content: Vec<ToolContentItem>,

    /// Whether the tool call resulted in an error.
    #[serde(default)]
    pub is_error: bool,
}

impl ToolCallResult {
    /// Create a successful tool call result with JSON content.
    pub fn success(value: serde_json::Value) -> Self {
        Self {
            content: vec![ToolContentItem {
                r#type: "text".into(),
                text: serde_json::to_string(&value).unwrap_or_default(),
            }],
            is_error: false,
        }
    }

    /// Create an error tool call result with a message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContentItem {
                r#type: "text".into(),
                text: message.into(),
            }],
            is_error: true,
        }
    }
}

/// A single content item in an MCP tool call result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolContentItem {
    /// Content type (e.g., "text", "resource").
    pub r#type: String,

    /// Content text.
    pub text: String,
}
