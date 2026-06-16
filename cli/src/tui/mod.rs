//! Terminal UI module — interactive TUI (ratatui).
//!
//! @canonical .pi/architecture/modules/tui.md
//! Implements: Contract Freeze — TUI module root
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! The TUI module is the primary user interface for Rigorix. It provides
//! an interactive terminal dashboard using `ratatui` + `crossterm`.
//!
//! ## Module Structure
//!
//! ```text
//! tui/
//! ├── mod.rs                  ← this file
//! ├── event_bridge.rs         # EventBridge (EventBus → ViewModel)
//! ├── view_model.rs           # ViewModel types (TuiViewModel, DagViewModel, etc.)
//! ├── orchestrator_spawner.rs # Background orchestrator task management
//! ├── plan_review.rs          # Plan preview state and actions
//! ├── command_bar.rs          # Command bar input state + history
//! ├── views/
//! │   ├── mod.rs              # View trait + implementations
//! │   ├── dashboard.rs
//! │   ├── plan.rs
//! │   ├── history.rs
//! │   ├── events.rs
//! │   ├── nodes.rs
//! │   ├── settings.rs
//! │   ├── templates.rs
//! │   ├── clarification.rs
//! │   └── diff.rs
//! ├── widgets/
//! │   ├── mod.rs              # Widget trait
//! │   ├── dag_tree.rs
//! │   ├── progress_bar.rs
//! │   ├── modal.rs
//! │   ├── status_bar.rs
//! │   ├── event_log.rs
//! │   ├── keybind_hint.rs
//! │   └── tool_output.rs
//! └── input/
//!     ├── mod.rs              # Input handler + keymap types
//!     ├── keymap.rs           # Key binding configuration
//!     └── command_palette.rs  # Fuzzy-find /commands
//! ```
//!
//! ## Components
//!
//! | Component | Module | Status |
//! |-----------|--------|--------|
//! | CommandBar | `tui::command_bar` | Planned |
//! | PlanReview | `tui::plan_review` | Planned |
//! | EventBridge | `tui::event_bridge` | Planned |
//! | ViewModel | `tui::view_model` | Planned |
//! | Renderer | `tui::widgets` | Planned |
//! | Views | `tui::views` | Planned |
//! | InputHandler | `tui::input` | Planned |
//! | OrchestratorSpawner | `tui::orchestrator_spawner` | Planned |

pub mod command_bar;
pub mod event_bridge;
pub mod input;
pub mod orchestrator_spawner;
pub mod plan_review;
pub mod view_model;
pub mod views;
pub mod widgets;

use tokio_util::sync::CancellationToken;

use crate::cli_boundary::config::CliConfig;

/// Run the interactive TUI.
///
/// This is the primary entry point when `rigorix` is invoked with no
/// subcommand. The TUI owns the terminal rendering loop, event bridge,
/// and orchestrator lifecycle.
///
/// # Parameters
///
/// * `config` — Merged CLI configuration (format, verbosity, engine config).
/// * `cancellation_token` — Shared cancellation handle from signal handler.
/// * `exec` — Optional execution ID to load into read-only mode.
/// * `run` — Optional intent to start executing immediately.
///
/// # Returns
///
/// Returns when the user quits the TUI (via `:q` or Ctrl+C).
pub async fn run(
    config: CliConfig,
    cancellation_token: CancellationToken,
    exec: Option<uuid::Uuid>,
    run: Option<String>,
) {
    // Placeholder: no-op until TUI implementation.
    // Implementation issue: initialise ratatui terminal, build ViewModel,
    // subscribe to EventBus, start render loop, handle keyboard input.
    let _ = (config, cancellation_token, exec, run);
}
