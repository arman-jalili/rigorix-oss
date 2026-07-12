//! In-memory implementation of McpServerRepository.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#mcp-server-repository
//! Implements: McpServerRepository
//!
//! Thread-safe in-memory storage using Arc<RwLock<HashMap>>.
//! Used in Phase 0 per ADR-002 (no database needed).

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::mcp_server::domain::entity::McpServer;
use crate::mcp_server::domain::error::McpServerError;

use super::McpServerRepository;

/// Thread-safe in-memory repository for McpServer aggregates.
pub struct InMemoryMcpServerRepository {
    servers: Arc<RwLock<HashMap<Uuid, McpServer>>>,
}

impl InMemoryMcpServerRepository {
    /// Create a new empty in-memory repository.
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryMcpServerRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpServerRepository for InMemoryMcpServerRepository {
    async fn find_by_id(&self, id: &Uuid) -> Result<Option<McpServer>, McpServerError> {
        let guard = self
            .servers
            .read()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        Ok(guard.get(id).cloned())
    }

    async fn save(&self, server: &McpServer) -> Result<(), McpServerError> {
        let mut guard = self
            .servers
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        guard.insert(server.id(), server.clone());
        Ok(())
    }

    async fn delete(&self, id: &Uuid) -> Result<(), McpServerError> {
        let mut guard = self
            .servers
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        guard.remove(id);
        Ok(())
    }
}
