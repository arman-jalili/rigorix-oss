//! Repository interface for the McpServer aggregate.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#mcp-server-repository
//! Implements: Contract Freeze — McpServerRepository trait
//!
//! Abstracts McpServer aggregate persistence.
//! The MCP Server is in-memory (see ADR-002), so the repository
//! primarily supports lookup and state queries rather than full CRUD.
//!
//! # Contract (Frozen)
//!
//! - Read operations return aggregate or error
//! - Write operations persist aggregate state
//! - All methods are async
//! - All errors return McpServerError

use async_trait::async_trait;
use uuid::Uuid;

use crate::mcp_server::domain::entity::McpServer;
use crate::mcp_server::domain::error::McpServerError;

/// Repository for McpServer aggregate persistence.
///
/// In Phase 0, the McpServer is fully in-memory (see ADR-002).
/// This interface exists for testability and future persistence needs.
///
/// # Contract (Frozen)
///
/// - `find_by_id` returns `None` if no server with the given ID exists
/// - `save` persists the full aggregate state
/// - `delete` removes the aggregate from storage
/// - Implementations MUST be thread-safe (Send + Sync)
#[async_trait]
pub trait McpServerRepository: Send + Sync {
    /// Find an MCP Server by its unique ID.
    ///
    /// Returns `Ok(None)` if no server with this ID exists.
    async fn find_by_id(&self, id: &Uuid) -> Result<Option<McpServer>, McpServerError>;

    /// Save (create or update) an MCP Server aggregate.
    ///
    /// Implementations should upsert — create if not exists, update if exists.
    async fn save(&self, server: &McpServer) -> Result<(), McpServerError>;

    /// Delete an MCP Server aggregate by ID.
    async fn delete(&self, id: &Uuid) -> Result<(), McpServerError>;
}
