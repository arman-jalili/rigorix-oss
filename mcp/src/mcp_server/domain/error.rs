//! Error types for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#errors
//! Implements: Contract Freeze — McpServerError, RegistrationError, SessionError
//!
//! Structured error types for MCP Server operations. Each variant carries
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
// McpServerError — root error type for MCP Server operations
// ---------------------------------------------------------------------------

/// Root error type for all MCP Server operations.
///
/// Aggregates sub-errors from session management, tool registry,
/// transport, and routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum McpServerError {
    /// Session-related error.
    #[error("Session error: {0}")]
    Session(#[from] SessionError),

    /// Registration-related error.
    #[error("Registration error: {0}")]
    Registration(#[from] RegistrationError),

    /// Transport error — communication channel failure.
    #[error("Transport error: {0}")]
    Transport(String),

    /// Server is not initialized.
    #[error("Server not initialized")]
    NotInitialized,

    /// Server is already running.
    #[error("Server already running")]
    AlreadyRunning,

    /// Server is shutting down.
    #[error("Server is shutting down")]
    ShuttingDown,

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Request time-out.
    #[error("Request timed out: {0}")]
    Timeout(String),

    /// Internal error (unexpected condition).
    #[error("Internal error: {0}")]
    Internal(String),
}

impl McpServerError {
    /// Returns true if the error is retriable (transient failures).
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            McpServerError::Timeout(_) | McpServerError::Transport(_) | McpServerError::Internal(_)
        )
    }

    /// Get a machine-readable error code for API responses.
    pub fn error_code(&self) -> &'static str {
        match self {
            McpServerError::Session(_) => "MCP_SESSION_ERROR",
            McpServerError::Registration(_) => "MCP_REGISTRATION_ERROR",
            McpServerError::Transport(_) => "MCP_TRANSPORT_ERROR",
            McpServerError::NotInitialized => "MCP_NOT_INITIALIZED",
            McpServerError::AlreadyRunning => "MCP_ALREADY_RUNNING",
            McpServerError::ShuttingDown => "MCP_SHUTTING_DOWN",
            McpServerError::InvalidConfig(_) => "MCP_INVALID_CONFIG",
            McpServerError::Timeout(_) => "MCP_TIMEOUT",
            McpServerError::Internal(_) => "MCP_INTERNAL_ERROR",
        }
    }

    /// Get the HTTP status code mapping for this error type.
    pub fn http_status(&self) -> u16 {
        match self {
            McpServerError::Session(_) => 400,
            McpServerError::Registration(_) => 409,
            McpServerError::Transport(_) => 500,
            McpServerError::NotInitialized => 503,
            McpServerError::AlreadyRunning => 409,
            McpServerError::ShuttingDown => 503,
            McpServerError::InvalidConfig(_) => 500,
            McpServerError::Timeout(_) => 504,
            McpServerError::Internal(_) => 500,
        }
    }
}

// ---------------------------------------------------------------------------
// SessionError — session management errors
// ---------------------------------------------------------------------------

/// Errors related to MCP session lifecycle management.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum SessionError {
    /// Session not found by the given session ID.
    #[error("Session not found: {0}")]
    NotFound(String),

    /// Session is in a state that doesn't allow the requested operation.
    #[error("Invalid session state: {0}")]
    InvalidState(String),

    /// Maximum number of sessions reached.
    #[error("Maximum sessions reached ({0})")]
    MaxSessionsReached(usize),

    /// Session time-out.
    #[error("Session timed out: {0}")]
    Timeout(String),

    /// Session initialization failed.
    #[error("Session initialization failed: {0}")]
    InitializationFailed(String),

    /// Session closed by transport error.
    #[error("Session transport error: {0}")]
    TransportError(String),
}

impl SessionError {
    /// Returns true if the error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            SessionError::Timeout(_) | SessionError::TransportError(_)
        )
    }

    /// Get a machine-readable error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            SessionError::NotFound(_) => "SESSION_NOT_FOUND",
            SessionError::InvalidState(_) => "SESSION_INVALID_STATE",
            SessionError::MaxSessionsReached(_) => "SESSION_MAX_SESSIONS",
            SessionError::Timeout(_) => "SESSION_TIMEOUT",
            SessionError::InitializationFailed(_) => "SESSION_INIT_FAILED",
            SessionError::TransportError(_) => "SESSION_TRANSPORT_ERROR",
        }
    }
}

// ---------------------------------------------------------------------------
// RegistrationError — tool registration errors
// ---------------------------------------------------------------------------

/// Errors related to tool registration in the ToolRegistry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum RegistrationError {
    /// A tool with the same name is already registered.
    #[error("Tool already registered: {0}")]
    AlreadyRegistered(String),

    /// The tool name doesn't match naming conventions.
    #[error("Invalid tool name: {0}. Tools must start with 'rigorix_' prefix")]
    InvalidName(String),

    /// The tool was not found for unregistration or lookup.
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Enterprise tools cannot be registered through the standard path.
    #[error("Enterprise tools must be registered via register_enterprise_tools")]
    EnterpriseRegistrationForbidden,

    /// Maximum number of tools reached.
    #[error("Maximum tool count reached ({0})")]
    MaxToolsReached(usize),
}

impl RegistrationError {
    /// Get a machine-readable error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            RegistrationError::AlreadyRegistered(_) => "TOOL_ALREADY_REGISTERED",
            RegistrationError::InvalidName(_) => "TOOL_INVALID_NAME",
            RegistrationError::NotFound(_) => "TOOL_NOT_FOUND",
            RegistrationError::EnterpriseRegistrationForbidden => {
                "ENTERPRISE_REGISTRATION_FORBIDDEN"
            }
            RegistrationError::MaxToolsReached(_) => "MAX_TOOLS_REACHED",
        }
    }
}
