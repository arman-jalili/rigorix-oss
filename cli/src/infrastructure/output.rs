//! Output formatting interface for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — LogFormatter trait
//! Issue: issue-contract-freeze
//!
//! Formats CLI output as human-readable or JSON for CI/CD integration.
//!
//! # Contract (Frozen)
//! - Every CLI output type has a corresponding `format_*` method
//! - JSON output is always valid JSON (for CI/CD consumers)
//! - Human-readable output uses color and formatting when connected to a TTY
//! - `format_error` produces consistent error output with code, message, hint

use async_trait::async_trait;

use crate::application::dto::{
    AuditDiffOutput, AuditListOutput, AuditShowOutput, GenerateOutput, HistoryListOutput,
    HistoryShowOutput, InitOutput, LogsOutput, PlanOutput, RunOutput, TemplateListOutput,
    TemplateShowOutput,
};
use crate::domain::config::OutputFormat;
use crate::domain::error::CliError;

/// Formats CLI output for terminal or JSON consumption.
#[async_trait]
pub trait LogFormatter: Send + Sync {
    /// The output format this formatter produces.
    fn output_format(&self) -> OutputFormat;

    /// Format a `run` command output.
    async fn format_run(&self, output: &RunOutput) -> Result<String, CliError>;

    /// Format a `plan` command output.
    async fn format_plan(&self, output: &PlanOutput) -> Result<String, CliError>;

    /// Format a `generate` command output.
    async fn format_generate(&self, output: &GenerateOutput) -> Result<String, CliError>;

    /// Format an `init` command output.
    async fn format_init(&self, output: &InitOutput) -> Result<String, CliError>;

    /// Format a `history list` command output.
    async fn format_history_list(&self, output: &HistoryListOutput) -> Result<String, CliError>;

    /// Format a `history show` command output.
    async fn format_history_show(&self, output: &HistoryShowOutput) -> Result<String, CliError>;

    /// Format a `logs` command output.
    async fn format_logs(&self, output: &LogsOutput) -> Result<String, CliError>;

    /// Format an `audit list` command output.
    async fn format_audit_list(&self, output: &AuditListOutput) -> Result<String, CliError>;

    /// Format an `audit show` command output.
    async fn format_audit_show(&self, output: &AuditShowOutput) -> Result<String, CliError>;

    /// Format an `audit diff` command output.
    async fn format_audit_diff(&self, output: &AuditDiffOutput) -> Result<String, CliError>;

    /// Format a `template list` command output.
    async fn format_template_list(&self, output: &TemplateListOutput) -> Result<String, CliError>;

    /// Format a `template show` command output.
    async fn format_template_show(&self, output: &TemplateShowOutput) -> Result<String, CliError>;

    /// Format a CLI error for display.
    ///
    /// Produces a user-friendly error message with code, description,
    /// and optional resolution hint.
    async fn format_error(&self, error: &CliError) -> String;
}
