//! Repository interface for Session entities.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#session-repository
//! Implements: Contract Freeze — SessionRepository trait
//!
//! Abstracts Session entity persistence and lookup.
//! Sessions are created and destroyed within the MCP Server lifecycle
//! and are fully in-memory in Phase 0 (see ADR-002).
//!
//! # Contract (Frozen)
//!
//! - Read operations return session or error
//! - Write operations persist session state
//! - All methods are async
//! - All errors return McpServerError

use async_trait::async_trait;

use crate::mcp_server::domain::entity::Session;
use crate::mcp_server::domain::error::McpServerError;
use crate::mcp_server::domain::value::SessionId;

/// Repository for Session entity persistence and lookup.
///
/// # Contract (Frozen)
///
/// - `find_by_id` returns `None` if no session with the given ID exists
/// - `save` persists a session (create or update)
/// - `delete` removes a session from storage
/// - `list_active` returns all sessions that are not in a terminal state
/// - `count` returns the total number of sessions
/// - Implementations MUST be thread-safe (Send + Sync)
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Find a session by its session ID.
    async fn find_by_id(&self, id: &SessionId) -> Result<Option<Session>, McpServerError>;

    /// Save (create or update) a session.
    async fn save(&self, session: &Session) -> Result<(), McpServerError>;

    /// Delete a session by its session ID.
    async fn delete(&self, id: &SessionId) -> Result<(), McpServerError>;

    /// List all active sessions (non-terminal state).
    async fn list_active(&self) -> Result<Vec<Session>, McpServerError>;

    /// Return the total number of sessions.
    async fn count(&self) -> Result<usize, McpServerError>;
}
