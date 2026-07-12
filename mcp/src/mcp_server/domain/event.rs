//! Domain events for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#events
//! Implements: Contract Freeze — McpServerEvent payload schemas
//!
//! These events are emitted throughout the MCP Server lifecycle whenever
//! significant state transitions occur. Consumers (observability, telemetry,
//! audit trail) subscribe to these event types.
//!
//! # Event Catalog
//!
//! | Event | Trigger | Published By |
//! |-------|---------|-------------|
//! | McpSessionStarted | SessionManager after successful initialize handshake | SessionManager |
//! | McpSessionEnded | SessionManager on transport close or error | SessionManager |
//! | McpToolsListed | Client requested tools/list | ToolRegistry |
//! | ToolCallReceived | RequestRouter before routing to handler | RequestRouter |
//! | ToolCallCompleted | Handler returned result successfully | RequestRouter |
//! | ToolCallFailed | Handler returned error | RequestRouter |
//! | ToolRegistered | New tool registered | ToolRegistry |
//! | TransportError | Transport encountered an error | Transport |
//!
//! # Contract (Frozen)
//!
//! - Every event carries session_id (or aggregate_id) and timestamp for correlation
//! - Serialized as tagged union with `#[serde(tag = "type")]`
//! - Events are facts — immutable and append-only
//! - No behavior — pure data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::value::SessionId;

/// All domain events emitted by the MCP Server bounded context.
///
/// Each variant represents a meaningful domain occurrence.
/// Consumers use these events for observability, logging, and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpServerEvent {
    /// A new MCP client session has been initialized successfully.
    McpSessionStarted {
        /// Session identifier.
        session_id: SessionId,
        /// Client name (e.g., "claude-code", "cursor").
        client_name: String,
        /// Client version (if provided).
        client_version: Option<String>,
        /// Supported protocol version.
        protocol_version: String,
        /// Timestamp of session start.
        timestamp: DateTime<Utc>,
    },

    /// An MCP client session has ended.
    McpSessionEnded {
        /// Session identifier.
        session_id: SessionId,
        /// Reason for session end (e.g., "client_disconnect", "timeout", "error").
        reason: String,
        /// Session duration in milliseconds.
        duration_ms: u64,
        /// Timestamp of session end.
        timestamp: DateTime<Utc>,
    },

    /// A client requested the list of available tools.
    McpToolsListed {
        /// Session identifier.
        session_id: SessionId,
        /// Number of tools returned.
        tool_count: usize,
        /// Whether enterprise tools are available.
        has_enterprise_tools: bool,
        /// Timestamp of the request.
        timestamp: DateTime<Utc>,
    },

    /// A tool call request has been received by the RequestRouter.
    ToolCallReceived {
        /// Session identifier.
        session_id: SessionId,
        /// Tool name being called.
        tool_name: String,
        /// Call identifier for correlation.
        call_id: Uuid,
        /// Size of parameters in bytes (for monitoring).
        params_size: usize,
        /// Timestamp of the request.
        timestamp: DateTime<Utc>,
    },

    /// A tool call completed successfully.
    ToolCallCompleted {
        /// Session identifier.
        session_id: SessionId,
        /// Tool name that was called.
        tool_name: String,
        /// Call identifier for correlation.
        call_id: Uuid,
        /// Execution duration in milliseconds.
        duration_ms: u64,
        /// Whether the result is an error (tool-level error).
        is_error: bool,
        /// Timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// A tool call failed with an error.
    ToolCallFailed {
        /// Session identifier.
        session_id: SessionId,
        /// Tool name that was called.
        tool_name: String,
        /// Call identifier for correlation.
        call_id: Uuid,
        /// Error code from the JSON-RPC error or system.
        error_code: i32,
        /// Error message.
        error_message: String,
        /// Timestamp of failure.
        timestamp: DateTime<Utc>,
    },

    /// A new tool was registered in the ToolRegistry.
    ToolRegistered {
        /// Tool name that was registered.
        tool_name: String,
        /// Whether this is an enterprise tool.
        is_enterprise: bool,
        /// Total number of registered tools after this registration.
        total_tools: usize,
        /// Timestamp of registration.
        timestamp: DateTime<Utc>,
    },

    /// A transport error occurred.
    TransportError {
        /// Transport mode ("stdio" or "sse").
        transport_mode: String,
        /// Error description.
        error: String,
        /// Timestamp of error.
        timestamp: DateTime<Utc>,
    },
}

impl McpServerEvent {
    /// Canonical snake_case name of this event variant.
    pub fn event_type(&self) -> &'static str {
        match self {
            McpServerEvent::McpSessionStarted { .. } => "mcp_session_started",
            McpServerEvent::McpSessionEnded { .. } => "mcp_session_ended",
            McpServerEvent::McpToolsListed { .. } => "mcp_tools_listed",
            McpServerEvent::ToolCallReceived { .. } => "tool_call_received",
            McpServerEvent::ToolCallCompleted { .. } => "tool_call_completed",
            McpServerEvent::ToolCallFailed { .. } => "tool_call_failed",
            McpServerEvent::ToolRegistered { .. } => "tool_registered",
            McpServerEvent::TransportError { .. } => "transport_error",
        }
    }

    /// Extract the session ID from the event, if present.
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            McpServerEvent::McpSessionStarted { session_id, .. } => Some(*session_id),
            McpServerEvent::McpSessionEnded { session_id, .. } => Some(*session_id),
            McpServerEvent::McpToolsListed { session_id, .. } => Some(*session_id),
            McpServerEvent::ToolCallReceived { session_id, .. } => Some(*session_id),
            McpServerEvent::ToolCallCompleted { session_id, .. } => Some(*session_id),
            McpServerEvent::ToolCallFailed { session_id, .. } => Some(*session_id),
            _ => None,
        }
    }

    /// Extract the timestamp from the event.
    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            McpServerEvent::McpSessionStarted { timestamp, .. } => timestamp,
            McpServerEvent::McpSessionEnded { timestamp, .. } => timestamp,
            McpServerEvent::McpToolsListed { timestamp, .. } => timestamp,
            McpServerEvent::ToolCallReceived { timestamp, .. } => timestamp,
            McpServerEvent::ToolCallCompleted { timestamp, .. } => timestamp,
            McpServerEvent::ToolCallFailed { timestamp, .. } => timestamp,
            McpServerEvent::ToolRegistered { timestamp, .. } => timestamp,
            McpServerEvent::TransportError { timestamp, .. } => timestamp,
        }
    }
}
