//! Template list/show command handler.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: `rigorix template list` and `rigorix template show`
//! Issue: Phase 2.1
//!
//! Wraps the engine's TemplateEngineService for CLI consumption.
//! The engine handles built-in and user template registration;
//! this handler just queries it for display.

use rigorix_engine::templates::application::{
    GetTemplateInput, TemplateEngineImpl, TemplateEngineService,
};
use tracing::info;

use crate::cli_boundary::application::dto::{
    TemplateListOutput, TemplateShowOutput, TemplateSummary as CliTemplateSummary,
};
use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

/// Handles the `rigorix template` command group.
///
/// Wraps the engine's TemplateEngineService to provide template
/// listing and inspection from the CLI.
pub struct TemplateCommandHandler {
    engine: TemplateEngineImpl,
    _config: CliConfig,
}

impl TemplateCommandHandler {
    /// Create a new handler with an empty template engine.
    ///
    /// Templates are loaded lazily by the engine on first access
    /// or registered via the engine's own initialization flow.
    pub async fn new(config: CliConfig) -> Result<Self, CliError> {
        let engine = TemplateEngineImpl::new();
        info!("Template command handler initialized");
        Ok(Self {
            engine,
            _config: config,
        })
    }

    /// List all registered templates.
    ///
    /// Returns all built-in and user templates registered in the engine.
    /// Maps the engine's `TemplateSummary` to the CLI's `TemplateSummary`.
    pub async fn list(&self) -> Result<TemplateListOutput, CliError> {
        let output = self
            .engine
            .list_templates()
            .await
            .map_err(|e| CliError::Internal {
                detail: format!("Failed to list templates: {}", e),
            })?;

        let summaries: Vec<CliTemplateSummary> = output
            .templates
            .into_iter()
            .map(|t| CliTemplateSummary {
                id: t.id,
                name: t.name,
                description: t.description,
                built_in: t.is_builtin,
            })
            .collect();

        Ok(TemplateListOutput {
            total: summaries.len() as u32,
            templates: summaries,
        })
    }

    /// Show a specific template's metadata by ID.
    ///
    /// Returns the template summary as TOML-formatted content.
    /// If the template is not found, returns a clear error.
    pub async fn show(&self, template_id: &str) -> Result<TemplateShowOutput, CliError> {
        let input = GetTemplateInput {
            template_id: template_id.to_string(),
        };

        let maybe_summary =
            self.engine
                .get_template(input)
                .await
                .map_err(|e| CliError::Internal {
                    detail: format!("Failed to get template '{}': {}", template_id, e),
                })?;

        match maybe_summary {
            Some(summary) => {
                let content = format!(
                    r#"# Template: {name}
# ID: {id}
# Version: {version}
# Built-in: {is_builtin}

[meta]
id = "{id}"
name = "{name}"
description = "{description}"
version = "{version}"
is_builtin = {is_builtin}

parameters = {param_count}
nodes = {node_count}
"#,
                    id = summary.id,
                    name = summary.name,
                    description = summary.description,
                    version = summary.version,
                    is_builtin = summary.is_builtin,
                    param_count = summary.param_count,
                    node_count = summary.node_count,
                );

                Ok(TemplateShowOutput { content })
            }
            None => Err(CliError::MissingArgument {
                command: "template show".into(),
                argument: format!("template_id '{}' not found", template_id),
            }),
        }
    }
}
