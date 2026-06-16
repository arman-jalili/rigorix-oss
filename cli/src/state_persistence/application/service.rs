use super::dto::{
    DeleteStateInput, DeleteStateOutput, ListStatesOutput, LoadStateInput, LoadStateOutput,
};
use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;
use async_trait::async_trait;

#[async_trait]
pub trait StatePersistenceCommandService: Send + Sync {
    async fn new(config: CliConfig) -> Result<Self, CliError>
    where
        Self: Sized;
    async fn load(&self, input: LoadStateInput) -> Result<LoadStateOutput, CliError>;
    async fn list(&self) -> Result<ListStatesOutput, CliError>;
    async fn delete(&self, input: DeleteStateInput) -> Result<DeleteStateOutput, CliError>;
}
