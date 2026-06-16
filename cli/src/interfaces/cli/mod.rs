//! CLI command definitions via clap derive.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CLI command definitions
//! Issue: issue-contract-freeze
//!
//! Defines the CLI argument parser using `clap` derive macros.
//! Maps user input to `CliCommand` variants that are dispatched
//! to the `CliOrchestrator`.
//!
//! # Contract (Frozen)
//! - Every command variant corresponds to a `CliOrchestrator` method
//! - Argument names match the DTO field names in `application::dto`
//! - `--help` is defined for every command and subcommand
//! - No framework-specific annotations beyond clap derives
//! - All commands support `--json` and `--quiet` global flags

use clap::{Parser, Subcommand};

/// Template-driven DAG execution engine with bounded autonomy.
///
/// rigorix plans, executes, and audits complex multi-step workflows
/// driven by LLM-generated templates. Use `rigorix <command> --help`
/// for detailed help on each command.
#[derive(Parser, Debug)]
#[command(name = "rigorix")]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    /// Global CLI flags.
    #[command(flatten)]
    pub global_opts: GlobalOptions,

    /// The command to execute.
    #[command(subcommand)]
    pub command: CliCommand,
}

/// Global options that apply to all commands.
#[derive(Parser, Debug)]
pub struct GlobalOptions {
    /// Output format: pretty, json, or quiet.
    #[arg(
        long = "format",
        global = true,
        default_value = "pretty",
        env = "RIGORIX_FORMAT"
    )]
    pub output_format: String,

    /// Enable color output: auto, always, or never.
    #[arg(
        long = "color",
        global = true,
        default_value = "auto",
        env = "RIGORIX_COLOR"
    )]
    pub color: String,

    /// Log level: trace, debug, info, warn, or error.
    #[arg(
        long = "log-level",
        global = true,
        default_value = "info",
        env = "RIGORIX_LOG"
    )]
    pub log_level: String,

    /// Log format: pretty or json.
    #[arg(
        long = "log-format",
        global = true,
        default_value = "pretty"
    )]
    pub log_format: String,

    /// Path to a custom config file.
    #[arg(
        long = "config",
        global = true,
        env = "RIGORIX_CONFIG"
    )]
    pub config_path: Option<String>,
}

/// All CLI commands supported by rigorix.
#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Execute a plan from an intent or template.
    ///
    /// Runs the full pipeline: plan → execute → render output.
    /// Use `--dry-run` to preview without executing.
    Run {
        /// The natural language intent or template ID to execute.
        intent: String,

        /// Preview the plan without executing.
        #[arg(long = "dry-run", default_value_t = false)]
        dry_run: bool,

        /// Skip risk gating confirmation prompts (for CI/CD).
        #[arg(long = "yes", short = 'y', default_value_t = false)]
        skip_confirmations: bool,

        /// Skip budget pre-checks.
        #[arg(long = "skip-budget-check", default_value_t = false)]
        skip_budget_check: bool,
    },

    /// Preview an execution plan without running.
    ///
    /// Runs the planning pipeline and shows a DAG preview with
    /// estimated costs and node dependencies.
    Plan {
        /// The natural language intent or template ID to plan.
        intent: String,
    },

    /// Initialize a new rigorix project.
    ///
    /// Creates the `.rigorix/` directory with default configuration,
    /// built-in templates, and a `.gitignore` entry for state files.
    Init {
        /// Path for the `.rigorix/` directory.
        #[arg(default_value = ".")]
        path: String,

        /// Run in non-interactive mode.
        #[arg(long = "non-interactive", default_value_t = false)]
        non_interactive: bool,

        /// API key for LLM provider.
        #[arg(long = "api-key", env = "RIGORIX_API_KEY")]
        api_key: Option<String>,

        /// Enforcement preset to configure.
        #[arg(long = "preset")]
        enforcement_preset: Option<String>,
    },

    /// Generate a new template from a natural language description.
    Generate {
        /// Natural language description of the template.
        intent: String,

        /// Print template to stdout instead of saving.
        #[arg(long = "stdout", default_value_t = false)]
        stdout: bool,

        /// Validate only — don't generate.
        #[arg(long = "dry-run", default_value_t = false)]
        dry_run: bool,
    },

    /// View past execution sessions.
    #[command(subcommand)]
    History(HistoryCommands),

    /// Stream or replay execution events.
    Logs {
        /// Session ID to replay logs from (empty for live).
        #[arg(long = "session")]
        session_id: Option<String>,

        /// Filter by event type.
        #[arg(long = "type")]
        event_type: Option<String>,

        /// Filter by node ID.
        #[arg(long = "node")]
        node_id: Option<String>,

        /// Filter by minimum severity.
        #[arg(long = "severity")]
        severity: Option<String>,

        /// Follow live execution events.
        #[arg(long = "follow", short = 'f', default_value_t = false)]
        follow: bool,

        /// Maximum number of events to return.
        #[arg(long = "limit")]
        limit: Option<u32>,
    },

    /// Inspect audit trails.
    #[command(subcommand)]
    Audit(AuditCommands),

    /// List or show registered templates.
    #[command(subcommand)]
    Template(TemplateCommands),
}

/// History subcommands.
#[derive(Subcommand, Debug)]
pub enum HistoryCommands {
    /// List past execution sessions.
    List {
        /// Maximum number of sessions to list.
        #[arg(long = "limit")]
        limit: Option<u32>,

        /// Filter by status: completed, failed, cancelled, timed_out.
        #[arg(long = "status")]
        status: Option<String>,
    },

    /// Show details of a specific session.
    Show {
        /// The session ID to show.
        session_id: String,
    },
}

/// Audit subcommands.
#[derive(Subcommand, Debug)]
pub enum AuditCommands {
    /// List audit envelopes.
    List {
        /// Maximum number to list.
        #[arg(long = "limit")]
        limit: Option<u32>,
    },

    /// Show a full audit envelope.
    Show {
        /// The audit envelope ID.
        audit_id: String,
    },

    /// Compare two audit envelopes by planning hash.
    Diff {
        /// First audit envelope ID.
        audit_id_1: String,

        /// Second audit envelope ID.
        audit_id_2: String,
    },
}

/// Template subcommands.
#[derive(Subcommand, Debug)]
pub enum TemplateCommands {
    /// List all registered templates.
    List,

    /// Show a template's TOML definition.
    Show {
        /// The template ID.
        template_id: String,
    },
}
