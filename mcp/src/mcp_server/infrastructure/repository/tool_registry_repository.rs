//! Repository interface for the ToolRegistry aggregate.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#tool-registry-repository
//! Implements: Contract Freeze — ToolRegistryRepository trait
//!
//! Abstracts ToolRegistry aggregate persistence.
//! Like the McpServer, the ToolRegistry is in-memory in Phase 0
//! (see ADR-002), but this interface exists for testability.
//!
//! # Contract (Frozen)
//!
//! - Read operations return aggregate or error
//! - Write operations persist aggregate state
//! - All methods are async
//! - All errors return McpServerError

use async_trait::async_trait;
use uuid::Uuid;

use crate::mcp_server::domain::entity::ToolRegistry;
use crate::mcp_server::domain::error::McpServerError;

/// Repository for ToolRegistry aggregate persistence.
///
/// # Contract (Frozen)
///
/// - `find_by_id` returns `None` if no registry with the given ID exists
/// - `save` persists the full aggregate state
/// - `delete` removes the aggregate from storage
/// - Implementations MUST be thread-safe (Send + Sync)
#[async_trait]
pub trait ToolRegistryRepository: Send + Sync {
    /// Find a ToolRegistry by its unique ID.
    async fn find_by_id(&self, id: &Uuid) -> Result<Option<ToolRegistry>, McpServerError>;

    /// Save (create or update) a ToolRegistry aggregate.
    async fn save(&self, tool_registry: &ToolRegistry) -> Result<(), McpServerError>;

    /// Delete a ToolRegistry aggregate by ID.
    async fn delete(&self, id: &Uuid) -> Result<(), McpServerError>;
}
