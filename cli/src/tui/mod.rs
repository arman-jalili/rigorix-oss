//! Terminal UI module — interactive TUI (ratatui).
//!
//! @canonical .pi/architecture/modules/tui.md
//! Implements: Contract Freeze — TUI module root
//! Issue: issue-contract-freeze
//!
//! # Contract (Frozen)
//!
//! The TUI module is the primary user interface for Rigorix. It provides
//! an interactive terminal dashboard using `ratatui` + `crossterm`.
//!
//! Entry point: `pub async fn run(config, cancellation_token, exec, run)`
//!
//! ## Views
//! - Dashboard (DAG tree + details + metrics)
//! - Plan Preview (template, confidence, nodes, actions)
//! - History (past execution browser)
//! - Events (filterable timeline)
//! - Nodes (full node list)
//! - Templates (list/show)
//! - Settings (configuration panel)
//! - Clarification (LLM clarification requests)
//! - Diff (plan comparison)
//!
//! ## Components
//! See `.pi/architecture/modules/tui.md#components` for detailed component breakdown.

use tokio_util::sync::CancellationToken;

use crate::cli_boundary::config::CliConfig;

/// Run the interactive TUI.
///
/// This is the default entry point when `rigorix` is invoked with no
/// subcommand. It:
///
/// 1. Initialises the ratatui terminal
/// 2. Builds the ViewModel
/// 3. Subscribes to the engine EventBus (via EventBridge)
/// 4. Renders the dashboard loop
/// 5. Handles keyboard input via the command bar
///
/// # Parameters
///
/// * `config` — Merged CLI configuration.
/// * `cancellation_token` — Shared cancellation handle.
/// * `exec` — Optional execution ID to load into read-only mode.
/// * `run` — Optional intent to start executing immediately.
///
/// # Returns
///
/// Returns when the user quits or the TUI is otherwise terminated.
pub async fn run(
    config: CliConfig,
    cancellation_token: CancellationToken,
    exec: Option<uuid::Uuid>,
    run: Option<String>,
) {
    // Placeholder: no-op until TUI implementation issue.
    // Implementation issue: implement ratatui event loop with all views.
    let _ = (config, cancellation_token, exec, run);
}
