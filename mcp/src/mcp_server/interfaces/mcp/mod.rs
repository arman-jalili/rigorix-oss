//! MCP protocol handler contracts — framework-agnostic interface definitions.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#mcp-handlers
//! Implements: Contract Freeze — MCP protocol message handler contracts
//!
//! Defines the contract for each MCP protocol message handler.
//! These handlers are called by the RequestRouter when an incoming
//! JSON-RPC message matches a method name.
//!
//! # MCP Protocol Methods
//!
//! | Method | Handler | Description |
//! |--------|---------|-------------|
//! | initialize | InitializeHandler | Session capability negotiation |
//! | initialized | InitializedHandler | Marks session as initialized |
//! | tools/list | ListToolsHandler | List available tools |
//! | tools/call | CallToolHandler | Execute a tool |
//! | resources/list | ListResourcesHandler | List available resources |
//! | resources/read | ReadResourceHandler | Read a resource by URI |
//! | prompts/list | ListPromptsHandler | List available prompts |
//! | prompts/get | GetPromptHandler | Get a prompt template |
//! | notifications/cancelled | CancelledHandler | Handle cancellation |
//!
pub mod handlers;

pub use handlers::*;

use async_trait::async_trait;

use crate::mcp_server::domain::value::{JsonRpcError, JsonRpcMessage};

// ---------------------------------------------------------------------------
// McpMethodHandler — Core handler trait for all MCP methods
// ---------------------------------------------------------------------------

/// Handler for a single MCP protocol method.
///
/// Each MCP method has a dedicated handler implementation that knows
/// how to process the method's parameters and produce a response.
///
/// # Contract (Frozen)
///
/// - Handlers are stateless (no mutable state in the handler itself)
/// - Handlers must not panic — all failures returned as Err
/// - Input validation happens inside the handler
/// - Handlers are async and thread-safe
#[async_trait]
pub trait McpMethodHandler: Send + Sync {
    /// The MCP method name this handler handles (e.g., "initialize", "tools/list").
    fn method_name(&self) -> &'static str;

    /// Handle an incoming MCP request.
    ///
    /// # Arguments
    ///
    /// * `message` - The full incoming JSON-RPC message (contains id, method, params)
    /// * `session_id` - The session ID for the active connection
    ///
    /// # Returns
    ///
    /// * `Ok(JsonRpcMessage)` - A JSON-RPC response (success or error)
    ///   Success responses contain `result: <response data>`
    ///   Error responses contain `error: JsonRpcError`
    async fn handle(
        &self,
        message: &JsonRpcMessage,
        session_id: &str,
    ) -> Result<JsonRpcMessage, McpHandlerError>;
}

// ---------------------------------------------------------------------------
// McpHandlerError — Error type for MCP handler operations
// ---------------------------------------------------------------------------

/// Error type for MCP handler operations.
///
/// Wraps a JsonRpcError that should be returned to the client.
/// This allows handlers to return structured errors that the router
/// can forward directly as JSON-RPC error responses.
#[derive(Debug, Clone, thiserror::Error)]
pub enum McpHandlerError {
    /// A structured JSON-RPC error to return to the client.
    #[error("MCP handler error: {0}")]
    JsonRpc(#[from] JsonRpcError),

    /// An internal error (converted to a generic JSON-RPC internal error).
    #[error("Internal handler error: {0}")]
    Internal(String),
}

impl McpHandlerError {
    /// Convert this error into a JSON-RPC error response message.
    pub fn into_json_rpc_error(self, _id: Option<uuid::Uuid>) -> JsonRpcError {
        match self {
            McpHandlerError::JsonRpc(err) => err,
            McpHandlerError::Internal(msg) => JsonRpcError::internal_error(msg),
        }
    }
}

// ---------------------------------------------------------------------------
// RequestRouter — Routes incoming messages to the correct handler
// ---------------------------------------------------------------------------

/// Routes incoming JSON-RPC messages to the appropriate handler based
/// on the method name.
///
/// The router maintains a registry of handlers mapped by method name.
/// When a message arrives, it looks up the handler by method and dispatches.
///
/// # Contract (Frozen)
///
/// - Method dispatch is O(1) via HashMap lookup
/// - Unknown methods return `method_not_found` JSON-RPC error
/// - Notifications (no id) are dispatched but no response is returned
/// - Handlers are injected at construction time
#[async_trait]
pub trait RequestRouter: Send + Sync {
    /// Register a handler for a specific MCP method.
    ///
    /// Returns an error if a handler is already registered for this method.
    fn register_handler(
        &mut self,
        handler: Box<dyn McpMethodHandler>,
    ) -> Result<(), McpHandlerError>;

    /// Route an incoming JSON-RPC message to the appropriate handler.
    ///
    /// # Arguments
    ///
    /// * `message` - The incoming JSON-RPC message
    /// * `session_id` - The session ID for the active connection
    ///
    /// # Returns
    ///
    /// * `Some(JsonRpcMessage)` - A JSON-RPC response for requests
    /// * `None` - For notifications (no response expected) or parse errors
    async fn route(
        &self,
        message: JsonRpcMessage,
        session_id: &str,
    ) -> Option<JsonRpcMessage>;

    /// Check if a handler is registered for the given method.
    fn has_handler(&self, method: &str) -> bool;
}

// ---------------------------------------------------------------------------
// Handler trait aliases for specific MCP methods
// ---------------------------------------------------------------------------

/// Handler for the MCP `initialize` method.
///
/// Processes the initialization handshake: validates protocol version,
/// negotiates capabilities, creates a session.
#[async_trait]
pub trait InitializeHandler: Send + Sync {
    /// Handle an initialize request.
    ///
    /// Input params: `{ protocolVersion, clientInfo, capabilities }`
    /// Output result: `{ protocolVersion, serverCapabilities, serverInfo }`
    async fn handle_initialize(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError>;
}

/// Handler for the MCP `initialized` notification.
///
/// Marks the session as fully initialized after the client sends
/// the `initialized` notification.
#[async_trait]
pub trait InitializedHandler: Send + Sync {
    /// Handle an initialized notification.
    ///
    /// No params expected. Marks the session as initialized.
    async fn handle_initialized(&self, session_id: &str) -> Result<(), JsonRpcError>;
}

/// Handler for the MCP `tools/list` method.
#[async_trait]
pub trait ListToolsHandler: Send + Sync {
    /// Handle a tools/list request.
    ///
    /// Output result: `{ tools: [ToolSchema] }`
    async fn handle_list_tools(&self) -> Result<serde_json::Value, JsonRpcError>;
}

/// Handler for the MCP `tools/call` method.
#[async_trait]
pub trait CallToolHandler: Send + Sync {
    /// Handle a tools/call request.
    ///
    /// Input params: `{ name, arguments }`
    /// Output result: `{ content: [ContentItem], isError: bool }`
    async fn handle_call_tool(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError>;
}

/// Handler for the MCP `resources/list` method.
#[async_trait]
pub trait ListResourcesHandler: Send + Sync {
    /// Handle a resources/list request.
    ///
    /// Output result: `{ resources: [ResourceSchema] }`
    async fn handle_list_resources(&self) -> Result<serde_json::Value, JsonRpcError>;
}

/// Handler for the MCP `resources/read` method.
#[async_trait]
pub trait ReadResourceHandler: Send + Sync {
    /// Handle a resources/read request.
    ///
    /// Input params: `{ uri }`
    /// Output result: `{ contents: [{ uri, mimeType, text }] }`
    async fn handle_read_resource(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError>;
}

/// Handler for the MCP `prompts/list` method.
#[async_trait]
pub trait ListPromptsHandler: Send + Sync {
    /// Handle a prompts/list request.
    ///
    /// Output result: `{ prompts: [PromptSchema] }`
    async fn handle_list_prompts(&self) -> Result<serde_json::Value, JsonRpcError>;
}

/// Handler for the MCP `prompts/get` method.
#[async_trait]
pub trait GetPromptHandler: Send + Sync {
    /// Handle a prompts/get request.
    ///
    /// Input params: `{ name, arguments? }`
    /// Output result: `{ description, messages: [PromptMessage] }`
    async fn handle_get_prompt(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError>;
}

/// Handler for the MCP `notifications/cancelled` notification.
#[async_trait]
pub trait CancelledHandler: Send + Sync {
    /// Handle a cancelled notification.
    ///
    /// Input params: `{ requestId }`
    async fn handle_cancelled(
        &self,
        params: serde_json::Value,
    ) -> Result<(), JsonRpcError>;
}

// ---------------------------------------------------------------------------
// TransportHandle — Handle returned by transport listeners
// ---------------------------------------------------------------------------

/// Handle returned by a transport listener, used for graceful shutdown.
///
/// The transport implementation returns this handle from its `listen` method.
/// The server can await/cancel this handle to stop the transport.
#[async_trait]
pub trait TransportHandle: Send + Sync {
    /// Wait for the transport to finish (blocks until transport stops).
    async fn wait_for_completion(&self);

    /// Signal the transport to stop.
    async fn signal_stop(&self);
}
