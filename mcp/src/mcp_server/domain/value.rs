//! Value objects for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#value-objects
//! Implements: Contract Freeze — JsonRpcMessage, ToolSchema, ResourceSchema,
//! PromptSchema, ServerCapabilities, ClientCapabilities, SessionId
//!
//! Value objects are immutable, interchangeable, and defined by their attributes,
//! not identity. They carry validation in their constructors and are serializable
//! for API transmission.
//!
//! # Contract (Frozen)
//!
//! - All value objects are immutable (no pub fields, no setters)
//! - All value objects implement PartialEq + Eq + Hash based on ALL fields
//! - Constructors validate invariants — return Result<_, Error> on failure
//! - All types derive Serialize + Deserialize for JSON-RPC transmission
//! - No behavior beyond field accessors and validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;


// ---------------------------------------------------------------------------
// SessionId — strongly-typed identifier for MCP sessions
// ---------------------------------------------------------------------------

/// Strongly-typed identifier for an MCP client session.
///
/// Wraps a UUID v4 for type safety and traceability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Create a new random session ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a session ID from an existing UUID.
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Return the inner UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// JsonRpcMessage — core JSON-RPC 2.0 message type
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 message — request, notification, response, or error.
///
/// This is the wire format for all MCP protocol communication.
/// Each variant is tagged by the presence/absence of `id` and `method`.
///
/// # Contract (Frozen)
///
/// - `jsonrpc` field is always "2.0"
/// - Requests have `id` (number or string) + `method` + optional `params`
/// - Notifications have `method` + `params` but NO `id`
/// - Success responses have `id` + `result`
/// - Error responses have `id` + `error`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcMessage {
    /// JSON-RPC protocol version (always "2.0").
    pub jsonrpc: String,

    /// Request identifier (present for requests and responses, absent for notifications).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,

    /// Method name (present for requests and notifications).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    /// Parameters (present for requests and notifications).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,

    /// Result payload (present for success responses).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error payload (present for error responses).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcMessage {
    /// Create a JSON-RPC request message.
    pub fn request(
        id: RequestId,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: Some(method.into()),
            params,
            result: None,
            error: None,
        }
    }

    /// Create a JSON-RPC notification (no id, no response expected).
    pub fn notification(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some(method.into()),
            params,
            result: None,
            error: None,
        }
    }

    /// Create a JSON-RPC success response.
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    /// Create a JSON-RPC error response.
    pub fn error(id: RequestId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: None,
            error: Some(error),
        }
    }

    /// Returns true if this is a request message (has id and method).
    pub fn is_request(&self) -> bool {
        self.id.is_some() && self.method.is_some()
    }

    /// Returns true if this is a notification (no id, has method).
    pub fn is_notification(&self) -> bool {
        self.id.is_none() && self.method.is_some()
    }

    /// Returns true if this is a response (has id, no method, result or error).
    pub fn is_response(&self) -> bool {
        self.id.is_some() && self.method.is_none()
    }

    /// Returns true if this is a success response.
    pub fn is_success(&self) -> bool {
        self.result.is_some()
    }

    /// Returns true if this is an error response.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

// ---------------------------------------------------------------------------
// RequestId — identifier for JSON-RPC requests
// ---------------------------------------------------------------------------

/// Identifier for a JSON-RPC request, which can be a number or string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// Numeric request ID.
    Number(u64),
    /// String request ID.
    String(String),
}

impl RequestId {
    /// Create a numeric request ID.
    pub fn number(n: u64) -> Self {
        Self::Number(n)
    }

    /// Create a string request ID.
    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }
}

// ---------------------------------------------------------------------------
// JsonRpcError — JSON-RPC error object
// ---------------------------------------------------------------------------

/// A JSON-RPC error object as specified in JSON-RPC 2.0.
///
/// # Error Code Ranges
///
/// | Range | Category | Examples |
/// |-------|----------|---------|
/// | -32768 to -32000 | Standard JSON-RPC errors | ParseError(-32700), InvalidRequest(-32600) |
/// | -32000 to -32099 | Server errors | Server error, InternalError |
/// | 0 to -32000 | Application errors | Tool errors, validation errors |
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
#[error("{message} (code: {code})")]
pub struct JsonRpcError {
    /// Error code (negative integer per JSON-RPC spec).
    pub code: i32,

    /// Short human-readable error message.
    pub message: String,

    /// Optional additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Standard JSON-RPC parse error (-32700).
    pub fn parse_error() -> Self {
        Self {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }
    }

    /// Standard JSON-RPC invalid request (-32600).
    pub fn invalid_request() -> Self {
        Self {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        }
    }

    /// Standard JSON-RPC method not found (-32601).
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }

    /// Standard JSON-RPC invalid params (-32602).
    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: format!("Invalid params: {}", detail.into()),
            data: None,
        }
    }

    /// Standard JSON-RPC internal error (-32603).
    pub fn internal_error(detail: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: format!("Internal error: {}", detail.into()),
            data: None,
        }
    }

    /// Application-level error for tool not found.
    pub fn tool_not_found(name: impl Into<String>) -> Self {
        Self {
            code: -32001,
            message: format!("Tool not found: {}", name.into()),
            data: None,
        }
    }

    /// Application-level error for tool execution failure.
    pub fn tool_execution_failed(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: -32002,
            message: format!("Tool execution failed: {} - {}", name.into(), detail.into()),
            data: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolSchema — MCP tool schema describing a callable tool
// ---------------------------------------------------------------------------

/// Schema describing an MCP tool — its name, description, and input parameters.
///
/// Tools are the primary mechanism for AI assistants to interact with Rigorix.
/// Each tool has a unique name (prefixed with `rigorix_` for OSS, `rigorix_enterprise_`
/// for enterprise), a description for LLM context, and a JSON Schema for parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Unique tool name (e.g., "rigorix_execute", "rigorix_list_templates").
    pub name: String,

    /// Human-readable description for LLM context and user-facing docs.
    pub description: String,

    /// JSON Schema for tool input parameters.
    pub input_schema: serde_json::Value,
}

impl ToolSchema {
    /// Create a new tool schema with the given name, description, and input schema.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }

    /// Returns true if this is an OSS tool name (prefixed with `rigorix_`).
    pub fn is_oss_tool(&self) -> bool {
        self.name.starts_with("rigorix_") && !self.is_enterprise_tool()
    }

    /// Returns true if this is an enterprise tool name (prefixed with `rigorix_enterprise_`).
    pub fn is_enterprise_tool(&self) -> bool {
        self.name.starts_with("rigorix_enterprise_")
    }
}

// ---------------------------------------------------------------------------
// ContentItem — tool call result content item
// ---------------------------------------------------------------------------

/// A content item returned by a tool call result.
///
/// MCP tool results can contain multiple content items of different types
/// (text, image, resource, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content (base64-encoded).
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type of the image (e.g., "image/png").
        mime_type: String,
    },
    /// Resource content (reference to a resource URI).
    Resource {
        /// The resource URI.
        uri: String,
        /// MIME type of the resource.
        mime_type: Option<String>,
        /// Text content of the resource (if available).
        text: Option<String>,
    },
}

impl ContentItem {
    /// Create a text content item.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ToolResult — result of a tool call
// ---------------------------------------------------------------------------

/// Result of a tool call execution.
///
/// Contains the content items produced by the tool and an optional error flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Content items produced by the tool execution.
    pub content: Vec<ContentItem>,

    /// Whether the result represents an error (true) or success (false).
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResult {
    /// Create a successful tool result with the given content.
    pub fn success(content: Vec<ContentItem>) -> Self {
        Self {
            content,
            is_error: false,
        }
    }

    /// Create an error tool result with the given error content.
    pub fn error(content: Vec<ContentItem>) -> Self {
        Self {
            content,
            is_error: true,
        }
    }

    /// Create a simple text success result.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentItem::text(text)],
            is_error: false,
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceSchema — schema for an exposed MCP resource
// ---------------------------------------------------------------------------

/// Schema describing an MCP resource — a URI-patterned data source.
///
/// Resources provide read-only access to Rigorix engine data via
/// `rigorix://` URIs (e.g., `rigorix://audit/{id}`, `rigorix://templates/{name}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSchema {
    /// URI pattern (e.g., "rigorix://audit/{id}").
    pub uri: String,

    /// Human-readable name.
    pub name: String,

    /// Description of the resource and its contents.
    pub description: String,

    /// MIME type of the resource content.
    pub mime_type: String,
}

impl ResourceSchema {
    /// Create a new resource schema.
    pub fn new(
        uri: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: description.into(),
            mime_type: mime_type.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceContent — content returned when reading a resource
// ---------------------------------------------------------------------------

/// Content returned from reading a resource URI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceContent {
    /// The resolved resource URI.
    pub uri: String,

    /// MIME type of the content.
    pub mime_type: String,

    /// Text content.
    pub text: String,
}

// ---------------------------------------------------------------------------
// PromptSchema — schema for an MCP prompt template
// ---------------------------------------------------------------------------

/// Schema describing an MCP prompt template.
///
/// Prompts provide pre-crafted templates that AI assistants can use
/// to guide their tool usage (e.g., "How to execute a Rigorix plan").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptSchema {
    /// Unique prompt name.
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Optional list of argument definitions for the prompt.
    #[serde(default)]
    pub arguments: Vec<PromptArgument>,
}

impl PromptSchema {
    /// Create a new prompt schema.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        arguments: Vec<PromptArgument>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            arguments,
        }
    }
}

// ---------------------------------------------------------------------------
// PromptArgument — argument definition for a prompt template
// ---------------------------------------------------------------------------

/// Argument that can be passed when requesting a prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,

    /// Description of the argument.
    pub description: String,

    /// Whether this argument is required.
    #[serde(default)]
    pub required: bool,
}

impl PromptArgument {
    /// Create a new prompt argument.
    pub fn new(name: impl Into<String>, description: impl Into<String>, required: bool) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            required,
        }
    }
}

// ---------------------------------------------------------------------------
// PromptContent — content returned by a prompt template
// ---------------------------------------------------------------------------

/// Content returned from requesting a prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptContent {
    /// Optional description of the prompt content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Message content (role + content parts).
    pub messages: Vec<PromptMessage>,
}

// ---------------------------------------------------------------------------
// PromptMessage — a single message in a prompt template
// ---------------------------------------------------------------------------

/// A single message within a prompt template response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptMessage {
    /// The role of the message author ("user", "assistant", "system").
    pub role: PromptRole,

    /// Content of the message.
    pub content: PromptMessageContent,
}

// ---------------------------------------------------------------------------
// PromptRole — role of a prompt message author
// ---------------------------------------------------------------------------

/// Role of a message author in a prompt conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptRole {
    /// System message (instructions/context).
    System,
    /// User message.
    User,
    /// Assistant message.
    Assistant,
}

impl std::fmt::Display for PromptRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptRole::System => write!(f, "system"),
            PromptRole::User => write!(f, "user"),
            PromptRole::Assistant => write!(f, "assistant"),
        }
    }
}

// ---------------------------------------------------------------------------
// PromptMessageContent — content of a prompt message
// ---------------------------------------------------------------------------

/// Content of a prompt message (text-only to start).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptMessageContent {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
}

impl PromptMessageContent {
    /// Create a text prompt message content.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ServerCapabilities — negotiated server capabilities
// ---------------------------------------------------------------------------

/// Server capabilities advertised to the client during initialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Supported MCP protocol versions (e.g., ["2025-03-26"]).
    pub protocol_versions: Vec<String>,

    /// Number of registered tools.
    pub tool_count: usize,

    /// Number of exposed resources.
    pub resource_count: usize,

    /// Number of available prompts.
    pub prompt_count: usize,

    /// Whether enterprise proxy is enabled.
    #[serde(default)]
    pub enterprise_enabled: bool,

    /// Optional server info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<String>,
}

impl ServerCapabilities {
    /// Create a new ServerCapabilities with the given values.
    pub fn new(
        protocol_versions: Vec<String>,
        tool_count: usize,
        resource_count: usize,
        prompt_count: usize,
        enterprise_enabled: bool,
    ) -> Self {
        Self {
            protocol_versions,
            tool_count,
            resource_count,
            prompt_count,
            enterprise_enabled,
            server_info: None,
        }
    }

    /// Create default server capabilities for the MCP Server.
    pub fn default_with_counts(
        tool_count: usize,
        resource_count: usize,
        prompt_count: usize,
    ) -> Self {
        Self {
            protocol_versions: vec!["2025-03-26".to_string()],
            tool_count,
            resource_count,
            prompt_count,
            enterprise_enabled: false,
            server_info: Some("Rigorix MCP Gateway".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// ClientCapabilities — capabilities advertised by the client
// ---------------------------------------------------------------------------

/// Client capabilities received during MCP session initialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientCapabilities {
    /// Supported protocol version.
    pub protocol_version: String,

    /// Client name (e.g., "claude-code", "cursor").
    pub client_name: Option<String>,

    /// Client version.
    pub client_version: Option<String>,

    /// Whether the client supports progress notifications.
    #[serde(default)]
    pub supports_progress: bool,
}

// ---------------------------------------------------------------------------
// ClientInfo — identifying information about the MCP client
// ---------------------------------------------------------------------------

/// Identifying information about the connecting MCP client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client name (e.g., "claude-code", "cursor").
    pub name: String,

    /// Client version string.
    pub version: Option<String>,
}

// ---------------------------------------------------------------------------
// ServerConfig — configuration for the MCP Server
// ---------------------------------------------------------------------------

/// Configuration for the MCP Server.
///
/// Controls transport mode, session limits, and server behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Transport mode: "stdio" or "sse".
    pub transport_mode: TransportMode,

    /// Maximum number of concurrent sessions (SSE-only).
    pub max_sessions: usize,

    /// Session idle timeout in seconds.
    pub session_timeout_secs: u64,

    /// SSE bind address (e.g., "127.0.0.1:3001").
    pub bind_address: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport_mode: TransportMode::Stdio,
            max_sessions: 10,
            session_timeout_secs: 300,
            bind_address: None,
        }
    }
}

// ---------------------------------------------------------------------------
// TransportMode — MCP transport mode
// ---------------------------------------------------------------------------

/// The transport mode for MCP communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportMode {
    /// Standard I/O transport (stdin/stdout).
    Stdio,
    /// Server-Sent Events transport (HTTP).
    Sse,
}

impl std::fmt::Display for TransportMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportMode::Stdio => write!(f, "stdio"),
            TransportMode::Sse => write!(f, "sse"),
        }
    }
}

// ---------------------------------------------------------------------------
// SessionStatus — lifecycle status of an MCP session
// ---------------------------------------------------------------------------

/// Lifecycle status of an MCP client session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session created, waiting for initialize.
    Pending,
    /// Session initialized and active.
    Active,
    /// Session is being drained (no new requests).
    Draining,
    /// Session closed/ended.
    Ended,
}

impl SessionStatus {
    /// Returns true if the session is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionStatus::Ended)
    }

    /// Returns true if the session is currently active.
    pub fn is_active(&self) -> bool {
        matches!(self, SessionStatus::Active)
    }
}

// ---------------------------------------------------------------------------
// SessionMetadata — metadata associated with an MCP session
// ---------------------------------------------------------------------------

/// Metadata associated with an MCP client session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Arbitrary key-value metadata.
    pub labels: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// ToolHandler — handler trait for MCP tool execution
// ---------------------------------------------------------------------------

/// Handler trait for executing a registered MCP tool.
///
/// Each registered tool must provide a handler that implements this trait.
/// The handler receives the deserialized tool parameters and returns a
/// `ToolResult` or a `JsonRpcError` on failure.
///
/// # Contract (Frozen)
///
/// - Handlers are thread-safe (Send + Sync)
/// - Handlers must not panic — all failures returned as `JsonRpcError`
/// - Input validation happens in the handler
/// - Side effects must be reported in ToolResult
pub trait ToolHandler: Send + Sync {
    /// Execute the tool with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - JSON value containing tool-specific parameters.
    ///   Each handler validates and deserializes its expected parameters.
    ///
    /// # Returns
    ///
    /// * `Ok(ToolResult)` on success with content and optional is_error flag
    /// * `Err(JsonRpcError)` on failure with structured error information
    fn handle(&self, params: serde_json::Value) -> Result<ToolResult, JsonRpcError>;

    /// Return the tool schema describing this handler's tool.
    fn schema(&self) -> &ToolSchema;
}

// ---------------------------------------------------------------------------
// RegisteredTool — a tool registered in the ToolRegistry
// ---------------------------------------------------------------------------

/// A registered tool with its schema and handler.
#[derive(Clone)]
pub struct RegisteredTool {
    /// The tool's unique name.
    pub name: String,

    /// The tool's JSON schema for parameter validation.
    pub schema: ToolSchema,

    /// The handler that executes this tool.
    pub handler: std::sync::Arc<dyn ToolHandler>,

    /// Whether this is an enterprise tool.
    pub is_enterprise: bool,
}
