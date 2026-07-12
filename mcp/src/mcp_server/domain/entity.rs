//! Aggregate roots and entities for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#aggregates
//! Implements: Contract Freeze — McpServer, ToolRegistry, Session aggregates
//!
//! # Aggregates
//!
//! - **McpServer** — Core aggregate root orchestrating transport, session management,
//!   tool registration, and request routing. Enforces the invariant that only one
//!   transport mode is active at a time.
//!
//! - **ToolRegistry** — Aggregate root holding all registered MCP tools with their
//!   JSON schemas and handler functions. Enforces tool naming conventions and
//!   separation between OSS and enterprise tools.
//!
//! - **Session** — Entity representing an active MCP client connection with negotiated
//!   capabilities, client metadata, and lifecycle state.
//!
//! # Contract (Frozen)
//!
//! - All state transitions go through aggregate methods, never direct field mutation
//! - Methods return Result<Vec<McpServerEvent>, McpServerError> for event sourcing
//! - Aggregates enforce invariants — invalid transitions return Err
//! - No pub fields on aggregates — always encapsulate
//! - Cross-aggregate references use IDs, not object references

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::error::{McpServerError, RegistrationError, SessionError};
use super::event::McpServerEvent;
use super::value::{
    ClientCapabilities, ClientInfo, ServerCapabilities, ServerConfig, SessionId, SessionMetadata,
    SessionStatus, ToolHandler, ToolSchema, TransportMode,
};

// ---------------------------------------------------------------------------
// McpServer — Aggregate Root
// ---------------------------------------------------------------------------

/// Core aggregate root for the MCP Server.
///
/// Orchestrates transport management, session lifecycle, tool registration,
/// and request routing. Every MCP client connection flows through this aggregate.
///
/// # Invariants (Frozen)
///
/// - Only one transport mode active at a time (stdio XOR SSE)
/// - Transport MUST be open before accepting sessions
/// - Sessions are isolated — one session's failure doesn't affect others
/// - Graceful shutdown: drain active requests → close transport → drop sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Unique server identifier.
    id: Uuid,

    /// Server configuration.
    config: ServerConfig,

    /// Server status.
    status: McpServerStatus,

    /// Active sessions keyed by session ID.
    sessions: HashMap<SessionId, Session>,

    /// Active transport mode.
    active_transport: Option<TransportMode>,

    /// When the server was started.
    started_at: Option<DateTime<Utc>>,

    /// When the server was last stopped.
    stopped_at: Option<DateTime<Utc>>,
}

impl McpServer {
    /// Create a new MCP Server with the given configuration.
    ///
    /// The server starts in the `Stopped` state. Call `start()` to initialize.
    pub fn new(config: ServerConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            status: McpServerStatus::Stopped,
            sessions: HashMap::new(),
            active_transport: None,
            started_at: None,
            stopped_at: None,
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Return the server's unique identifier.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Return the server configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Return the current server status.
    pub fn status(&self) -> McpServerStatus {
        self.status
    }

    /// Return the active transport mode, if any.
    pub fn active_transport(&self) -> Option<TransportMode> {
        self.active_transport
    }

    /// Return the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Return a reference to all active sessions.
    pub fn sessions(&self) -> &HashMap<SessionId, Session> {
        &self.sessions
    }

    // -----------------------------------------------------------------------
    // Lifecycle Methods
    // -----------------------------------------------------------------------

    /// Start the server.
    ///
    /// Transitions from `Stopped` to `Initializing` state.
    /// Returns an error if the server is already running or shutting down.
    pub fn start(&mut self) -> Result<Vec<McpServerEvent>, McpServerError> {
        match self.status {
            McpServerStatus::Stopped => {
                self.status = McpServerStatus::Initializing;
                self.started_at = Some(Utc::now());
                Ok(Vec::new())
            }
            McpServerStatus::Running | McpServerStatus::Initializing => {
                Err(McpServerError::AlreadyRunning)
            }
            McpServerStatus::ShuttingDown => Err(McpServerError::ShuttingDown),
        }
    }

    /// Mark the transport as opened and transition to `Running` state.
    ///
    /// Must be called after `start()` when the transport is successfully opened.
    pub fn on_transport_opened(
        &mut self,
        mode: TransportMode,
    ) -> Result<Vec<McpServerEvent>, McpServerError> {
        if self.status != McpServerStatus::Initializing {
            return Err(McpServerError::NotInitialized);
        }
        self.status = McpServerStatus::Running;
        self.active_transport = Some(mode);
        Ok(Vec::new())
    }

    /// Shutdown the server gracefully.
    ///
    /// Drains active sessions, closes transport, and transitions to `Stopped`.
    pub fn shutdown(&mut self) -> Result<Vec<McpServerEvent>, McpServerError> {
        match self.status {
            McpServerStatus::Running | McpServerStatus::Initializing => {
                self.status = McpServerStatus::ShuttingDown;
                self.stopped_at = Some(Utc::now());
                let events: Vec<McpServerEvent> = self
                    .sessions
                    .drain()
                    .map(|(session_id, session)| McpServerEvent::McpSessionEnded {
                        session_id,
                        reason: "server_shutdown".to_string(),
                        duration_ms: session
                            .started_at
                            .map(|t| (Utc::now() - t).num_milliseconds() as u64)
                            .unwrap_or(0),
                        timestamp: Utc::now(),
                    })
                    .collect();
                self.status = McpServerStatus::Stopped;
                self.active_transport = None;
                Ok(events)
            }
            McpServerStatus::Stopped => Ok(Vec::new()),
            McpServerStatus::ShuttingDown => Err(McpServerError::ShuttingDown),
        }
    }

    // -----------------------------------------------------------------------
    // Session Management
    // -----------------------------------------------------------------------

    /// Create a new session for an MCP client.
    ///
    /// Validates that the server is running and the maximum session count
    /// has not been reached.
    pub fn create_session(
        &mut self,
        client_info: ClientInfo,
        capabilities: ClientCapabilities,
        server_capabilities: ServerCapabilities,
    ) -> Result<(Session, Vec<McpServerEvent>), McpServerError> {
        if self.status != McpServerStatus::Running {
            return Err(McpServerError::NotInitialized);
        }

        if self.sessions.len() >= self.config.max_sessions {
            return Err(McpServerError::Session(SessionError::MaxSessionsReached(
                self.config.max_sessions,
            )));
        }

        let session = Session::new(client_info, capabilities, server_capabilities);

        let events = vec![McpServerEvent::McpSessionStarted {
            session_id: session.id,
            client_name: session.client_info.name.clone(),
            client_version: session.client_info.version.clone(),
            protocol_version: session.client_capabilities.protocol_version.clone(),
            timestamp: Utc::now(),
        }];

        self.sessions.insert(session.id, session.clone());
        Ok((session, events))
    }

    /// End an existing session.
    pub fn end_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
    ) -> Result<Vec<McpServerEvent>, McpServerError> {
        let session = self.sessions.remove(&session_id).ok_or_else(|| {
            McpServerError::Session(SessionError::NotFound(session_id.to_string()))
        })?;

        let duration_ms = session
            .started_at
            .map(|t| (Utc::now() - t).num_milliseconds() as u64)
            .unwrap_or(0);

        Ok(vec![McpServerEvent::McpSessionEnded {
            session_id,
            reason: reason.into(),
            duration_ms,
            timestamp: Utc::now(),
        }])
    }

    /// Find a session by its ID.
    pub fn find_session(&self, session_id: &SessionId) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    /// Evict expired sessions (sessions past their timeout).
    pub fn evict_expired_sessions(&mut self) -> Vec<McpServerEvent> {
        let now = Utc::now();
        let timeout = chrono::Duration::seconds(self.config.session_timeout_secs as i64);
        let mut events = Vec::new();

        self.sessions.retain(|session_id, session| {
            let elapsed = session
                .started_at
                .map(|t| now - t)
                .unwrap_or(chrono::Duration::zero());
            if elapsed > timeout {
                events.push(McpServerEvent::McpSessionEnded {
                    session_id: *session_id,
                    reason: "timeout".to_string(),
                    duration_ms: elapsed.num_milliseconds() as u64,
                    timestamp: now,
                });
                false
            } else {
                true
            }
        });

        events
    }
}

// ---------------------------------------------------------------------------
// McpServerStatus — Server lifecycle state
// ---------------------------------------------------------------------------

/// Lifecycle status of the MCP Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
    /// Server is stopped or has not been started.
    Stopped,
    /// Server is initializing (transport being opened).
    Initializing,
    /// Server is running and accepting connections.
    Running,
    /// Server is shutting down (draining requests).
    ShuttingDown,
}

impl McpServerStatus {
    /// Returns true if the server is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, McpServerStatus::Stopped)
    }

    /// Returns true if the server can accept new sessions.
    pub fn can_accept_sessions(&self) -> bool {
        matches!(self, McpServerStatus::Running)
    }
}

// ---------------------------------------------------------------------------
// Session — Entity
// ---------------------------------------------------------------------------

/// An active MCP client session with negotiated capabilities.
///
/// Each session represents one MCP client connection with its own
/// lifecycle, metadata, and negotiated protocol capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,

    /// Client identifying information.
    pub client_info: ClientInfo,

    /// Client capabilities received during initialization.
    pub client_capabilities: ClientCapabilities,

    /// Server capabilities negotiated during initialization.
    pub server_capabilities: ServerCapabilities,

    /// Current session status.
    pub status: SessionStatus,

    /// Whether the initialize handshake has completed.
    pub initialized: bool,

    /// When the session was created.
    pub started_at: Option<DateTime<Utc>>,

    /// When the session was last active.
    pub last_active_at: Option<DateTime<Utc>>,

    /// Additional metadata.
    pub metadata: SessionMetadata,
}

impl Session {
    /// Create a new session with the given client and server capabilities.
    pub fn new(
        client_info: ClientInfo,
        client_capabilities: ClientCapabilities,
        server_capabilities: ServerCapabilities,
    ) -> Self {
        Self {
            id: SessionId::new(),
            client_info,
            client_capabilities,
            server_capabilities,
            status: SessionStatus::Pending,
            initialized: false,
            started_at: Some(Utc::now()),
            last_active_at: Some(Utc::now()),
            metadata: SessionMetadata {
                labels: HashMap::new(),
            },
        }
    }

    /// Mark the session as initialized after a successful handshake.
    pub fn mark_initialized(&mut self) -> Result<(), SessionError> {
        if self.initialized {
            return Err(SessionError::InvalidState(
                "Session already initialized".to_string(),
            ));
        }
        if self.status.is_terminal() {
            return Err(SessionError::InvalidState("Session has ended".to_string()));
        }
        self.initialized = true;
        self.status = SessionStatus::Active;
        self.last_active_at = Some(Utc::now());
        Ok(())
    }

    /// Mark the session as draining (no new requests).
    pub fn mark_draining(&mut self) -> Result<(), SessionError> {
        if !self.status.is_active() {
            return Err(SessionError::InvalidState(format!(
                "Cannot drain session in state: {:?}",
                self.status
            )));
        }
        self.status = SessionStatus::Draining;
        Ok(())
    }

    /// Update the last active timestamp.
    pub fn touch(&mut self) {
        self.last_active_at = Some(Utc::now());
    }
}

// ---------------------------------------------------------------------------
// ToolRegistry — Aggregate Root
// ---------------------------------------------------------------------------

/// Aggregate root holding all registered MCP tools with their schemas and handlers.
///
/// The ToolRegistry enforces:
/// - Unique tool names
/// - `rigorix_` prefix for OSS tools, `rigorix_enterprise_` for enterprise
/// - Separation between OSS and enterprise registration paths
/// - Schema immutability after registration
///
/// # Invariants (Frozen)
///
/// - Tool names are unique (registration of duplicate returns error)
/// - Tool names must match `rigorix_` prefix for OSS, `rigorix_enterprise_` for enterprise
/// - Enterprise tools are registered separately — never mixed with OSS path
/// - Schemas are immutable after registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistry {
    /// Unique registry identifier.
    id: Uuid,

    /// Registered tools keyed by name.
    tools: HashMap<String, RegisteredToolProxy>,

    /// Maximum number of tools allowed.
    max_tools: usize,

    /// Whether enterprise tools are present.
    has_enterprise_tools: bool,
}

/// Serializable proxy for RegisteredTool (without the handler trait object).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RegisteredToolProxy {
    pub name: String,
    pub schema: ToolSchema,
    pub is_enterprise: bool,
}

impl ToolRegistry {
    /// Create a new ToolRegistry.
    pub fn new(max_tools: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            tools: HashMap::new(),
            max_tools,
            has_enterprise_tools: false,
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Return the registry's unique identifier.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Return the number of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Return the maximum number of allowed tools.
    pub fn max_tools(&self) -> usize {
        self.max_tools
    }

    /// Returns true if enterprise tools are registered.
    pub fn has_enterprise_tools(&self) -> bool {
        self.has_enterprise_tools
    }

    /// Check if a tool name is registered.
    pub fn is_registered(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// List all registered tool schemas.
    pub fn list_schemas(&self) -> Vec<ToolSchema> {
        self.tools
            .values()
            .map(|proxy| proxy.schema.clone())
            .collect()
    }

    /// Find a registered tool proxy by name.
    #[allow(dead_code)]
    pub(crate) fn find_proxy(&self, name: &str) -> Option<&RegisteredToolProxy> {
        self.tools.get(name)
    }

    // -----------------------------------------------------------------------
    // Registration Methods
    // -----------------------------------------------------------------------

    /// Register a tool with its schema and handler.
    ///
    /// Validates naming conventions and uniqueness.
    /// Returns an error if:
    /// - The tool name is empty or doesn't follow `rigorix_` prefix convention
    /// - A tool with the same name is already registered
    /// - The maximum tool count has been reached
    /// - Enterprise tools are registered through this path
    pub fn register(
        &mut self,
        schema: ToolSchema,
        handler: std::sync::Arc<dyn ToolHandler>,
    ) -> Result<Vec<McpServerEvent>, RegistrationError> {
        self.validate_and_register(schema, handler, false)
    }

    /// Register enterprise tools separately.
    ///
    /// Enterprise tools must use the `rigorix_enterprise_` prefix.
    pub fn register_enterprise(
        &mut self,
        schema: ToolSchema,
        handler: std::sync::Arc<dyn ToolHandler>,
    ) -> Result<Vec<McpServerEvent>, RegistrationError> {
        self.validate_and_register(schema, handler, true)
    }

    /// Internal validation and registration logic.
    fn validate_and_register(
        &mut self,
        schema: ToolSchema,
        _handler: std::sync::Arc<dyn ToolHandler>,
        is_enterprise: bool,
    ) -> Result<Vec<McpServerEvent>, RegistrationError> {
        let name = &schema.name;

        // Validate name
        if name.is_empty() {
            return Err(RegistrationError::InvalidName(
                "Tool name cannot be empty".to_string(),
            ));
        }

        // Check prefix conventions
        if is_enterprise {
            if !name.starts_with("rigorix_enterprise_") {
                return Err(RegistrationError::InvalidName(format!(
                    "Enterprise tool '{}' must start with 'rigorix_enterprise_' prefix",
                    name
                )));
            }
        } else {
            if !name.starts_with("rigorix_") {
                return Err(RegistrationError::InvalidName(format!(
                    "Tool '{}' must start with 'rigorix_' prefix",
                    name
                )));
            }
            if name.starts_with("rigorix_enterprise_") {
                return Err(RegistrationError::EnterpriseRegistrationForbidden);
            }
        }

        // Check duplicate
        if self.tools.contains_key(name) {
            return Err(RegistrationError::AlreadyRegistered(name.clone()));
        }

        // Check max tools
        if self.tools.len() >= self.max_tools {
            return Err(RegistrationError::MaxToolsReached(self.max_tools));
        }

        // Register
        let proxy = RegisteredToolProxy {
            name: name.clone(),
            schema: schema.clone(),
            is_enterprise,
        };

        self.tools.insert(name.clone(), proxy);
        if is_enterprise {
            self.has_enterprise_tools = true;
        }

        let total_tools = self.tools.len();

        Ok(vec![McpServerEvent::ToolRegistered {
            tool_name: name.clone(),
            is_enterprise,
            total_tools,
            timestamp: Utc::now(),
        }])
    }

    /// Unregister a tool by name.
    pub fn unregister(&mut self, name: &str) -> Result<Vec<McpServerEvent>, RegistrationError> {
        if !self.tools.contains_key(name) {
            return Err(RegistrationError::NotFound(name.to_string()));
        }

        let removed = self.tools.remove(name);
        if let Some(proxy) = removed
            && proxy.is_enterprise
        {
            self.has_enterprise_tools = self.tools.values().any(|p| p.is_enterprise);
        }

        Ok(vec![McpServerEvent::ToolRegistered {
            tool_name: name.to_string(),
            is_enterprise: false,
            total_tools: self.tools.len(),
            timestamp: Utc::now(),
        }])
    }

    /// Merge enterprise tool schemas into the registry.
    ///
    /// Used when the Enterprise Proxy discovers tools dynamically.
    /// All schemas are registered under the enterprise prefix convention.
    pub fn merge_enterprise_schemas(
        &mut self,
        schemas: Vec<ToolSchema>,
        handler: std::sync::Arc<dyn ToolHandler>,
    ) -> Result<Vec<McpServerEvent>, RegistrationError> {
        let mut events = Vec::new();
        for schema in schemas {
            let result = self.register_enterprise(schema, handler.clone())?;
            events.extend(result);
        }
        Ok(events)
    }

    /// Return the number of OSS (non-enterprise) tools.
    pub fn oss_tool_count(&self) -> usize {
        self.tools.values().filter(|p| !p.is_enterprise).count()
    }
}

// ---------------------------------------------------------------------------
// Default impl for ToolRegistry
// ---------------------------------------------------------------------------

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct TestHandler {
        schema: ToolSchema,
    }

    impl ToolHandler for TestHandler {
        fn handle(
            &self,
            _params: serde_json::Value,
        ) -> Result<
            crate::mcp_server::domain::value::ToolResult,
            crate::mcp_server::domain::value::JsonRpcError,
        > {
            Ok(crate::mcp_server::domain::value::ToolResult::text("ok"))
        }

        fn schema(&self) -> &ToolSchema {
            &self.schema
        }
    }

    fn make_handler(name: &str) -> Arc<dyn ToolHandler> {
        Arc::new(TestHandler {
            schema: ToolSchema::new(name, "test", serde_json::json!({})),
        })
    }

    #[test]
    fn test_tool_registry_register_and_list() {
        let mut registry = ToolRegistry::default();
        let schema = ToolSchema::new("rigorix_test", "Test tool", serde_json::json!({}));
        let handler = make_handler("rigorix_test");

        let result = registry.register(schema, handler);
        assert!(result.is_ok());
        assert_eq!(registry.tool_count(), 1);
        assert_eq!(registry.list_schemas().len(), 1);
    }

    #[test]
    fn test_tool_registry_rejects_duplicate() {
        let mut registry = ToolRegistry::default();
        let schema1 = ToolSchema::new("rigorix_test", "Test tool", serde_json::json!({}));
        let schema2 = ToolSchema::new("rigorix_test", "Duplicate", serde_json::json!({}));

        let handler1 = make_handler("rigorix_test");
        let handler2 = make_handler("rigorix_test");

        assert!(registry.register(schema1, handler1).is_ok());
        assert!(matches!(
            registry.register(schema2, handler2),
            Err(RegistrationError::AlreadyRegistered(_))
        ));
    }

    #[test]
    fn test_tool_registry_rejects_invalid_prefix() {
        let mut registry = ToolRegistry::default();
        let schema = ToolSchema::new("invalid_name", "No prefix", serde_json::json!({}));
        let handler = make_handler("invalid_name");

        assert!(matches!(
            registry.register(schema, handler),
            Err(RegistrationError::InvalidName(_))
        ));
    }

    #[test]
    fn test_tool_registry_enterprise_registration() {
        let mut registry = ToolRegistry::default();
        let schema = ToolSchema::new(
            "rigorix_enterprise_custom",
            "Enterprise tool",
            serde_json::json!({}),
        );
        let handler = make_handler("rigorix_enterprise_custom");

        assert!(registry.register_enterprise(schema, handler).is_ok());
        assert!(registry.has_enterprise_tools());
        assert_eq!(registry.oss_tool_count(), 0);
    }

    #[test]
    fn test_tool_registry_enterprise_through_oss_fails() {
        let mut registry = ToolRegistry::default();
        let schema = ToolSchema::new(
            "rigorix_enterprise_custom",
            "Enterprise tool",
            serde_json::json!({}),
        );
        let handler = make_handler("rigorix_enterprise_custom");

        assert!(matches!(
            registry.register(schema, handler),
            Err(RegistrationError::EnterpriseRegistrationForbidden)
        ));
    }

    #[test]
    fn test_session_lifecycle() {
        let session = Session::new(
            ClientInfo {
                name: "test-client".to_string(),
                version: Some("1.0".to_string()),
            },
            ClientCapabilities {
                protocol_version: "2025-03-26".to_string(),
                client_name: Some("test-client".to_string()),
                client_version: Some("1.0".to_string()),
                supports_progress: false,
            },
            ServerCapabilities::default_with_counts(0, 0, 0),
        );

        assert_eq!(session.status, SessionStatus::Pending);
        assert!(!session.initialized);
    }

    #[test]
    fn test_mcp_server_create_and_start() {
        let config = ServerConfig::default();
        let mut server = McpServer::new(config);

        assert_eq!(server.status(), McpServerStatus::Stopped);

        let events = server.start();
        assert!(events.is_ok());
        assert_eq!(server.status(), McpServerStatus::Initializing);

        let transport_events = server.on_transport_opened(TransportMode::Stdio);
        assert!(transport_events.is_ok());
        assert_eq!(server.status(), McpServerStatus::Running);
    }

    #[test]
    fn test_mcp_server_rejects_session_before_start() {
        let config = ServerConfig::default();
        let mut server = McpServer::new(config);

        let result = server.create_session(
            ClientInfo {
                name: "test".to_string(),
                version: None,
            },
            ClientCapabilities {
                protocol_version: "2025-03-26".to_string(),
                client_name: None,
                client_version: None,
                supports_progress: false,
            },
            ServerCapabilities::default_with_counts(0, 0, 0),
        );

        assert!(matches!(result, Err(McpServerError::NotInitialized)));
    }
}
