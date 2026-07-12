//! Data Transfer Objects for the MCP Server module.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#dto
//! Implements: Contract Freeze — all input/output DTO schemas
//!
//! DTOs define the input/output contracts for all service operations.
//! They carry documentation and validation metadata but no behavior.
//!
//! # Contract (Frozen)
//!
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::mcp_server::domain::value::{
    ClientCapabilities, ClientInfo, ContentItem, PromptSchema, ResourceSchema, ServerCapabilities,
    ServerConfig, SessionId, ToolSchema,
};

// ---------------------------------------------------------------------------
// Initialize DTOs
// ---------------------------------------------------------------------------

/// Input for the initialize handshake.
///
/// Sent by the MCP client to negotiate protocol capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeInput {
    /// Protocol version supported by the client.
    pub protocol_version: String,

    /// Client identifying information.
    pub client_info: ClientInfo,

    /// Client capabilities.
    pub capabilities: ClientCapabilities,
}

/// Output from the initialize handshake.
///
/// Contains the server capabilities and protocol version to use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitializeOutput {
    /// Negotiated protocol version.
    pub protocol_version: String,

    /// Server capabilities.
    pub server_capabilities: ServerCapabilities,

    /// Server info string.
    pub server_info: String,
}

// ---------------------------------------------------------------------------
// List Tools DTOs
// ---------------------------------------------------------------------------

/// Input for listing available tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsInput {
    /// Optional filter to only return OSS or enterprise tools.
    pub filter: Option<ToolFilter>,
}

/// Filter for tools/list requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolFilter {
    /// Only OSS tools (rigorix_ prefix).
    Oss,
    /// Only enterprise tools (rigorix_enterprise_ prefix).
    Enterprise,
    /// All tools (default).
    All,
}

/// Output from listing available tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListToolsOutput {
    /// Tool schemas.
    pub tools: Vec<ToolSchema>,
}

// ---------------------------------------------------------------------------
// Call Tool DTOs
// ---------------------------------------------------------------------------

/// Input for calling/executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolInput {
    /// Session ID making the call.
    pub session_id: SessionId,

    /// Tool name to execute.
    pub name: String,

    /// Tool-specific parameters as a JSON object.
    pub arguments: serde_json::Value,
}

/// Output from calling a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallToolOutput {
    /// Content items produced by the tool.
    pub content: Vec<ContentItem>,

    /// Whether the result represents an error.
    #[serde(default)]
    pub is_error: bool,
}

// ---------------------------------------------------------------------------
// List Resources DTOs
// ---------------------------------------------------------------------------

/// Output from listing available resources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListResourcesOutput {
    /// Resource schemas.
    pub resources: Vec<ResourceSchema>,
}

// ---------------------------------------------------------------------------
// Read Resource DTOs
// ---------------------------------------------------------------------------

/// Input for reading a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResourceInput {
    /// Resource URI to read (e.g., "rigorix://audit/{id}").
    pub uri: String,
}

/// Output from reading a resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadResourceOutput {
    /// The resolved URI.
    pub uri: String,

    /// MIME type of the content.
    pub mime_type: String,

    /// Text content.
    pub text: String,
}

// ---------------------------------------------------------------------------
// List Prompts DTOs
// ---------------------------------------------------------------------------

/// Output from listing available prompts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListPromptsOutput {
    /// Prompt schemas.
    pub prompts: Vec<PromptSchema>,
}

// ---------------------------------------------------------------------------
// Get Prompt DTOs
// ---------------------------------------------------------------------------

/// Input for getting a prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptInput {
    /// Prompt name.
    pub name: String,

    /// Optional arguments for the prompt.
    pub arguments: Option<HashMap<String, String>>,
}

/// Output from getting a prompt template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetPromptOutput {
    /// Optional description.
    pub description: Option<String>,

    /// Messages in the prompt template.
    pub messages: Vec<PromptMessageDto>,
}

/// A message within a prompt template (DTO version).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptMessageDto {
    /// Role of the message author.
    pub role: String,

    /// Content of the message.
    pub content: PromptContentDto,
}

/// Content of a prompt message (DTO version).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptContentDto {
    /// Text content.
    Text { text: String },
}

// ---------------------------------------------------------------------------
// Register Tool DTOs (for server-side tool registration)
// ---------------------------------------------------------------------------

/// Input for registering a tool in the ToolRegistry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterToolInput {
    /// Tool schema describing the tool.
    pub schema: ToolSchema,

    /// Whether this is an enterprise tool.
    #[serde(default)]
    pub is_enterprise: bool,
}

/// Output from registering a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterToolOutput {
    /// The tool name that was registered.
    pub name: String,

    /// Whether this was a replacement (always false for contract freeze).
    pub registered: bool,

    /// Total number of registered tools.
    pub total_tools: usize,
}

// ---------------------------------------------------------------------------
// Unregister Tool DTOs
// ---------------------------------------------------------------------------

/// Input for unregistering a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnregisterToolInput {
    /// Tool name to unregister.
    pub name: String,
}

/// Output from unregistering a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnregisterToolOutput {
    /// The tool name that was unregistered.
    pub name: String,

    /// Total number of remaining registered tools.
    pub total_tools: usize,
}

// ---------------------------------------------------------------------------
// Session DTOs
// ---------------------------------------------------------------------------

/// Input for ending a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndSessionInput {
    /// Session ID to end.
    pub session_id: SessionId,

    /// Reason for ending the session.
    pub reason: String,
}

/// Information about an active session (for listing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session ID.
    pub session_id: SessionId,

    /// Client name.
    pub client_name: String,

    /// Session status.
    pub status: String,

    /// When the session started.
    pub started_at: Option<DateTime<Utc>>,

    /// When the session was last active.
    pub last_active_at: Option<DateTime<Utc>>,
}

/// Output from listing active sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListSessionsOutput {
    /// Active session information.
    pub sessions: Vec<SessionInfo>,

    /// Total number of sessions.
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Server DTOs
// ---------------------------------------------------------------------------

/// Input for starting the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartServerInput {
    /// Server configuration.
    pub config: ServerConfig,
}

/// Output from starting the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartServerOutput {
    /// Whether the server was started successfully.
    pub started: bool,

    /// Server ID.
    pub server_id: Uuid,
}

/// Server status information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerStatusInfo {
    /// Server ID.
    pub id: Uuid,

    /// Current server status.
    pub status: String,

    /// Active transport mode.
    pub transport_mode: Option<String>,

    /// Number of active sessions.
    pub active_sessions: usize,

    /// Number of registered tools.
    pub registered_tools: usize,

    /// Whether enterprise tools are available.
    pub has_enterprise_tools: bool,

    /// When the server was started.
    pub started_at: Option<DateTime<Utc>>,
}
