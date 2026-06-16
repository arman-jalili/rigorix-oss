//! Clap-based argument parser for all CLI commands.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#commands
//! Implements: Contract Freeze — CliParser component: command type definitions
//! Issue: issue-contract-freeze
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

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Output format
// ---------------------------------------------------------------------------

/// Supported output formats for CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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

impl Format {
    /// Default format when no `--format` flag is provided.
    pub const fn default() -> Self {
        Format::Pretty
    }
}

// ---------------------------------------------------------------------------
// Sub-command action enums
// ---------------------------------------------------------------------------

/// Actions for the `template` subcommand.
#[derive(Debug)]
pub enum TemplateAction {
    /// List all available templates.
    List,
    /// Show a specific template by ID or name.
    Show { id: String },
}

/// Actions for the `audit` subcommand.
#[derive(Debug)]
pub enum AuditAction {
    /// List audit entries with optional limit.
    List { limit: Option<u32> },
    /// Show a specific audit entry.
    Show { id: String },
    /// Show diff between two audit entries.
    Diff { id1: String, id2: String },
}

/// Actions for the `config` subcommand.
#[derive(Debug)]
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
#[derive(Debug)]
pub enum CliCommand {
    // ── Tier 1: Via OrchestratorService ──────────────────────────────
    /// Full lifecycle: plan → execute → persist → emit → record.
    Run {
        /// Natural-language intent describing what to execute.
        intent: String,

        /// Optional enforcement preset override.
        enforcement: Option<String>,

        /// Optional LLM budget cap (max calls).
        max_llm_calls: Option<u32>,

        /// Optional LLM budget cap (max tokens).
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
        limit: Option<u32>,

        /// Filter by execution status.
        status: Option<String>,
    },

    /// Show detailed information about a single execution.
    Explain {
        /// Execution ID to inspect.
        execution_id: uuid::Uuid,

        /// Optional second execution ID for comparison.
        diff_id: Option<uuid::Uuid>,
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
    Template { action: TemplateAction },

    /// Browse audit trails for execution records.
    Audit { action: AuditAction },

    /// View raw execution session logs.
    Logs {
        /// Optional session ID to filter logs.
        session_id: Option<uuid::Uuid>,
    },

    /// Manage or inspect CLI/engine configuration.
    Config { action: ConfigAction },

    // ── Tier 3: CLI-Only ────────────────────────────────────────
    /// Scaffold `.rigorix/` directory with a default `rigorix.toml`.
    Init,

    /// Generate API keys.
    Key {
        /// Optional human-readable label for the key.
        label: Option<String>,
    },

    // ── TUI ──────────────────────────────────────────────────────
    /// Launch the interactive terminal UI.
    ///
    /// When no subcommand is given, `parse_args()` returns this variant
    /// with `exec` and `run` set to `None`.
    Tui {
        /// Load a specific past execution into the TUI.
        exec: Option<uuid::Uuid>,

        /// Start the TUI with a running orchestrator for this intent.
        run: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse command-line arguments and return the resolved `CliCommand`.
///
/// **Contract:**
/// - Shortcut flags (`--run`, `--exec`, `--history`) are expanded to their
///   equivalent command variants.
/// - No subcommand and no shortcut → `CliCommand::Tui` (interactive default).
/// - `--format` and `-v` are consumed before command resolution.
///
/// **Implementation notes:**
/// - Use `clap::Parser` derive on a top-level `CliArgs` struct
/// - Use `clap::Subcommand` derive or manual `ArgMatches` parsing
/// - Shortcut flags must be checked before subcommand resolution
/// - Invalid UUID for `--exec` should produce a user-friendly error
pub fn parse_args() -> CliCommand {
    // Placeholder: returns the default interactive TUI command.
    // Implementation issue: replace with full clap parsing.
    CliCommand::Tui {
        exec: None,
        run: None,
    }
}
