use super::dto::{
    AbortInput, AbortOutput, ExecuteInput, ExecuteOutput, ExecutionStatusInput,
    ExecutionStatusOutput,
};
use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;
use async_trait::async_trait;

#[async_trait]
pub trait ExecutionCommandService: Send + Sync {
    async fn new(config: CliConfig) -> Result<Self, CliError>
    where
        Self: Sized;
    async fn execute(&self, input: ExecuteInput) -> Result<ExecuteOutput, CliError>;
    async fn status(&self, input: ExecutionStatusInput) -> Result<ExecutionStatusOutput, CliError>;
    async fn abort(&self, input: AbortInput) -> Result<AbortOutput, CliError>;
}
