//! Service interfaces (use cases) for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#services
//! Implements: Contract Freeze — McpServerService, ToolRegistryService, SessionService traits
//!
//! These traits define the application-level operations for the MCP Server.
//! All methods are async and return domain error types. Input/output types
//! are DTOs defined in `dto/`.
//!
//! # Contract (Frozen)
//!
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures
//! - Services are thread-safe (Send + Sync)

use async_trait::async_trait;
use std::sync::Arc;

use crate::mcp_server::domain::error::McpServerError;
use crate::mcp_server::domain::value::{ToolHandler, ToolSchema};

use super::dto::{
    CallToolInput, CallToolOutput, EndSessionInput, GetPromptInput, GetPromptOutput,
    InitializeInput, InitializeOutput, ListPromptsOutput, ListResourcesOutput, ListSessionsOutput,
    ListToolsInput, ListToolsOutput, ReadResourceInput, ReadResourceOutput, RegisterToolInput,
    RegisterToolOutput, ServerStatusInfo, StartServerInput, StartServerOutput, UnregisterToolInput,
    UnregisterToolOutput,
};

// ---------------------------------------------------------------------------
// McpServerService
// ---------------------------------------------------------------------------

/// Application service for the MCP Server lifecycle.
///
/// Manages server start/stop, client initialization, and MCP protocol
/// operations (initialize, tools/list, tools/call, resources/*, prompts/*).
///
/// # Contract (Frozen)
///
/// - Server must be started before accepting connections
/// - Initialize handshake must complete before tool/resource/prompt operations
/// - Graceful shutdown drains active requests
#[async_trait]
pub trait McpServerService: Send + Sync {
    /// Start the MCP Server with the given configuration.
    ///
    /// Opens the transport channel (stdio or SSE) and transitions
    /// the server to the Running state.
    ///
    /// # Errors
    /// - `McpServerError::AlreadyRunning` if server is already running
    /// - `McpServerError::InvalidConfig` if configuration is invalid
    async fn start(&self, input: StartServerInput) -> Result<StartServerOutput, McpServerError>;

    /// Shutdown the MCP Server gracefully.
    ///
    /// Drains active sessions, closes transport, and stops the server.
    async fn shutdown(&self) -> Result<(), McpServerError>;

    /// Handle the MCP initialize handshake.
    ///
    /// Creates a new session, negotiates capabilities, and returns
    /// the server capabilities for the client.
    ///
    /// # Errors
    /// - `McpServerError::NotInitialized` if server is not running
    /// - `McpServerError::Session` if session creation fails
    async fn initialize(
        &self,
        input: InitializeInput,
    ) -> Result<(InitializeOutput, Vec<McpServerEvent>), McpServerError>;

    /// List available tools.
    ///
    /// Returns all registered tool schemas matching the optional filter.
    async fn list_tools(
        &self,
        input: ListToolsInput,
    ) -> Result<ListToolsOutput, McpServerError>;

    /// Call/execute a tool.
    ///
    /// Routes the tool call to the appropriate handler and returns the result.
    ///
    /// # Errors
    /// - `McpServerError::Registration(RegistrationError::NotFound)` if tool not found
    /// - `McpServerError::Session(SessionError::NotFound)` if session not found
    async fn call_tool(&self, input: CallToolInput) -> Result<CallToolOutput, McpServerError>;

    /// List available resources.
    async fn list_resources(&self) -> Result<ListResourcesOutput, McpServerError>;

    /// Read a resource by URI.
    ///
    /// # Errors
    /// - Returns error if URI format is invalid or resource not found
    async fn read_resource(
        &self,
        input: ReadResourceInput,
    ) -> Result<ReadResourceOutput, McpServerError>;

    /// List available prompts.
    async fn list_prompts(&self) -> Result<ListPromptsOutput, McpServerError>;

    /// Get a prompt template by name.
    ///
    /// # Errors
    /// - Returns error if prompt name is not found
    async fn get_prompt(&self, input: GetPromptInput) -> Result<GetPromptOutput, McpServerError>;

    /// Get server status information.
    async fn status(&self) -> Result<ServerStatusInfo, McpServerError>;
}

// ---------------------------------------------------------------------------
// ToolRegistryService
// ---------------------------------------------------------------------------

/// Application service for the ToolRegistry.
///
/// Manages tool registration, unregistration, and lookup.
/// All tools must follow the `rigorix_` prefix convention.
///
/// # Contract (Frozen)
///
/// - Tools must be registered before they can be called
/// - OSS tools use `rigorix_` prefix, enterprise tools use `rigorix_enterprise_`
/// - Enterprise tools are registered via `register_enterprise_tools`
/// - Schemas are immutable after registration
#[async_trait]
pub trait ToolRegistryService: Send + Sync {
    /// Register a tool in the ToolRegistry.
    ///
    /// # Errors
    /// - `RegistrationError::InvalidName` if name doesn't follow prefix convention
    /// - `RegistrationError::AlreadyRegistered` if tool name already exists
    /// - `RegistrationError::EnterpriseRegistrationForbidden` if trying to register
    ///   enterprise tool through standard path
    async fn register_tool(
        &self,
        input: RegisterToolInput,
        handler: Arc<dyn ToolHandler>,
    ) -> Result<RegisterToolOutput, McpServerError>;

    /// Register enterprise tools.
    ///
    /// Enterprise tools must use `rigorix_enterprise_` prefix.
    async fn register_enterprise_tools(
        &self,
        schemas: Vec<ToolSchema>,
        handler: Arc<dyn ToolHandler>,
    ) -> Result<Vec<RegisterToolOutput>, McpServerError>;

    /// Unregister a tool by name.
    async fn unregister_tool(
        &self,
        input: UnregisterToolInput,
    ) -> Result<UnregisterToolOutput, McpServerError>;

    /// List all registered tool schemas.
    async fn list_tool_schemas(&self) -> Result<Vec<ToolSchema>, McpServerError>;

    /// Find a registered tool by name.
    async fn find_tool(
        &self,
        name: &str,
    ) -> Result<Option<Arc<dyn ToolHandler>>, McpServerError>;

    /// Check if a tool name is registered.
    async fn is_tool_registered(&self, name: &str) -> bool;
}

// ---------------------------------------------------------------------------
// SessionService
// ---------------------------------------------------------------------------

/// Application service for MCP session lifecycle management.
///
/// Manages session creation, validation, and teardown.
///
/// # Contract (Frozen)
///
/// - Sessions are created during the initialize handshake
/// - Sessions must be active for tool/resource/prompt operations
/// - Sessions are isolated — one session's failure doesn't affect others
#[async_trait]
pub trait SessionService: Send + Sync {
    /// End a session by its ID.
    ///
    /// # Errors
    /// - `SessionError::NotFound` if session doesn't exist
    async fn end_session(&self, input: EndSessionInput) -> Result<(), McpServerError>;

    /// List all active sessions.
    async fn list_sessions(&self) -> Result<ListSessionsOutput, McpServerError>;

    /// Validate that a session is active and initialized.
    ///
    /// Returns an error if the session doesn't exist, has ended,
    /// or hasn't completed the initialize handshake.
    async fn validate_session(
        &self,
        session_id: &SessionId,
    ) -> Result<(), McpServerError>;

    /// Evict expired sessions.
    async fn evict_expired(&self) -> Result<usize, McpServerError>;
}

// Re-export domain events for service users
pub use crate::mcp_server::domain::event::McpServerEvent;
pub use crate::mcp_server::domain::value::SessionId;
