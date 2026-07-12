//! Concrete implementations of MCP Server service traits.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#services
//! Implements: McpServerService, ToolRegistryService, SessionService
//!
//! These are the concrete implementations that wire domain aggregates
//! with infrastructure repositories to provide application-level use cases.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::mcp_server::domain::entity::{McpServer, ToolRegistry};
use crate::mcp_server::domain::error::{McpServerError, SessionError};
use crate::mcp_server::domain::value::{
    ClientCapabilities, ClientInfo, ServerCapabilities, SessionId, ToolHandler, ToolSchema,
};
use crate::mcp_server::infrastructure::{
    InMemoryMcpServerRepository, InMemorySessionRepository, InMemoryToolRegistryRepository,
    McpServerRepository, SessionRepository, ToolRegistryRepository,
};

use super::dto::{
    CallToolInput, CallToolOutput, EndSessionInput, GetPromptInput, GetPromptOutput,
    InitializeInput, InitializeOutput, ListPromptsOutput, ListResourcesOutput, ListSessionsOutput,
    ListToolsInput, ListToolsOutput, ReadResourceInput, ReadResourceOutput, RegisterToolInput,
    RegisterToolOutput, ServerStatusInfo, SessionInfo, StartServerInput, StartServerOutput,
    UnregisterToolInput, UnregisterToolOutput,
};
use super::service::{McpServerEvent, McpServerService, SessionService, ToolRegistryService};

// ---------------------------------------------------------------------------
// McpServerServiceImpl
// ---------------------------------------------------------------------------

/// Concrete implementation of McpServerService.
///
/// Wires the McpServer aggregate with in-memory repository, transport,
/// and protocol message dispatching.
pub struct McpServerServiceImpl {
    server_repo: InMemoryMcpServerRepository,
    registry_repo: InMemoryToolRegistryRepository,
    #[allow(dead_code)]
    session_repo: InMemorySessionRepository,
}

impl McpServerServiceImpl {
    /// Create a new service with in-memory storage.
    pub fn new() -> Self {
        Self {
            server_repo: InMemoryMcpServerRepository::new(),
            registry_repo: InMemoryToolRegistryRepository::new(),
            session_repo: InMemorySessionRepository::new(),
        }
    }
}

impl Default for McpServerServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpServerService for McpServerServiceImpl {
    async fn start(&self, input: StartServerInput) -> Result<StartServerOutput, McpServerError> {
        let mut server = McpServer::new(input.config);
        let _events = server.start()?;

        // Create initial ToolRegistry
        let registry = ToolRegistry::default();
        self.registry_repo.save(&registry).await?;
        self.server_repo.save(&server).await?;

        Ok(StartServerOutput {
            started: true,
            server_id: server.id(),
        })
    }

    async fn shutdown(&self) -> Result<(), McpServerError> {
        // In-memory: clear all state
        // In production, we'd drain sessions and close transport
        Ok(())
    }

    async fn initialize(
        &self,
        input: InitializeInput,
    ) -> Result<(InitializeOutput, Vec<McpServerEvent>), McpServerError> {
        let protocol_version = input.protocol_version.clone();
        let client_info = ClientInfo {
            name: input.client_info.name,
            version: input.client_info.version,
        };
        let _client_caps = ClientCapabilities {
            protocol_version: input.protocol_version,
            client_name: input.capabilities.client_name,
            client_version: input.capabilities.client_version,
            supports_progress: input.capabilities.supports_progress,
        };

        // Count registered tools from the registry
        let tool_count = 0;
        let server_caps =
            ServerCapabilities::default_with_counts(tool_count, RESOURCE_COUNT, PROMPT_COUNT);

        let output = InitializeOutput {
            protocol_version: "2025-03-26".to_string(),
            server_capabilities: server_caps,
            server_info: "Rigorix MCP Gateway".to_string(),
        };

        let events = vec![McpServerEvent::McpSessionStarted {
            session_id: SessionId::new(),
            client_name: client_info.name.clone(),
            client_version: client_info.version.clone(),
            protocol_version,
            timestamp: chrono::Utc::now(),
        }];

        Ok((output, events))
    }

    async fn list_tools(&self, _input: ListToolsInput) -> Result<ListToolsOutput, McpServerError> {
        Ok(ListToolsOutput { tools: Vec::new() })
    }

    async fn call_tool(&self, _input: CallToolInput) -> Result<CallToolOutput, McpServerError> {
        Err(McpServerError::Internal(
            "Tool not implemented yet".to_string(),
        ))
    }

    async fn list_resources(&self) -> Result<ListResourcesOutput, McpServerError> {
        Ok(ListResourcesOutput {
            resources: vec![
                crate::mcp_server::domain::value::ResourceSchema::new(
                    "rigorix://audit/{id}",
                    "Audit Trail",
                    "Read an audit trail by execution ID",
                    "text/plain",
                ),
                crate::mcp_server::domain::value::ResourceSchema::new(
                    "rigorix://templates/{name}",
                    "Template",
                    "Read a template by name",
                    "text/plain",
                ),
            ],
        })
    }

    async fn read_resource(
        &self,
        input: ReadResourceInput,
    ) -> Result<ReadResourceOutput, McpServerError> {
        Err(McpServerError::Internal(format!(
            "Resource '{}' not implemented yet",
            input.uri
        )))
    }

    async fn list_prompts(&self) -> Result<ListPromptsOutput, McpServerError> {
        Ok(ListPromptsOutput {
            prompts: vec![crate::mcp_server::domain::value::PromptSchema::new(
                "rigorix_introduction",
                "Introduction to Rigorix tool usage",
                vec![],
            )],
        })
    }

    async fn get_prompt(&self, input: GetPromptInput) -> Result<GetPromptOutput, McpServerError> {
        Err(McpServerError::Internal(format!(
            "Prompt '{}' not implemented yet",
            input.name
        )))
    }

    async fn status(&self) -> Result<ServerStatusInfo, McpServerError> {
        Ok(ServerStatusInfo {
            id: uuid::Uuid::new_v4(),
            status: "running".to_string(),
            transport_mode: Some("stdio".to_string()),
            active_sessions: 0,
            registered_tools: 0,
            has_enterprise_tools: false,
            started_at: Some(chrono::Utc::now()),
        })
    }
}

// Constants
const RESOURCE_COUNT: usize = 2;
const PROMPT_COUNT: usize = 1;

// ---------------------------------------------------------------------------
// McpServerServiceImpl with repositories (full version)
// ---------------------------------------------------------------------------

/// Full-context service implementation that works with all injected repos.
pub struct McpServerServiceWithRepos {
    pub server_repo: Arc<dyn McpServerRepository>,
    pub registry_repo: Arc<dyn ToolRegistryRepository>,
    pub session_repo: Arc<dyn SessionRepository>,
}

#[async_trait]
impl McpServerService for McpServerServiceWithRepos {
    async fn start(&self, input: StartServerInput) -> Result<StartServerOutput, McpServerError> {
        let mut server = McpServer::new(input.config);
        let _events = server.start()?;
        let server_id = server.id();
        self.server_repo.save(&server).await?;

        let registry = ToolRegistry::default();
        self.registry_repo.save(&registry).await?;

        Ok(StartServerOutput {
            started: true,
            server_id,
        })
    }

    async fn shutdown(&self) -> Result<(), McpServerError> {
        Ok(())
    }

    async fn initialize(
        &self,
        input: InitializeInput,
    ) -> Result<(InitializeOutput, Vec<McpServerEvent>), McpServerError> {
        let tool_count = 0; // Would query registry_repo
        let server_caps =
            ServerCapabilities::default_with_counts(tool_count, RESOURCE_COUNT, PROMPT_COUNT);

        let output = InitializeOutput {
            protocol_version: "2025-03-26".to_string(),
            server_capabilities: server_caps,
            server_info: "Rigorix MCP Gateway".to_string(),
        };

        let events = vec![McpServerEvent::McpSessionStarted {
            session_id: SessionId::new(),
            client_name: input.client_info.name.clone(),
            client_version: input.client_info.version.clone(),
            protocol_version: input.protocol_version,
            timestamp: chrono::Utc::now(),
        }];

        Ok((output, events))
    }

    async fn list_tools(&self, _input: ListToolsInput) -> Result<ListToolsOutput, McpServerError> {
        Ok(ListToolsOutput { tools: Vec::new() })
    }

    async fn call_tool(&self, input: CallToolInput) -> Result<CallToolOutput, McpServerError> {
        Err(McpServerError::Internal(format!(
            "Tool '{}' not implemented",
            input.name
        )))
    }

    async fn list_resources(&self) -> Result<ListResourcesOutput, McpServerError> {
        Ok(ListResourcesOutput {
            resources: vec![crate::mcp_server::domain::value::ResourceSchema::new(
                "rigorix://audit/{id}",
                "Audit Trail",
                "Read an audit trail by execution ID",
                "text/plain",
            )],
        })
    }

    async fn read_resource(
        &self,
        input: ReadResourceInput,
    ) -> Result<ReadResourceOutput, McpServerError> {
        Err(McpServerError::Internal(format!(
            "Resource '{}' not implemented",
            input.uri
        )))
    }

    async fn list_prompts(&self) -> Result<ListPromptsOutput, McpServerError> {
        Ok(ListPromptsOutput {
            prompts: vec![crate::mcp_server::domain::value::PromptSchema::new(
                "rigorix_introduction",
                "Introduction to Rigorix tool usage",
                vec![],
            )],
        })
    }

    async fn get_prompt(&self, input: GetPromptInput) -> Result<GetPromptOutput, McpServerError> {
        Err(McpServerError::Internal(format!(
            "Prompt '{}' not implemented",
            input.name
        )))
    }

    async fn status(&self) -> Result<ServerStatusInfo, McpServerError> {
        Ok(ServerStatusInfo {
            id: uuid::Uuid::new_v4(),
            status: "running".to_string(),
            transport_mode: Some("stdio".to_string()),
            active_sessions: 0,
            registered_tools: 0,
            has_enterprise_tools: false,
            started_at: Some(chrono::Utc::now()),
        })
    }
}

// ---------------------------------------------------------------------------
// ToolRegistryServiceImpl
// ---------------------------------------------------------------------------

/// Concrete implementation of ToolRegistryService.
pub struct ToolRegistryServiceImpl {
    #[allow(dead_code)]
    registry_repo: InMemoryToolRegistryRepository,
    handlers: Arc<std::sync::RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,
}

impl ToolRegistryServiceImpl {
    pub fn new() -> Self {
        Self {
            registry_repo: InMemoryToolRegistryRepository::new(),
            handlers: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ToolRegistryServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistryService for ToolRegistryServiceImpl {
    async fn register_tool(
        &self,
        input: RegisterToolInput,
        handler: Arc<dyn ToolHandler>,
    ) -> Result<RegisterToolOutput, McpServerError> {
        // Find the registry (create if needed)
        let _registry_id = {
            // For simplicity, we use a fixed ID
            uuid::Uuid::new_v4()
        };

        // In production: load registry, register, save
        let output = RegisterToolOutput {
            name: input.schema.name.clone(),
            registered: true,
            total_tools: 1,
        };

        // Store the handler
        self.handlers
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock error: {}", e)))?
            .insert(input.schema.name.clone(), handler);

        Ok(output)
    }

    async fn register_enterprise_tools(
        &self,
        _schemas: Vec<ToolSchema>,
        _handler: Arc<dyn ToolHandler>,
    ) -> Result<Vec<RegisterToolOutput>, McpServerError> {
        Ok(Vec::new())
    }

    async fn unregister_tool(
        &self,
        input: UnregisterToolInput,
    ) -> Result<UnregisterToolOutput, McpServerError> {
        self.handlers
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock error: {}", e)))?
            .remove(&input.name);

        Ok(UnregisterToolOutput {
            name: input.name,
            total_tools: 0,
        })
    }

    async fn list_tool_schemas(&self) -> Result<Vec<ToolSchema>, McpServerError> {
        // Would iterate registry
        Ok(Vec::new())
    }

    async fn find_tool(&self, name: &str) -> Result<Option<Arc<dyn ToolHandler>>, McpServerError> {
        let guard = self
            .handlers
            .read()
            .map_err(|e| McpServerError::Internal(format!("Lock error: {}", e)))?;
        Ok(guard.get(name).cloned())
    }

    async fn is_tool_registered(&self, name: &str) -> bool {
        self.handlers
            .read()
            .ok()
            .map(|g| g.contains_key(name))
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// SessionServiceImpl
// ---------------------------------------------------------------------------

/// Concrete implementation of SessionService.
pub struct SessionServiceImpl {
    session_repo: InMemorySessionRepository,
}

impl SessionServiceImpl {
    pub fn new() -> Self {
        Self {
            session_repo: InMemorySessionRepository::new(),
        }
    }
}

impl Default for SessionServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionService for SessionServiceImpl {
    async fn end_session(&self, input: EndSessionInput) -> Result<(), McpServerError> {
        self.session_repo
            .delete(&input.session_id)
            .await
            .map_err(|_| {
                McpServerError::Session(SessionError::NotFound(input.session_id.to_string()))
            })
    }

    async fn list_sessions(&self) -> Result<ListSessionsOutput, McpServerError> {
        let sessions = self.session_repo.list_active().await?;
        let session_infos: Vec<SessionInfo> = sessions
            .into_iter()
            .map(|s| SessionInfo {
                session_id: s.id,
                client_name: s.client_info.name,
                status: format!("{:?}", s.status),
                started_at: s.started_at,
                last_active_at: s.last_active_at,
            })
            .collect();

        let total = session_infos.len();
        Ok(ListSessionsOutput {
            sessions: session_infos,
            total,
        })
    }

    async fn validate_session(&self, _session_id: &SessionId) -> Result<(), McpServerError> {
        // Would check the session exists and is active
        Ok(())
    }

    async fn evict_expired(&self) -> Result<usize, McpServerError> {
        // In-memory: no eviction needed
        Ok(0)
    }
}
