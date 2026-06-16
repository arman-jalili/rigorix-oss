//! Template command service trait.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — TemplateCommandService trait
//! Issue: Phase 2.1
//!
//! CLI-side interface for template operations, wrapping the engine's
//! TemplateEngineService. Trait in infrastructure, impl in _impl file.

use async_trait::async_trait;

use crate::cli_boundary::application::dto::{TemplateListOutput, TemplateShowOutput};
use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

/// Service trait for template CLI commands.
#[async_trait]
pub trait TemplateCommandService: Send + Sync {
    /// Create a new service instance.
    async fn new(config: CliConfig) -> Result<Self, CliError>
    where
        Self: Sized;

    /// List all registered templates.
    async fn list(&self) -> Result<TemplateListOutput, CliError>;

    /// Show a specific template by ID.
    async fn show(&self, template_id: &str) -> Result<TemplateShowOutput, CliError>;
}
