//! Error types for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#errors
//! Implements: Contract Freeze — ProxyError, HandlerError
//!
//! Structured error types for enterprise proxy operations. Each variant carries
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
// ProxyError — root error type for EnterpriseProxy operations
// ---------------------------------------------------------------------------

/// Root error type for all EnterpriseProxy, ProxyClient, and SchemaCache operations.
///
/// Covers configuration errors, transport failures, API errors, auth issues,
/// and deserialization problems.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum ProxyError {
    /// Configuration error (invalid URL, missing key, etc.).
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Network transport error (connection refused, DNS failure, etc.).
    #[error("Transport error: {0}")]
    Transport(String),

    /// API returned an error status code.
    #[error("API error (status {status}): {message}")]
    ApiError {
        /// HTTP status code.
        status: u16,
        /// Error response body.
        message: String,
    },

    /// Request timed out.
    #[error("Request timed out after {timeout_secs}s: {operation}")]
    Timeout {
        /// Name of the operation that timed out.
        operation: String,
        /// Timeout duration in seconds.
        timeout_secs: u64,
    },

    /// Authentication failure (invalid or expired API key).
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Failed to deserialize response from enterprise API.
    #[error("Deserialization failed: {0}")]
    Deserialization(String),

    /// Enterprise proxy is not enabled (no configuration).
    #[error("Enterprise proxy is not enabled")]
    NotEnabled,

    /// No tool schemas available (not initialized or fetch failed).
    #[error("No tool schemas available: {0}")]
    NoSchemas(String),

    /// Internal error (unexpected state, should not happen).
    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// HandlerError — error type for MCP tool handlers
// ---------------------------------------------------------------------------

/// Error type returned by enterprise tool handlers.
///
/// Wraps ProxyError and adds handler-specific errors
/// like input validation failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum HandlerError {
    /// The input arguments did not match the expected schema.
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// The proxy returned an error.
    #[error("Proxy error: {0}")]
    ProxyError(#[from] ProxyError),

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
/// Used by all enterprise tool handlers to format responses.
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
