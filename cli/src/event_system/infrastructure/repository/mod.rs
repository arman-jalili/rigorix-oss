use crate::event_system::domain::EventSystemCliError;
use async_trait::async_trait;

#[async_trait]
pub trait EventSystemRepository: Send + Sync {
    async fn store_event(
        &self,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<(), EventSystemCliError>;
    async fn list_event_types(&self) -> Result<Vec<String>, EventSystemCliError>;
    async fn clear(&self) -> Result<(), EventSystemCliError>;
}
