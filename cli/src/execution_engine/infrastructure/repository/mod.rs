use crate::execution_engine::domain::ExecutionCliError;
use async_trait::async_trait;

#[async_trait]
pub trait ExecutionRepository: Send + Sync {
    async fn store_execution(
        &self,
        execution_id: &str,
        success: bool,
    ) -> Result<(), ExecutionCliError>;
    async fn get_execution(&self, execution_id: &str) -> Result<Option<bool>, ExecutionCliError>;
    async fn list_executions(&self, limit: usize)
    -> Result<Vec<(String, bool)>, ExecutionCliError>;
    async fn clear(&self) -> Result<(), ExecutionCliError>;
}
