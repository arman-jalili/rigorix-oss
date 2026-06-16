//! Clap-based argument parser for all CLI commands.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#commands
//! Implements: contract freeze — CliParser component
//! Issue: issue-cliparser
//!
//! # Contract (Frozen)
//!
//! This module defines the complete command surface of the `rigorix` CLI.
//! All 14 commands, global flags, and shortcut flags are defined here.
//!
//! Commands are organised into three tiers (see cli-boundary.md):
//! - **Tier 1**: Via `OrchestratorService` (run, plan, cancel, status)
//! - **Tier 2**: Via engine services directly (history, explain, diff-plan, generate,
//!   template, audit, logs, config)
//! - **Tier 3**: CLI-only (init, key)
//!
//! TUI launch is the default when no subcommand is given.
//!
//! # Types
//!
//! - `CliCommand` — enum of all 14+1 command variants
//! - `Format` — output format selection (Pretty, Json, Markdown, Quiet)
//! - `parse_args()` — parse command-line arguments and return the resolved `CliCommand`

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Output format
// ---------------------------------------------------------------------------

/// Supported output formats for CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
pub enum Format {
    /// Human-readable with Unicode symbols.
    #[default]
    Pretty,
    /// JSON for CI/CD integration and scripting.
    Json,
    /// Markdown documentation output.
    Markdown,
    /// Minimal output, exit codes only.
    Quiet,
}

// ---------------------------------------------------------------------------
// Sub-command action enums
// ---------------------------------------------------------------------------

/// Actions for the `template` subcommand.
#[derive(Debug, Subcommand)]
pub enum TemplateAction {
    /// List all available templates.
    List,
    /// Show a specific template by ID or name.
    Show {
        /// Template ID or name to display.
        id: String,
    },
}

/// Actions for the `audit` subcommand.
#[derive(Debug, Subcommand)]
pub enum AuditAction {
    /// List audit entries with optional limit.
    List {
        /// Maximum number of entries to show.
        #[arg(short = 'n', long)]
        limit: Option<u32>,
    },
    /// Show a specific audit entry.
    Show {
        /// Audit entry ID to display.
        id: String,
    },
    /// Show diff between two audit entries.
    Diff {
        /// First audit entry ID.
        id1: String,
        /// Second audit entry ID.
        id2: String,
    },
}

/// Actions for the `config` subcommand.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Scaffold a default rigorix.toml in CWD.
    Init,
    /// Show current merged configuration (secrets redacted).
    Show,
    /// Validate configuration against safety caps.
    Validate,
}

// ---------------------------------------------------------------------------
// Command enum — complete CLI command surface
// ---------------------------------------------------------------------------

/// All CLI commands organised by execution tier.
///
/// **Tier 1** — Full lifecycle via `OrchestratorService`
/// **Tier 2** — Direct engine service calls
/// **Tier 3** — CLI-only operations
/// **Tui** — Interactive terminal UI
#[derive(Debug, Subcommand)]
pub enum CliCommand {
    // ── Tier 1: Via OrchestratorService ──────────────────────────────
    /// Full lifecycle: plan → execute → persist → emit → record.
    Run {
        /// Natural-language intent describing what to execute.
        intent: String,

        /// Optional enforcement preset override.
        #[arg(long)]
        enforcement: Option<String>,

        /// Optional LLM budget cap (max calls).
        #[arg(long = "max-llm-calls")]
        max_llm_calls: Option<u32>,

        /// Optional LLM budget cap (max tokens).
        #[arg(long = "max-llm-tokens")]
        max_llm_tokens: Option<u64>,
    },

    /// Plan only — preview the generated plan without executing.
    Plan {
        /// Natural-language intent to plan.
        intent: String,
    },

    /// Cancel a running execution by ID.
    Cancel {
        /// Execution ID to cancel.
        execution_id: uuid::Uuid,
    },

    /// Show the current or most recent execution status.
    Status,

    // ── Tier 2: Via Engine Services Directly ─────────────────────────
    /// List past executions with optional filtering.
    History {
        /// Maximum number of entries to show.
        #[arg(short = 'n', long)]
        limit: Option<u32>,

        /// Filter by execution status.
        #[arg(long)]
        status: Option<String>,
    },

    /// Show detailed information about a single execution.
    Explain {
        /// Execution ID to inspect.
        execution_id: uuid::Uuid,

        /// Optional second execution ID for comparison.
        #[arg(long)]
        diff: Option<uuid::Uuid>,
    },

    /// Compare two plans side-by-side.
    DiffPlan {
        /// First plan identifier.
        id1: uuid::Uuid,
        /// Second plan identifier.
        id2: uuid::Uuid,
    },

    /// Generate a reusable template from a natural-language intent.
    Generate {
        /// Intent to be converted into a template.
        intent: String,
    },

    /// Browse or inspect available templates.
    Template {
        #[command(subcommand)]
        action: TemplateAction,
    },

    /// Browse audit trails for execution records.
    Audit {
        #[command(subcommand)]
        action: AuditAction,
    },

    /// View raw execution session logs.
    Logs {
        /// Optional session ID to filter logs.
        #[arg(short = 'n', long)]
        session_id: Option<uuid::Uuid>,
    },

    /// Manage or inspect CLI/engine configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    // ── Tier 3: CLI-Only ────────────────────────────────────────
    /// Scaffold `.rigorix/` directory with a default `rigorix.toml`.
    Init,

    /// Generate API keys.
    Key {
        /// Optional human-readable label for the key.
        #[arg(long)]
        label: Option<String>,
    },

    // ── TUI (hidden from help — default with no subcommand) ──────
    /// Launch the interactive terminal UI.
    #[command(hide = true)]
    Tui {
        /// Load a specific past execution into the TUI.
        #[arg(long)]
        exec: Option<uuid::Uuid>,

        /// Start the TUI with a running orchestrator for this intent.
        #[arg(long)]
        run: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Top-level CLI argument structure
// ---------------------------------------------------------------------------

/// Rigorix — Template-driven DAG execution engine.
///
/// Run `rigorix` with no arguments to launch the interactive TUI.
/// Use subcommands for scripting and CI/CD integration.
#[derive(Debug, Parser)]
#[command(name = "rigorix", version, about)]
pub struct CliArgs {
    /// Output format override
    #[arg(long = "format", value_enum, default_value_t = Format::Pretty, global = true)]
    pub format: Format,

    /// Verbose output (-v = debug, -vv = trace)
    #[arg(short = 'v', action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    // ── Shortcut Flags ──────────────────────────────────────────────
    /// Shortcut: rigorix run <intent>
    #[arg(long = "run")]
    pub run: Option<String>,

    /// Shortcut: rigorix tui --exec <id>
    #[arg(long = "exec")]
    pub exec: Option<String>,

    /// Shortcut: rigorix history
    #[arg(long = "history")]
    pub history: bool,

    // ── Subcommands ─────────────────────────────────────────────────
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse CLI arguments and return the resolved `CliCommand`.
///
/// **Contract:**
/// - Shortcut flags (`--run`, `--exec`, `--history`) are expanded to their
///   equivalent command variants.
/// - No subcommand and no shortcut → `CliCommand::Tui` (interactive default).
/// - `--format` and `-v` are consumed before command resolution.
pub fn parse_args() -> CliCommand {
    let args = CliArgs::parse();

    // Check shortcut flags first (they take priority over subcommands)
    if let Some(intent) = args.run {
        return CliCommand::Run {
            intent,
            enforcement: None,
            max_llm_calls: None,
            max_llm_tokens: None,
        };
    }

    if let Some(exec_str) = args.exec {
        let exec = exec_str.parse().ok();
        return CliCommand::Tui { exec, run: None };
    }

    if args.history {
        return CliCommand::History {
            limit: None,
            status: None,
        };
    }

    // Resolve subcommand or default to TUI
    args.command.unwrap_or(CliCommand::Tui {
        exec: None,
        run: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_default() {
        assert_eq!(Format::default(), Format::Pretty);
    }

    #[test]
    fn test_format_variants() {
        assert_eq!(
            Format::Pretty.to_possible_value().unwrap().get_name(),
            "pretty"
        );
        assert_eq!(Format::Json.to_possible_value().unwrap().get_name(), "json");
        assert_eq!(
            Format::Markdown.to_possible_value().unwrap().get_name(),
            "markdown"
        );
        assert_eq!(
            Format::Quiet.to_possible_value().unwrap().get_name(),
            "quiet"
        );
    }
}
