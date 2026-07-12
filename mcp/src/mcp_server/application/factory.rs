//! Factory interfaces for constructing MCP Server domain objects.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#factories
//! Implements: Contract Freeze — McpServerFactory, ToolSchemaFactory interfaces
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//!
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations
//! - Factory methods are async where construction involves I/O

use async_trait::async_trait;

use crate::mcp_server::domain::value::{
    PromptContent, PromptMessage, PromptMessageContent, PromptRole, PromptSchema, PromptArgument,
    ResourceSchema, ServerCapabilities, ServerConfig, ToolSchema,
};

// ---------------------------------------------------------------------------
// McpServerFactory
// ---------------------------------------------------------------------------

/// Factory for constructing McpServer components with validation.
///
/// Implementations handle constructing the McpServer aggregate,
/// creating default configurations, and building capability objects.
#[async_trait]
pub trait McpServerFactory: Send + Sync {
    /// Create a default `ServerConfig`.
    fn default_config(&self) -> ServerConfig;

    /// Build `ServerCapabilities` for the initialize response.
    ///
    /// Takes current tool count, resource count, and prompt count
    /// and produces the capability object to advertise to clients.
    fn build_server_capabilities(
        &self,
        tool_count: usize,
        resource_count: usize,
        prompt_count: usize,
        enterprise_enabled: bool,
    ) -> ServerCapabilities;
}

// ---------------------------------------------------------------------------
// ToolSchemaFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `ToolSchema` objects.
///
/// Handles creating tool schemas with proper naming conventions,
/// input schema generation, and description formatting.
#[async_trait]
pub trait ToolSchemaFactory: Send + Sync {
    /// Build a `ToolSchema` for a tool with the given name and description.
    ///
    /// Automatically constructs a JSON Schema with the given parameters.
    /// The `parameters` map defines each parameter's type and description.
    fn build_tool_schema(
        &self,
        name: &str,
        description: &str,
        parameters: Vec<ToolParameterDef>,
    ) -> ToolSchema;
}

/// Definition of a tool parameter for schema construction.
#[derive(Debug, Clone)]
pub struct ToolParameterDef {
    /// Parameter name.
    pub name: String,

    /// Parameter description.
    pub description: String,

    /// JSON Schema type (e.g., "string", "number", "boolean", "object", "array").
    pub param_type: String,

    /// Whether this parameter is required.
    pub required: bool,
}

// ---------------------------------------------------------------------------
// ResourceSchemaFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `ResourceSchema` objects.
///
/// Handles creating resource schemas with proper URI patterns
/// and MIME type configuration.
#[async_trait]
pub trait ResourceSchemaFactory: Send + Sync {
    /// Build a `ResourceSchema` for a resource URI template.
    fn build_resource_schema(
        &self,
        uri: &str,
        name: &str,
        description: &str,
        mime_type: &str,
    ) -> ResourceSchema;
}

// ---------------------------------------------------------------------------
// PromptSchemaFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `PromptSchema` objects.
///
/// Handles creating prompt schemas, argument definitions,
/// and building prompt content/messages.
#[async_trait]
pub trait PromptSchemaFactory: Send + Sync {
    /// Build a `PromptSchema` for a prompt template.
    fn build_prompt_schema(
        &self,
        name: &str,
        description: &str,
        arguments: Vec<PromptArgument>,
    ) -> PromptSchema;

    /// Build `PromptContent` from messages.
    fn build_prompt_content(
        &self,
        description: Option<String>,
        messages: Vec<(PromptRole, String)>,
    ) -> PromptContent;

    /// Build a single `PromptMessage` from role and text.
    fn build_prompt_message(&self, role: PromptRole, text: String) -> PromptMessage {
        PromptMessage {
            role,
            content: PromptMessageContent::text(text),
        }
    }
}
