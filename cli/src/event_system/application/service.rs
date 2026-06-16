use super::dto::{ListEventsOutput, PublishInput, PublishOutput, SubscribeInput, SubscribeOutput};
use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;
use async_trait::async_trait;

#[async_trait]
pub trait EventSystemCommandService: Send + Sync {
    async fn new(config: CliConfig) -> Result<Self, CliError>
    where
        Self: Sized;
    async fn subscribe(&self, input: SubscribeInput) -> Result<SubscribeOutput, CliError>;
    async fn publish(&self, input: PublishInput) -> Result<PublishOutput, CliError>;
    async fn list_events(&self) -> Result<ListEventsOutput, CliError>;
}
