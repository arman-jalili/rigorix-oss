//! Service interfaces for the CLI Template Generation module.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — GenerateCommandService trait
//! Issue: issue-contract-freeze
//!
//! Wraps the engine's TemplateGenerator for CLI consumption.

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

use super::dto::{
    CostEstimateInput, CostEstimateOutput, DryRunInput, DryRunOutput, GenerateInput, GenerateOutput,
};

#[async_trait]
pub trait GenerateCommandService: Send + Sync {
    async fn new(config: CliConfig) -> Result<Self, CliError>
    where
        Self: Sized;

    async fn generate(&self, input: GenerateInput) -> Result<GenerateOutput, CliError>;

    async fn dry_run(&self, input: DryRunInput) -> Result<DryRunOutput, CliError>;

    async fn estimate_cost(&self, input: CostEstimateInput)
    -> Result<CostEstimateOutput, CliError>;
}
