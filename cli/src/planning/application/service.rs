//! Service interfaces for the CLI Planning module.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanCommandService trait
//! Issue: issue-contract-freeze

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

use super::dto::{ClassifyInput, ClassifyOutput, PlanInput, PlanOutput};

#[async_trait]
pub trait PlanCommandService: Send + Sync {
    async fn new(config: CliConfig) -> Result<Self, CliError>
    where
        Self: Sized;

    async fn plan(&self, input: PlanInput) -> Result<PlanOutput, CliError>;

    async fn classify(&self, input: ClassifyInput) -> Result<ClassifyOutput, CliError>;
}
