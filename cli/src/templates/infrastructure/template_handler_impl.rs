//! TemplateCommandService implementation.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: TemplateCommandService trait — wraps engine's TemplateEngineService
//! Issue: Phase 2.1
//!
//! Bridges the CLI's TemplateCommandService trait to the engine's
//! TemplateEngineService. Handles template listing and inspection.

use async_trait::async_trait;
use rigorix_engine::templates::application::{
    GetTemplateInput, TemplateEngineImpl, TemplateEngineService,
};
use tracing::info;

use crate::cli_boundary::application::dto::{
    TemplateListOutput, TemplateShowOutput, TemplateSummary as CliTemplateSummary,
};
use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;
use crate::templates::infrastructure::service::TemplateCommandService;

/// Implementation of `TemplateCommandService` backed by the engine's
/// `TemplateEngineImpl`.
pub struct TemplateEngineHandler {
    engine: TemplateEngineImpl,
    _config: CliConfig,
}

#[async_trait]
impl TemplateCommandService for TemplateEngineHandler {
    async fn new(config: CliConfig) -> Result<Self, CliError> {
        let engine = TemplateEngineImpl::new();
        info!("Template engine handler initialized");
        Ok(Self {
            engine,
            _config: config,
        })
    }

    async fn list(&self) -> Result<TemplateListOutput, CliError> {
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

    async fn show(&self, template_id: &str) -> Result<TemplateShowOutput, CliError> {
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
