//! In-memory implementation of ToolRegistryRepository.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#tool-registry-repository
//! Implements: ToolRegistryRepository

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::mcp_server::domain::entity::ToolRegistry;
use crate::mcp_server::domain::error::McpServerError;

use super::ToolRegistryRepository;

/// Thread-safe in-memory repository for ToolRegistry aggregates.
pub struct InMemoryToolRegistryRepository {
    registries: Arc<RwLock<HashMap<Uuid, ToolRegistry>>>,
}

impl InMemoryToolRegistryRepository {
    pub fn new() -> Self {
        Self {
            registries: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryToolRegistryRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistryRepository for InMemoryToolRegistryRepository {
    async fn find_by_id(&self, id: &Uuid) -> Result<Option<ToolRegistry>, McpServerError> {
        let guard = self
            .registries
            .read()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        Ok(guard.get(id).cloned())
    }

    async fn save(&self, registry: &ToolRegistry) -> Result<(), McpServerError> {
        let mut guard = self
            .registries
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        guard.insert(registry.id(), registry.clone());
        Ok(())
    }

    async fn delete(&self, id: &Uuid) -> Result<(), McpServerError> {
        let mut guard = self
            .registries
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        guard.remove(id);
        Ok(())
    }
}
