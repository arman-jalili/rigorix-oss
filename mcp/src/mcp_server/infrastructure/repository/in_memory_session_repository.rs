//! In-memory implementation of SessionRepository.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#session-repository
//! Implements: SessionRepository

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::mcp_server::domain::entity::Session;
use crate::mcp_server::domain::error::McpServerError;
use crate::mcp_server::domain::value::SessionId;

use super::SessionRepository;

/// Thread-safe in-memory repository for Session entities.
pub struct InMemorySessionRepository {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
}

impl InMemorySessionRepository {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemorySessionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    async fn find_by_id(&self, id: &SessionId) -> Result<Option<Session>, McpServerError> {
        let guard = self
            .sessions
            .read()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        Ok(guard.get(id).cloned())
    }

    async fn save(&self, session: &Session) -> Result<(), McpServerError> {
        let mut guard = self
            .sessions
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        guard.insert(session.id, session.clone());
        Ok(())
    }

    async fn delete(&self, id: &SessionId) -> Result<(), McpServerError> {
        let mut guard = self
            .sessions
            .write()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        guard.remove(id);
        Ok(())
    }

    async fn list_active(&self) -> Result<Vec<Session>, McpServerError> {
        let guard = self
            .sessions
            .read()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        Ok(guard
            .values()
            .filter(|s| !s.status.is_terminal())
            .cloned()
            .collect())
    }

    async fn count(&self) -> Result<usize, McpServerError> {
        let guard = self
            .sessions
            .read()
            .map_err(|e| McpServerError::Internal(format!("Lock poisoned: {}", e)))?;
        Ok(guard.len())
    }
}
