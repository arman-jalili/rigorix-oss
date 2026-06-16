//! Command dispatcher — routes parsed commands to engine services.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#dispatch-logic
//! Implements: Contract Freeze — Dispatcher component: dispatch trait and result type
//! Issue: issue-contract-freeze
//!
//! # Contract (Frozen)
//!
//! The dispatcher is a `match` over `CliCommand` variants that routes each
//! command to the appropriate handler:
//!
//! | Command | Target | Method |
//! |---------|--------|--------|
//! | Run | OrchestratorService | `orchestrator.run(RunInput)` |
//! | Plan | OrchestratorService | `orchestrator.plan_only(PlanOnlyInput)` |
//! | Cancel | OrchestratorService | `orchestrator.cancel(CancelInput)` |
//! | Status | OrchestratorService | `orchestrator.status()` |
//! | History | StateManagerService | `list_executions(limit, status)` |
//! | Explain | StateManagerService | `load_state(execution_id)` |
//! | DiffPlan | DagPlanningService | `compare_plans(id1, id2)` |
//! | Generate | TemplateGenerator | `generate(intent)` |
//! | Template | TemplateEngine | `list() / show(id)` |
//! | Audit | AuditService | `list() / show() / diff()` |
//! | Logs | EventBusService | `subscribe(session_id)` |
//! | Config | ConfigService | `validate() / load() / init()` |
//! | Init | CLI-only | scaffold `.rigorix/` |
//! | Key | CLI-only | generate API key |
//! | Tui | TUI module | `tui::run(config, ct, exec, run)` |
//!
//! The `DispatchResult` wraps the engine's output into a unified type that
//! the output formatter can render.

use std::fmt;

use serde_json::Value as JsonValue;

use crate::cli_boundary::cli::CliCommand;

// ---------------------------------------------------------------------------
// Dispatch result type
// ---------------------------------------------------------------------------

/// Unified result from any command dispatch.
///
/// The output formatter (`output::format_and_exit`) renders this type
/// into the selected output format (Pretty, JSON, Markdown, Quiet).
#[derive(Debug)]
pub struct DispatchResult {
    /// Human-readable summary of the operation result.
    pub summary: String,

    /// Structured data payload for JSON/Markdown output.
    pub data: Option<JsonValue>,

    /// Exit code to return (0 = success, non-zero = error).
    pub exit_code: i32,
}

impl DispatchResult {
    /// Create a successful dispatch result.
    pub fn success(summary: impl Into<String>) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: None,
            exit_code: 0,
        }
    }

    /// Create a successful dispatch result with structured data.
    pub fn success_with_data(summary: impl Into<String>, data: JsonValue) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: Some(data),
            exit_code: 0,
        }
    }

    /// Create an error dispatch result.
    pub fn error(summary: impl Into<String>, exit_code: i32) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: None,
            exit_code,
        }
    }

    /// Returns `true` if this result represents a successful dispatch.
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

impl fmt::Display for DispatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Dispatch a parsed CLI command to the appropriate handler.
///
/// Routes the command to:
/// - `OrchestratorService` for Tier 1 commands (run, plan, cancel, status)
/// - Individual engine services for Tier 2 commands
/// - CLI-only logic for Tier 3 commands (init, key)
/// - The TUI module for the Tui variant
///
/// Returns a `DispatchResult` that the output formatter can render.
///
/// # Errors
///
/// Returns `DispatchResult` with `exit_code != 0` on failure. The error
/// summary is already formatted for user display.
pub async fn dispatch(
    command: CliCommand,
    config: crate::cli_boundary::config::CliConfig,
    cancellation_token: crate::cli_boundary::signal::CancellationToken,
) -> DispatchResult {
    // Placeholder: routes each command to a NotImplemented handler.
    // Implementation issue: replace with full dispatch to engine services.
    let _ = (config, cancellation_token);
    match command {
        CliCommand::Run { intent, .. } => DispatchResult::success(format!("Run: {intent}")),
        CliCommand::Plan { intent } => DispatchResult::success(format!("Plan: {intent}")),
        CliCommand::Cancel { execution_id } => {
            DispatchResult::success(format!("Cancel: {execution_id}"))
        }
        CliCommand::Status => DispatchResult::success("Status: ok"),
        CliCommand::History { limit, status } => {
            DispatchResult::success(format!("History: limit={limit:?}, status={status:?}"))
        }
        CliCommand::Explain { execution_id, diff } => {
            DispatchResult::success(format!("Explain: {execution_id}, diff={diff:?}"))
        }
        CliCommand::DiffPlan { id1, id2 } => {
            DispatchResult::success(format!("DiffPlan: {id1} vs {id2}"))
        }
        CliCommand::Generate { intent } => DispatchResult::success(format!("Generate: {intent}")),
        CliCommand::Template { .. } => DispatchResult::success("Template"),
        CliCommand::Audit { .. } => DispatchResult::success("Audit"),
        CliCommand::Logs { .. } => DispatchResult::success("Logs"),
        CliCommand::Config { .. } => DispatchResult::success("Config"),
        CliCommand::Init => DispatchResult::success("Init"),
        CliCommand::Key { .. } => DispatchResult::success("Key"),
        CliCommand::Tui { exec, run } => {
            DispatchResult::success(format!("Tui: exec={exec:?}, run={run:?}"))
        }
    }
}
