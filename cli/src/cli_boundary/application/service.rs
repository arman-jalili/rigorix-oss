//! Service interfaces for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CliOrchestrator, ExecutionSession traits
//! Issue: issue-contract-freeze
//!
//! These traits define the top-level CLI operations that wire engine
//! capabilities to user-facing commands. All methods are async and
//! return CLI error types.
//!
//! # Contract (Frozen)
//! - Every CLI command has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

use super::dto::{
    AuditDiffInput, AuditDiffOutput, AuditListInput, AuditListOutput, AuditShowInput,
    AuditShowOutput, GenerateInput, GenerateOutput, HistoryListInput, HistoryListOutput,
    HistoryShowInput, HistoryShowOutput, InitInput, InitOutput, LogsInput, LogsOutput, PlanInput,
    PlanOutput, RunInput, RunOutput, TemplateListInput, TemplateListOutput, TemplateShowInput,
    TemplateShowOutput,
};

/// Top-level orchestrator for the CLI.
///
/// Wires the full lifecycle: config loading → command dispatch →
/// engine execution → output rendering. Manages signal handlers and
/// TUI lifecycle.
///
/// Implementations own the execution session and coordinate all
/// components (config, signal handler, engine orchestrator, event bus,
/// output formatter, TUI).
#[async_trait]
pub trait CliOrchestrator: Send + Sync {
    /// Run a full execution session (plan + execute).
    ///
    /// This is the main entry point for `rigorix run <intent>`.
    /// It loads config, creates a session, runs the planning pipeline,
    /// executes the DAG, renders output, and returns a structured result.
    ///
    /// # Cancellation
    /// - Single Ctrl+C → graceful shutdown (finish current node)
    /// - Double Ctrl+C within 2s → immediate abort
    async fn run(&self, input: RunInput) -> Result<RunOutput, CliError>;

    /// Preview execution plan without running.
    ///
    /// Entry point for `rigorix plan <intent>`. Runs the 6-phase
    /// planning pipeline but does not execute. Returns a DAG preview
    /// with node dependencies, tool bindings, and cost estimates.
    async fn plan(&self, input: PlanInput) -> Result<PlanOutput, CliError>;

    /// Generate a new template from natural language.
    ///
    /// Entry point for `rigorix generate <intent>`. Uses the LLM to
    /// generate a TOML template, validates it, and persists to
    /// `.rigorix/templates/`. Supports `--dry-run` and `--stdout`.
    async fn generate(&self, input: GenerateInput) -> Result<GenerateOutput, CliError>;

    /// Initialize a new `.rigorix/` project directory.
    ///
    /// Entry point for `rigorix init`. Creates the directory structure,
    /// default config, and optionally prompts for API key and enforcement
    /// preset.
    async fn init(&self, input: InitInput) -> Result<InitOutput, CliError>;

    /// List past execution sessions.
    ///
    /// Entry point for `rigorix history`. Reads persisted execution
    /// state files and returns session summaries.
    async fn history_list(&self, input: HistoryListInput) -> Result<HistoryListOutput, CliError>;

    /// Show details of a specific execution session.
    ///
    /// Entry point for `rigorix history show <id>`. Returns per-node
    /// execution results.
    async fn history_show(&self, input: HistoryShowInput) -> Result<HistoryShowOutput, CliError>;

    /// Stream or replay execution events.
    ///
    /// Entry point for `rigorix logs`. Can replay past events or
    /// follow live execution with `--follow`.
    async fn logs(&self, input: LogsInput) -> Result<LogsOutput, CliError>;

    /// List audit envelopes.
    ///
    /// Entry point for `rigorix audit list`.
    async fn audit_list(&self, input: AuditListInput) -> Result<AuditListOutput, CliError>;

    /// Show a full audit envelope.
    ///
    /// Entry point for `rigorix audit show <id>`.
    async fn audit_show(&self, input: AuditShowInput) -> Result<AuditShowOutput, CliError>;

    /// Diff two audit envelopes by planning hash.
    ///
    /// Entry point for `rigorix audit diff <id1> <id2>`.
    async fn audit_diff(&self, input: AuditDiffInput) -> Result<AuditDiffOutput, CliError>;

    /// List all registered templates.
    ///
    /// Entry point for `rigorix template list`.
    async fn template_list(&self, input: TemplateListInput)
    -> Result<TemplateListOutput, CliError>;

    /// Show a specific template's TOML definition.
    ///
    /// Entry point for `rigorix template show <id>`.
    async fn template_show(&self, input: TemplateShowInput)
    -> Result<TemplateShowOutput, CliError>;

    /// Get the current CLI configuration.
    fn config(&self) -> &CliConfig;
}

/// Manages a single execution session lifecycle.
///
/// Created by `CliOrchestrator` for each `rigorix run` invocation.
/// Manages the steps: load config → plan → execute → render output.
///
/// # Lifecycle
/// 1. `new()` — create with session config and intent
/// 2. `start()` — initialize session, begin execution
/// 3. Wait for completion or cancellation
/// 4. `result()` — get final result
#[async_trait]
pub trait ExecutionSession: Send + Sync {
    /// Start the execution session.
    ///
    /// Loads config, runs the planning pipeline, creates the engine
    /// orchestrator, and begins DAG execution. Returns immediately;
    /// progress is communicated via the event bus.
    async fn start(&mut self) -> Result<(), CliError>;

    /// Cancel the session gracefully (finish current node).
    async fn cancel_graceful(&mut self) -> Result<(), CliError>;

    /// Cancel the session immediately (abort in-flight).
    async fn cancel_immediate(&mut self) -> Result<(), CliError>;

    /// Wait for the session to complete.
    async fn wait_for_completion(&mut self) -> Result<(), CliError>;

    /// Get the session ID.
    fn session_id(&self) -> &str;

    /// Get the final execution result.
    ///
    /// Returns `None` if the session has not completed yet.
    fn result(&self) -> Option<RunOutput>;
}
