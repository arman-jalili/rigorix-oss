use crate::state_persistence::domain::StatePersistenceCliError;
use async_trait::async_trait;

#[async_trait]
pub trait StatePersistenceRepository: Send + Sync {
    async fn store_state(
        &self,
        session_id: &str,
        data: serde_json::Value,
    ) -> Result<(), StatePersistenceCliError>;
    async fn load_state(
        &self,
        session_id: &str,
    ) -> Result<Option<serde_json::Value>, StatePersistenceCliError>;
    async fn list_sessions(&self) -> Result<Vec<String>, StatePersistenceCliError>;
    async fn delete_state(&self, session_id: &str) -> Result<bool, StatePersistenceCliError>;
    async fn clear(&self) -> Result<(), StatePersistenceCliError>;
}
