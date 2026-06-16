//! LogFormatter implementation.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#output
//! Implements: CLI output formatting — human-readable and JSON
//! Issue: #237
//!
//! Formats CLI output as human-readable (colorized terminal text) or
//! JSON for CI/CD integration.

use async_trait::async_trait;

use crate::application::dto::{
    AuditDiffOutput, AuditListOutput, AuditShowOutput, GenerateOutput, HistoryListOutput,
    HistoryShowOutput, InitOutput, LogsOutput, PlanOutput, RunOutput, TemplateListOutput,
    TemplateShowOutput,
};
use crate::domain::config::OutputFormat;
use crate::domain::error::CliError;
use crate::infrastructure::output::LogFormatter;

/// Formats CLI output for terminal or JSON consumption.
pub struct LogFormatterImpl {
    format: OutputFormat,
}

impl LogFormatterImpl {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Render a key-value pair for human-readable output.
    fn kv(key: &str, value: impl std::fmt::Display) -> String {
        format!("  {}: {}", key, value)
    }

    /// Render a section header for human-readable output.
    fn header(text: &str) -> String {
        format!("\n─── {} ───", text)
    }

    /// Render a separator line.
    fn separator() -> String {
        "─".repeat(50)
    }
}

#[async_trait]
impl LogFormatter for LogFormatterImpl {
    fn output_format(&self) -> OutputFormat {
        self.format
    }

    async fn format_run(&self, output: &RunOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(format!("{}", output.outcome)),
            OutputFormat::Pretty => {
                let lines = vec![
                    Self::separator(),
                    format!("  Session: {}", output.session_id),
                    format!("  Outcome: {}", output.outcome),
                    Self::header("Execution Summary"),
                    Self::kv("Total nodes", output.summary.total_nodes),
                    Self::kv("Completed", output.summary.completed),
                    Self::kv("Failed", output.summary.failed),
                    Self::kv("Skipped", output.summary.skipped),
                    Self::kv(
                        "Duration",
                        format!("{}ms", output.summary.total_duration_ms),
                    ),
                    Self::separator(),
                ];
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_plan(&self, output: &PlanOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(format!(
                "Plan: {} (confidence: {:.1}%)",
                output.template_name,
                output.confidence * 100.0
            )),
            OutputFormat::Pretty => {
                let mut lines = vec![
                    Self::separator(),
                    format!(
                        "  Template: {} ({})",
                        output.template_name, output.template_id
                    ),
                    format!("  Confidence: {:.1}%", output.confidence * 100.0),
                    format!(
                        "  Budget: ~{} tokens / ~{} calls",
                        output.total_estimated_tokens, output.total_estimated_calls
                    ),
                    Self::header("Execution Plan"),
                ];
                for node in &output.nodes {
                    lines.push(format!("  ◇ {} ({})", node.label, node.tool));
                    if !node.depends_on.is_empty() {
                        lines.push(format!("    depends on: {}", node.depends_on.join(", ")));
                    }
                }
                lines.push(Self::separator());
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_generate(&self, output: &GenerateOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(output.template_id.clone()),
            OutputFormat::Pretty => {
                let mut lines = vec![
                    Self::separator(),
                    format!("  Template ID: {}", output.template_id),
                ];
                if let Some(path) = &output.saved_path {
                    lines.push(format!("  Saved to: {}", path));
                }
                if output.persisted {
                    lines.push("  Status: persisted".into());
                }
                lines.push(Self::separator());
                if !output.content.is_empty() {
                    lines.push(String::new());
                    lines.push(output.content.clone());
                }
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_init(&self, output: &InitOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(output.created_path.clone()),
            OutputFormat::Pretty => {
                let mut lines = vec![
                    Self::separator(),
                    format!("  Created: {}", output.created_path),
                    Self::header("Files Created"),
                ];
                for file in &output.files_created {
                    lines.push(format!("  ✓ {}", file));
                }
                lines.push(Self::separator());
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_history_list(&self, output: &HistoryListOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(format!("{} sessions", output.total)),
            OutputFormat::Pretty => {
                if output.sessions.is_empty() {
                    return Ok("No past sessions found.".into());
                }
                let mut lines = vec![Self::separator()];
                for session in &output.sessions {
                    lines.push(format!(
                        "  ◇ {} — {} ({}) [{}ms]",
                        session.session_id, session.command, session.outcome, session.duration_ms
                    ));
                }
                lines.push(format!("\n  Total: {} sessions", output.total));
                lines.push(Self::separator());
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_history_show(&self, output: &HistoryShowOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(output.session.session_id.clone()),
            OutputFormat::Pretty => {
                let mut lines = vec![
                    Self::separator(),
                    format!("  Session: {}", output.session.session_id),
                    format!("  Command: {}", output.session.command),
                    format!("  Outcome: {}", output.session.outcome),
                    format!("  Duration: {}ms", output.session.duration_ms),
                    Self::header("Nodes"),
                ];
                for node in &output.nodes {
                    let status = if node.success { "✓" } else { "✗" };
                    lines.push(format!(
                        "  {} {} ({}ms)",
                        status, node.label, node.duration_ms
                    ));
                    if let Some(ref error) = node.error {
                        lines.push(format!("    error: {}", error));
                    }
                }
                lines.push(Self::separator());
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_logs(&self, output: &LogsOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(format!("{} entries", output.total)),
            OutputFormat::Pretty => {
                let mut lines = vec![Self::separator()];
                for entry in &output.entries {
                    lines.push(format!(
                        "  [{}] {}: {}",
                        entry.severity, entry.event_type, entry.message
                    ));
                }
                lines.push(format!("\n  Total: {} entries", output.total));
                lines.push(Self::separator());
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_audit_list(&self, output: &AuditListOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(format!("{} audits", output.total)),
            OutputFormat::Pretty => {
                if output.audits.is_empty() {
                    return Ok("No audit envelopes found.".into());
                }
                let mut lines = vec![Self::separator()];
                for audit in &output.audits {
                    lines.push(format!(
                        "  ◇ {} — session: {}",
                        audit.audit_id, audit.session_id
                    ));
                }
                lines.push(format!("\n  Total: {} audits", output.total));
                lines.push(Self::separator());
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_audit_show(&self, output: &AuditShowOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(output.audit.audit_id.clone()),
            OutputFormat::Pretty => Ok([
                Self::separator(),
                format!("  Audit ID: {}", output.audit.audit_id),
                format!("  Session: {}", output.audit.session_id),
                format!("  Planning Hash: {}", output.audit.planning_hash),
                format!("  Events: {}", output.events.len()),
                Self::separator(),
            ]
            .join("\n")),
        }
    }

    async fn format_audit_diff(&self, output: &AuditDiffOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(if output.identical {
                "identical"
            } else {
                "different"
            }
            .into()),
            OutputFormat::Pretty => {
                let identical = if output.identical {
                    "identical"
                } else {
                    "different"
                };
                Ok(Self::separator()
                    + "\n"
                    + &format!("  Plans are: {}\n", identical)
                    + &format!("  Hash 1: {}\n", output.planning_hash_1)
                    + &format!("  Hash 2: {}\n", output.planning_hash_2)
                    + &format!("  Diff: {}\n", output.diff_description)
                    + &Self::separator())
            }
        }
    }

    async fn format_template_list(&self, output: &TemplateListOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(format!("{} templates", output.total)),
            OutputFormat::Pretty => {
                if output.templates.is_empty() {
                    return Ok("No templates registered.".into());
                }
                let mut lines = vec![Self::separator()];
                for tmpl in &output.templates {
                    let badge = if tmpl.built_in {
                        "[built-in]"
                    } else {
                        "[user]"
                    };
                    lines.push(format!("  {} {} — {}", badge, tmpl.id, tmpl.name));
                    lines.push(format!("    {}", tmpl.description));
                }
                lines.push(format!("\n  Total: {} templates", output.total));
                lines.push(Self::separator());
                Ok(lines.join("\n"))
            }
        }
    }

    async fn format_template_show(&self, output: &TemplateShowOutput) -> Result<String, CliError> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(output).map_err(|e| CliError::OutputRenderError {
                    detail: e.to_string(),
                })
            }
            OutputFormat::Quiet => Ok(output.content.clone()),
            OutputFormat::Pretty => Ok(output.content.clone()),
        }
    }

    async fn format_error(&self, error: &CliError) -> String {
        match self.format {
            OutputFormat::Json => {
                let error_obj = serde_json::json!({
                    "error": {
                        "code": error.exit_code(),
                        "message": error.to_string(),
                        "retriable": error.is_retriable(),
                    }
                });
                serde_json::to_string_pretty(&error_obj).unwrap_or_else(|_| error.to_string())
            }
            OutputFormat::Quiet => error.to_string(),
            OutputFormat::Pretty => {
                format!(
                    "{}[Error] {}\n{}Hint: {}",
                    "  ",
                    error,
                    "  ",
                    error_hint(error)
                )
            }
        }
    }
}

/// Provide a user-friendly hint for common CLI errors.
fn error_hint(error: &CliError) -> &str {
    match error {
        CliError::ConfigNotFound { .. } => {
            "Run `rigorix init` to create a config file, or use --config to specify a path"
        }
        CliError::ConfigParseError { .. } => "Check that your rigorix.toml is valid TOML syntax",
        CliError::MissingConfig { field, .. } => {
            if field == "api_key" {
                "Set RIGORIX_API_KEY env var or add api_key to rigorix.toml"
            } else {
                "Check your configuration file for the required field"
            }
        }
        CliError::UnknownCommand { suggestions, .. } => {
            if suggestions.is_empty() {
                "Run `rigorix --help` to see available commands"
            } else {
                "Did you mean one of the suggested commands?"
            }
        }
        CliError::InvalidArguments { .. } => "Use `rigorix <command> --help` for usage information",
        CliError::MissingArgument { command: _, .. } => {
            // Return static string to avoid allocation
            "Use `rigorix <command> --help` for usage information"
        }
        _ => "Run `rigorix --help` for usage information",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::dto::ExecutionSummary;

    #[test]
    fn test_new_with_format() {
        let formatter = LogFormatterImpl::new(OutputFormat::Pretty);
        assert_eq!(formatter.output_format(), OutputFormat::Pretty);
    }

    #[test]
    fn test_kv_format() {
        let result = LogFormatterImpl::kv("key", "value");
        assert_eq!(result, "  key: value");
    }

    #[test]
    fn test_header_format() {
        let result = LogFormatterImpl::header("Test");
        assert!(result.contains("Test"));
    }

    #[test]
    fn test_quiet_format_run() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let formatter = LogFormatterImpl::new(OutputFormat::Quiet);
        let output = RunOutput {
            session_id: "test".into(),
            outcome: crate::domain::event::SessionOutcome::Completed,
            summary: ExecutionSummary {
                total_nodes: 3,
                completed: 3,
                failed: 0,
                skipped: 0,
                total_duration_ms: 100,
            },
        };
        let result = rt.block_on(formatter.format_run(&output)).unwrap();
        assert_eq!(result, "completed");
    }

    #[test]
    fn test_error_hint_config_not_found() {
        let err = CliError::ConfigNotFound {
            detail: "test".into(),
        };
        let hint = error_hint(&err);
        assert!(hint.contains("rigorix init"));
    }

    #[test]
    fn test_error_hint_missing_api_key() {
        let err = CliError::MissingConfig {
            field: "api_key".into(),
            hint: "set key".into(),
        };
        let hint = error_hint(&err);
        assert!(hint.contains("RIGORIX_API_KEY"));
    }
}
