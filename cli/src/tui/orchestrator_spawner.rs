//! OrchestratorSpawner — background orchestrator task management.
//!
//! @canonical .pi/architecture/modules/tui.md#orchestrator-spawner
//! Implements: Contract Freeze — OrchestratorSpawner component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Spawns the engine orchestrator in a `tokio::spawn`'d background task.
//! Builds the same `OrchestratorBuilder` as `cli_boundary`. Manages
//! lifecycle — start, graceful cancel, immediate abort — keeping UI
//! responsive.

use tokio::sync::mpsc;

use crate::cli_boundary::config::CliConfig;
use crate::cli_boundary::error::CliError;

use super::event_bridge::TuiCommand;

// ---------------------------------------------------------------------------
// Orchestrator state
// ---------------------------------------------------------------------------

/// Current state of the spawned orchestrator task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorTaskState {
    /// No orchestrator running.
    Idle,
    /// Orchestrator is running in a background task.
    Running,
    /// Orchestrator completed successfully.
    Completed,
    /// Orchestrator failed.
    Failed,
    /// Orchestrator was cancelled.
    Cancelled,
}

// ---------------------------------------------------------------------------
// OrchestratorSpawner trait
// ---------------------------------------------------------------------------

/// Manages the lifecycle of a background orchestrator task.
///
/// The spawner is responsible for:
/// 1. Building the `OrchestratorService` from config
/// 2. Spawning it in a `tokio::spawn`'d task
/// 3. Providing a `CancellationToken` for cancellation
/// 4. Tracking task state (running, completed, failed, cancelled)
/// 5. Exposing the EventBus for subscription
#[async_trait::async_trait]
pub trait OrchestratorSpawner: Send + Sync {
    /// Spawn a new orchestrator run in a background task.
    ///
    /// Builds the orchestrator service from config, subscribes the
    /// EventBridge, and spawns the run. Returns immediately —
    /// the orchestrator runs in the background.
    ///
    /// # Errors
    ///
    /// Returns `CliError` if the orchestrator builder fails or config
    /// is invalid.
    async fn spawn_run(&self, config: CliConfig, intent: String) -> Result<(), CliError>;

    /// Spawn a plan-only operation in a background task.
    async fn spawn_plan_only(&self, config: CliConfig, intent: String) -> Result<(), CliError>;

    /// Request graceful cancellation of the running orchestrator.
    async fn cancel_graceful(&self) -> Result<(), CliError>;

    /// Request immediate abort of the running orchestrator.
    async fn cancel_immediate(&self) -> Result<(), CliError>;

    /// Get the current task state.
    fn task_state(&self) -> OrchestratorTaskState;

    /// Get a sender for TUI commands (reverse channel).
    fn command_sender(&self) -> Option<mpsc::Sender<TuiCommand>>;
}
