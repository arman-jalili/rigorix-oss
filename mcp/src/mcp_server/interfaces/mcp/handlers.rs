//! Concrete MCP protocol handler implementations.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#mcp-handlers
//! Implements: InitializeHandler, ListToolsHandler, CallToolHandler,
//! ListResourcesHandler, ReadResourceHandler, ListPromptsHandler, GetPromptHandler
//!
//! Each handler implements a single MCP protocol method and delegates
//! to the appropriate application service.

use async_trait::async_trait;
use serde_json::json;

use crate::mcp_server::application::dto::{
    CallToolInput, GetPromptInput, InitializeInput, ListToolsInput, ReadResourceInput,
};
use crate::mcp_server::application::service::McpServerService;
use crate::mcp_server::domain::value::{
    ClientCapabilities, ClientInfo, JsonRpcError, SessionId,
};
use super::{
    CallToolHandler, CancelledHandler, GetPromptHandler, InitializeHandler, InitializedHandler,
    ListPromptsHandler, ListResourcesHandler, ListToolsHandler, ReadResourceHandler,
    TransportHandle,
};

// ---------------------------------------------------------------------------
// InitializeHandlerImpl
// ---------------------------------------------------------------------------

pub struct InitializeHandlerImpl {
    service: Box<dyn McpServerService>,
}

impl InitializeHandlerImpl {
    pub fn new(service: Box<dyn McpServerService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl InitializeHandler for InitializeHandlerImpl {
    async fn handle_initialize(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let protocol_version = params
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                JsonRpcError::invalid_params("Missing required field: protocolVersion")
            })?;

        let client_name = params
            .get("clientInfo")
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let client_version = params
            .get("clientInfo")
            .and_then(|c| c.get("version"))
            .and_then(|v| v.as_str());

        let input = InitializeInput {
            protocol_version: protocol_version.to_string(),
            client_info: ClientInfo {
                name: client_name.to_string(),
                version: client_version.map(|s| s.to_string()),
            },
            capabilities: ClientCapabilities {
                protocol_version: protocol_version.to_string(),
                client_name: Some(client_name.to_string()),
                client_version: client_version.map(|s| s.to_string()),
                supports_progress: false,
            },
        };

        let (output, _events) = self
            .service
            .initialize(input)
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        Ok(json!({
            "protocolVersion": output.protocol_version,
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {}
            },
            "serverInfo": {
                "name": output.server_info,
                "version": "0.1.0"
            }
        }))
    }
}

// ---------------------------------------------------------------------------
// InitializedHandlerImpl
// ---------------------------------------------------------------------------

pub struct InitializedHandlerImpl;

#[async_trait]
impl InitializedHandler for InitializedHandlerImpl {
    async fn handle_initialized(&self, _session_id: &str) -> Result<(), JsonRpcError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ListToolsHandlerImpl
// ---------------------------------------------------------------------------

pub struct ListToolsHandlerImpl {
    service: Box<dyn McpServerService>,
}

impl ListToolsHandlerImpl {
    pub fn new(service: Box<dyn McpServerService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ListToolsHandler for ListToolsHandlerImpl {
    async fn handle_list_tools(&self) -> Result<serde_json::Value, JsonRpcError> {
        let output = self
            .service
            .list_tools(ListToolsInput { filter: None })
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        let tools: Vec<serde_json::Value> = output
            .tools
            .into_iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "description": s.description,
                    "inputSchema": s.input_schema
                })
            })
            .collect();

        Ok(json!({ "tools": tools }))
    }
}

// ---------------------------------------------------------------------------
// CallToolHandlerImpl
// ---------------------------------------------------------------------------

pub struct CallToolHandlerImpl {
    service: Box<dyn McpServerService>,
}

impl CallToolHandlerImpl {
    pub fn new(service: Box<dyn McpServerService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CallToolHandler for CallToolHandlerImpl {
    async fn handle_call_tool(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                JsonRpcError::invalid_params("Missing required field: name")
            })?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
        let session_id = SessionId::new();

        let input = CallToolInput {
            session_id,
            name: name.to_string(),
            arguments,
        };

        let output = self
            .service
            .call_tool(input)
            .await
            .map_err(|e| JsonRpcError::tool_execution_failed(name, e.to_string()))?;

        let content: Vec<serde_json::Value> = output
            .content
            .into_iter()
            .map(|c| match c {
                crate::mcp_server::domain::value::ContentItem::Text { text } => {
                    json!({ "type": "text", "text": text })
                }
                crate::mcp_server::domain::value::ContentItem::Image {
                    data,
                    mime_type,
                } => json!({ "type": "image", "data": data, "mimeType": mime_type }),
                crate::mcp_server::domain::value::ContentItem::Resource {
                    uri,
                    mime_type,
                    text,
                } => json!({ "type": "resource", "resource": { "uri": uri, "mimeType": mime_type, "text": text } }),
            })
            .collect();

        Ok(json!({
            "content": content,
            "isError": output.is_error
        }))
    }
}

// ---------------------------------------------------------------------------
// ListResourcesHandlerImpl
// ---------------------------------------------------------------------------

pub struct ListResourcesHandlerImpl {
    service: Box<dyn McpServerService>,
}

impl ListResourcesHandlerImpl {
    pub fn new(service: Box<dyn McpServerService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ListResourcesHandler for ListResourcesHandlerImpl {
    async fn handle_list_resources(&self) -> Result<serde_json::Value, JsonRpcError> {
        let output = self
            .service
            .list_resources()
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        let resources: Vec<serde_json::Value> = output
            .resources
            .into_iter()
            .map(|r| {
                json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type
                })
            })
            .collect();

        Ok(json!({ "resources": resources }))
    }
}

// ---------------------------------------------------------------------------
// ReadResourceHandlerImpl
// ---------------------------------------------------------------------------

pub struct ReadResourceHandlerImpl {
    service: Box<dyn McpServerService>,
}

impl ReadResourceHandlerImpl {
    pub fn new(service: Box<dyn McpServerService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ReadResourceHandler for ReadResourceHandlerImpl {
    async fn handle_read_resource(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let uri = params.get("uri").and_then(|v| v.as_str()).ok_or_else(|| {
            JsonRpcError::invalid_params("Missing required field: uri")
        })?;

        let output = self
            .service
            .read_resource(ReadResourceInput {
                uri: uri.to_string(),
            })
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        Ok(json!({
            "contents": [{
                "uri": output.uri,
                "mimeType": output.mime_type,
                "text": output.text
            }]
        }))
    }
}

// ---------------------------------------------------------------------------
// ListPromptsHandlerImpl
// ---------------------------------------------------------------------------

pub struct ListPromptsHandlerImpl {
    service: Box<dyn McpServerService>,
}

impl ListPromptsHandlerImpl {
    pub fn new(service: Box<dyn McpServerService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ListPromptsHandler for ListPromptsHandlerImpl {
    async fn handle_list_prompts(&self) -> Result<serde_json::Value, JsonRpcError> {
        let output = self
            .service
            .list_prompts()
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        let prompts: Vec<serde_json::Value> = output
            .prompts
            .into_iter()
            .map(|p| {
                let args: Vec<serde_json::Value> = p
                    .arguments
                    .into_iter()
                    .map(|a| {
                        json!({
                            "name": a.name,
                            "description": a.description,
                            "required": a.required
                        })
                    })
                    .collect();

                json!({
                    "name": p.name,
                    "description": p.description,
                    "arguments": args
                })
            })
            .collect();

        Ok(json!({ "prompts": prompts }))
    }
}

// ---------------------------------------------------------------------------
// GetPromptHandlerImpl
// ---------------------------------------------------------------------------

pub struct GetPromptHandlerImpl {
    service: Box<dyn McpServerService>,
}

impl GetPromptHandlerImpl {
    pub fn new(service: Box<dyn McpServerService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl GetPromptHandler for GetPromptHandlerImpl {
    async fn handle_get_prompt(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
            JsonRpcError::invalid_params("Missing required field: name")
        })?;

        let output = self
            .service
            .get_prompt(GetPromptInput {
                name: name.to_string(),
                arguments: None,
            })
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        let messages: Vec<serde_json::Value> = output
            .messages
            .into_iter()
            .map(|m| {
                let content = match m.content {
                    crate::mcp_server::application::dto::PromptContentDto::Text { text } => {
                        json!({ "type": "text", "text": text })
                    }
                };
                json!({ "role": m.role, "content": content })
            })
            .collect();

        Ok(json!({
            "description": output.description,
            "messages": messages
        }))
    }
}

// ---------------------------------------------------------------------------
// CancelledHandlerImpl
// ---------------------------------------------------------------------------

pub struct CancelledHandlerImpl;

#[async_trait]
impl CancelledHandler for CancelledHandlerImpl {
    async fn handle_cancelled(
        &self,
        _params: serde_json::Value,
    ) -> Result<(), JsonRpcError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// StdioTransportHandle
// ---------------------------------------------------------------------------

pub struct StdioTransportHandle;

#[async_trait]
impl TransportHandle for StdioTransportHandle {
    async fn wait_for_completion(&self) {
        // In Phase 0, stdio runs until stdin closes
        // Implementation will use tokio::io::AsyncBufRead
    }

    async fn signal_stop(&self) {
        // Signal to stop reading from stdin
    }
}
