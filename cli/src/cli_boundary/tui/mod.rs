//! TUI renderer module for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — TUI renderer interfaces
//! Issue: issue-contract-freeze
//!
//! Ratatui-based terminal UI that subscribes to the engine's EventBus
//! broadcast channel and renders live execution progress.
//!
//! Three panels:
//! - DAG graph: node status transitions
//! - Budget bars: tokens/calls/time usage
//! - Event log: scrollable event stream
//!
//! # Contract (Frozen)
//! - TUI renderer subscribes to EventBus via broadcast channel
//! - Runs in a separate tokio task — never blocks the event loop
//! - `--no-tui` flag falls back to console output
//! - Handles terminal resize via SIGWINCH
//! - Ctrl+C still works (TUI doesn't consume the signal)

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;

/// Renders a ratatui-based terminal UI during execution.
///
/// Subscribes to the engine's event stream and renders three panels:
/// DAG graph, budget bars, and event log. Manages terminal setup and
/// teardown (raw mode, alternate screen).
#[async_trait]
pub trait TuiRenderer: Send + Sync {
    /// Start the TUI renderer.
    ///
    /// Enters raw mode, switches to alternate screen, starts listening
    /// for events on the broadcast channel. Blocks until `stop()` is
    /// called or an unrecoverable error occurs.
    async fn start(&mut self) -> Result<(), CliError>;

    /// Stop the TUI renderer.
    ///
    /// Leaves raw mode, restores the main screen, and flushes any
    /// remaining output. Safe to call multiple times.
    async fn stop(&mut self) -> Result<(), CliError>;

    /// Check whether the TUI is currently active.
    fn is_active(&self) -> bool;

    /// Get the terminal dimensions (columns, rows).
    fn terminal_size(&self) -> Option<(u16, u16)>;
}
