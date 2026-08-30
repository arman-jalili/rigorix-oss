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
    ClientCapabilities, ClientInfo, ContentItem, ServerCapabilities, SessionId, ToolHandler,
    ToolSchema,
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
use super::service::{
    McpServerEvent, McpServerService, McpToolExecutor, SessionService, ToolRegistryService,
};

// ---------------------------------------------------------------------------
// McpServerServiceImpl
// ---------------------------------------------------------------------------

/// Concrete implementation of McpServerService.
///
/// Wires the McpServer aggregate with in-memory repository, transport,
/// and protocol message dispatching. When an executor is wired via
/// [`McpServerServiceImpl::with_executor`], the protocol surface is live.
pub struct McpServerServiceImpl {
    server_repo: InMemoryMcpServerRepository,
    registry_repo: InMemoryToolRegistryRepository,
    #[allow(dead_code)]
    session_repo: InMemorySessionRepository,
    executor: Option<Arc<dyn McpToolExecutor>>,
    tool_schemas: Vec<ToolSchema>,
}

impl McpServerServiceImpl {
    /// Create a new service with in-memory storage.
    pub fn new() -> Self {
        Self {
            server_repo: InMemoryMcpServerRepository::new(),
            registry_repo: InMemoryToolRegistryRepository::new(),
            session_repo: InMemorySessionRepository::new(),
            executor: None,
            tool_schemas: Vec::new(),
        }
    }

    /// Wire a live executor and the tool schemas it can dispatch.
    pub fn with_executor(
        mut self,
        executor: Arc<dyn McpToolExecutor>,
        tool_schemas: Vec<ToolSchema>,
    ) -> Self {
        self.executor = Some(executor);
        self.tool_schemas = tool_schemas;
        self
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
            server_capabilities: server_caps.clone(),
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

    async fn list_tools(&self, input: ListToolsInput) -> Result<ListToolsOutput, McpServerError> {
        let tools = match input.filter {
            Some(super::dto::ToolFilter::Oss) => self
                .tool_schemas
                .iter()
                .filter(|t| t.is_oss_tool())
                .cloned()
                .collect(),
            Some(super::dto::ToolFilter::Enterprise) => self
                .tool_schemas
                .iter()
                .filter(|t| t.is_enterprise_tool())
                .cloned()
                .collect(),
            Some(super::dto::ToolFilter::All) | None => self.tool_schemas.clone(),
        };
        Ok(ListToolsOutput { tools })
    }

    async fn call_tool(&self, input: CallToolInput) -> Result<CallToolOutput, McpServerError> {
        let Some(ref executor) = self.executor else {
            return Err(McpServerError::Internal(
                "No tool executor wired — server is not connected to a backend".to_string(),
            ));
        };
        match executor.execute_tool(&input.name, input.arguments).await {
            Ok(result) => Ok(CallToolOutput {
                content: vec![ContentItem::text(
                    serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string()),
                )],
                is_error: false,
            }),
            Err(err) => Ok(CallToolOutput {
                content: vec![ContentItem::text(err)],
                is_error: true,
            }),
        }
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
        let Some(ref executor) = self.executor else {
            return Err(McpServerError::Internal(
                "No tool executor wired — server is not connected to a backend".to_string(),
            ));
        };
        let text = executor
            .read_resource(&input.uri)
            .await
            .map_err(McpServerError::Internal)?;
        Ok(ReadResourceOutput {
            uri: input.uri.clone(),
            mime_type: "text/plain".to_string(),
            text,
        })
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
        match input.name.as_str() {
            "rigorix_introduction" => Ok(GetPromptOutput {
                description: Some("Introduction to Rigorix tool usage".to_string()),
                messages: vec![super::dto::PromptMessageDto {
                    role: "user".to_string(),
                    content: super::dto::PromptContentDto::Text {
                        text: INTRODUCTION_PROMPT_TEXT.to_string(),
                    },
                }],
            }),
            _ => Err(McpServerError::Internal(format!(
                "Prompt '{}' not found",
                input.name
            ))),
        }
    }

    async fn status(&self) -> Result<ServerStatusInfo, McpServerError> {
        Ok(ServerStatusInfo {
            id: uuid::Uuid::new_v4(),
            status: "running".to_string(),
            transport_mode: Some("stdio".to_string()),
            active_sessions: 0,
            registered_tools: self.tool_schemas.len(),
            has_enterprise_tools: self.tool_schemas.iter().any(|t| t.is_enterprise_tool()),
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

/// The `rigorix_introduction` prompt body — shared so the stdio server and the
/// `mcp_server` module advertise the same content.
pub const INTRODUCTION_PROMPT_TEXT: &str = concat!(
    "You are using Rigorix, an AI code-governance engine that plans, ",
    "executes, and audits multi-step work against a frozen contract.\n\n",
    "Key capabilities:\n",
    "  • rigorix_list_templates / rigorix_get_template — inspect plan templates\n",
    "  • rigorix_validate_plan — check a plan against enforcement policies\n",
    "  • rigorix_run — execute a template's DAG through the engine\n",
    "  • rigorix_approve_execution — human sign-off when a step requires it\n",
    "    (plans may mark steps requires_approval: true; execution pauses until approved)\n",
    "  • rigorix_check_enforcement — current enforcement status and budget\n",
    "  • rigorix_get_execution_status / rigorix_get_audit_log — inspect evidence\n\n",
    "All execution is gated: budgets, safety caps, tool policy, and (when ",
    "configured) a permission mode (read_only / workspace_write / ",
    "dangerous_full_access). Every audit event is timestamped and, when a ",
    "signing key is configured, HMAC-signed for tamper-evident evidence.\n\n",
    "Start by listing templates, then run one with rigorix_run.",
);

/// Full-context service implementation that works with all injected repos.
///
/// When an `McpToolExecutor` and tool schemas are wired in, the protocol
/// surface (tools/call, resources/read, prompts/get) is live: calls are
/// dispatched to the production handlers instead of returning stubs.
pub struct McpServerServiceWithRepos {
    pub server_repo: Arc<dyn McpServerRepository>,
    pub registry_repo: Arc<dyn ToolRegistryRepository>,
    pub session_repo: Arc<dyn SessionRepository>,
    /// Optional live dispatcher backed by the composition root.
    pub executor: Option<Arc<dyn McpToolExecutor>>,
    /// Tool schemas advertised by `tools/list`.
    pub tool_schemas: Vec<ToolSchema>,
}

impl McpServerServiceWithRepos {
    /// Create the service with the given repositories.
    pub fn new(
        server_repo: Arc<dyn McpServerRepository>,
        registry_repo: Arc<dyn ToolRegistryRepository>,
        session_repo: Arc<dyn SessionRepository>,
    ) -> Self {
        Self {
            server_repo,
            registry_repo,
            session_repo,
            executor: None,
            tool_schemas: Vec::new(),
        }
    }

    /// Wire a live executor and the tool schemas it can dispatch.
    pub fn with_executor(
        mut self,
        executor: Arc<dyn McpToolExecutor>,
        tool_schemas: Vec<ToolSchema>,
    ) -> Self {
        self.executor = Some(executor);
        self.tool_schemas = tool_schemas;
        self
    }
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
            server_capabilities: server_caps.clone(),
            server_info: "Rigorix MCP Gateway".to_string(),
        };

        // GAP-A-22: the initialize handshake must create a REAL session —
        // previously the event carried a throwaway SessionId and the
        // session repository was never written.
        let client_caps = ClientCapabilities {
            protocol_version: input.protocol_version.clone(),
            client_name: Some(input.client_info.name.clone()),
            client_version: input.client_info.version.clone(),
            supports_progress: false,
        };
        let session = crate::mcp_server::domain::entity::Session::new(
            input.client_info.clone(),
            client_caps,
            server_caps,
        );
        self.session_repo
            .save(&session)
            .await
            .map_err(|e| McpServerError::Transport(format!("session save failed: {e}")))?;

        let events = vec![McpServerEvent::McpSessionStarted {
            session_id: session.id.clone(),
            client_name: input.client_info.name.clone(),
            client_version: input.client_info.version.clone(),
            protocol_version: input.protocol_version,
            timestamp: chrono::Utc::now(),
        }];

        Ok((output, events))
    }

    async fn list_tools(&self, input: ListToolsInput) -> Result<ListToolsOutput, McpServerError> {
        let tools = match input.filter {
            Some(super::dto::ToolFilter::Oss) => self
                .tool_schemas
                .iter()
                .filter(|t| t.is_oss_tool())
                .cloned()
                .collect(),
            Some(super::dto::ToolFilter::Enterprise) => self
                .tool_schemas
                .iter()
                .filter(|t| t.is_enterprise_tool())
                .cloned()
                .collect(),
            Some(super::dto::ToolFilter::All) | None => self.tool_schemas.clone(),
        };
        Ok(ListToolsOutput { tools })
    }

    async fn call_tool(&self, input: CallToolInput) -> Result<CallToolOutput, McpServerError> {
        let Some(ref executor) = self.executor else {
            return Err(McpServerError::Internal(
                "No tool executor wired — server is not connected to a backend".to_string(),
            ));
        };
        match executor.execute_tool(&input.name, input.arguments).await {
            Ok(result) => Ok(CallToolOutput {
                content: vec![ContentItem::text(
                    serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string()),
                )],
                is_error: false,
            }),
            Err(err) => Ok(CallToolOutput {
                content: vec![ContentItem::text(err)],
                is_error: true,
            }),
        }
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
        let Some(ref executor) = self.executor else {
            return Err(McpServerError::Internal(
                "No tool executor wired — server is not connected to a backend".to_string(),
            ));
        };
        let text = executor
            .read_resource(&input.uri)
            .await
            .map_err(McpServerError::Internal)?;
        Ok(ReadResourceOutput {
            uri: input.uri.clone(),
            mime_type: "text/plain".to_string(),
            text,
        })
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
        match input.name.as_str() {
            "rigorix_introduction" => Ok(GetPromptOutput {
                description: Some("Introduction to Rigorix tool usage".to_string()),
                messages: vec![super::dto::PromptMessageDto {
                    role: "user".to_string(),
                    content: super::dto::PromptContentDto::Text {
                        text: INTRODUCTION_PROMPT_TEXT.to_string(),
                    },
                }],
            }),
            _ => Err(McpServerError::Internal(format!(
                "Prompt '{}' not found",
                input.name
            ))),
        }
    }

    async fn status(&self) -> Result<ServerStatusInfo, McpServerError> {
        Ok(ServerStatusInfo {
            id: uuid::Uuid::new_v4(),
            status: "running".to_string(),
            transport_mode: Some("stdio".to_string()),
            active_sessions: 0,
            registered_tools: self.tool_schemas.len(),
            has_enterprise_tools: self.tool_schemas.iter().any(|t| t.is_enterprise_tool()),
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
