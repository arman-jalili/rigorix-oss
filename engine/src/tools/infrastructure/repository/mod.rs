//! Repository interfaces for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md#repository
//! Implements: Contract Freeze — ToolRepository trait
//! Issue: #124
//!
//! Repositories abstract tool data storage and retrieval behind interfaces,
//! allowing implementations to use filesystem, database, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::tools::domain::ToolError;

/// Repository for tool metadata and storage.
///
/// Abstracts the source of tool definitions — whether from filesystem
/// tool scripts, configuration files, or remote registries.
///
/// # Contract (Frozen)
/// - Read operations return tool metadata for registry initialization
/// - Write operations persist tool execution history or configuration
/// - Implementations MUST validate paths against directory traversal
/// - History records are append-only (immutable after creation)
#[async_trait]
pub trait ToolRepository: Send + Sync {
    /// Load tool metadata from storage.
    ///
    /// Returns a list of tool names that should be registered.
    /// Returns an empty vec if no tools are configured.
    async fn load_tool_names(&self) -> Result<Vec<String>, ToolError>;

    /// Save a tool execution record for audit/history.
    ///
    /// Records the tool name, execution parameters, result, and timestamp.
    /// The record ID is returned for correlation.
    async fn record_execution(
        &self,
        tool_name: &str,
        execution_id: &uuid::Uuid,
        output: &crate::tools::application::dto::ToolResult,
    ) -> Result<String, ToolError>;

    /// Retrieve execution history for a tool.
    ///
    /// Returns the most recent `limit` execution records for the given tool.
    /// Returns an empty vec if no history exists or the tool is unknown.
    async fn get_execution_history(
        &self,
        tool_name: &str,
        limit: usize,
    ) -> Result<Vec<crate::tools::application::dto::ToolResult>, ToolError>;

    /// Save tool configuration.
    ///
    /// Persists tool-specific configuration (allowlists, timeouts, etc.).
    async fn save_tool_config(
        &self,
        tool_name: &str,
        config: &serde_json::Value,
    ) -> Result<(), ToolError>;

    /// Load tool configuration.
    ///
    /// Returns the saved configuration for a tool, or `None` if none exists.
    async fn load_tool_config(
        &self,
        tool_name: &str,
    ) -> Result<Option<serde_json::Value>, ToolError>;
}
