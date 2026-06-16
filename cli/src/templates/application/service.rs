//! Service interfaces for the CLI Templates module.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — TemplateCommandService trait
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for CLI template
//! commands (list/show). All methods are async and return domain error types.
//! Implementations reside in the infrastructure layer.
//!
//! # Contract (Frozen)
//! - Every template CLI command has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

use super::dto::{TemplateListOutput, TemplateShowOutput};

/// Service trait for CLI template commands.
///
/// Handles `rigorix template list` and `rigorix template show <id>`.
/// Wraps the engine's `TemplateEngineService` for CLI consumption.
///
/// # Contract (Frozen)
/// - `list()` returns all registered templates with summary metadata
/// - `show()` returns the full TOML content for a specific template
/// - `new()` creates a fully initialized service instance
/// - All methods return `CliError` for boundary-level error handling
#[async_trait]
pub trait TemplateCommandService: Send + Sync {
    /// Create a new service instance.
    ///
    /// Initializes the underlying engine's template system (loads built-ins,
    /// scans template directories, registers all found templates).
    /// Returns `CliError::Internal` if initialization fails.
    async fn new(config: CliConfig) -> Result<Self, CliError>
    where
        Self: Sized;

    /// List all registered templates.
    ///
    /// Returns summary metadata for every registered template.
    /// Returns an empty list if no templates are registered.
    async fn list(&self) -> Result<TemplateListOutput, CliError>;

    /// Show a specific template by ID.
    ///
    /// Returns the full TOML content of the template as a formatted string.
    /// Returns `CliError::Engine(TemplateError::NotFound)` if the template
    /// ID doesn't match any registered template.
    async fn show(&self, template_id: &str) -> Result<TemplateShowOutput, CliError>;
}
