//! Event payload schemas for the Cancellation bounded context.
//!
//! @canonical .pi/architecture/decisions/ADR-005-event-bus-persistence.md
//! Implements: Contract Freeze — CancellationEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the `EventBus` whenever cancellation is
//! requested, progresses, or completes. Consumers (orchestrator, audit,
//! TUI, budget tracking) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use serde::{Deserialize, Serialize};

/// Events emitted by the Cancellation module.
///
/// Wrapped in `ExecutionEvent::Cancellation(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CancellationEvent {
    /// Cancellation was requested with a specific shutdown signal.
    CancellationRequested {
        /// The execution ID being cancelled.
        execution_id: String,
        /// The shutdown signal level requested.
        signal: String,
        /// Human-readable reason for cancellation.
        reason: Option<String>,
        /// Source of the cancellation request (e.g., "user", "signal", "enforcement").
        source: String,
    },

    /// A graceful shutdown is in progress — running tasks continue.
    GracefulShutdownStarted {
        /// The execution ID being shut down.
        execution_id: String,
        /// Number of tasks still running.
        running_tasks: u32,
        /// Graceful shutdown timeout in seconds.
        timeout_secs: u64,
    },

    /// An immediate shutdown was triggered — all tasks are aborted.
    ImmediateShutdownExecuted {
        /// The execution ID being shut down.
        execution_id: String,
        /// Number of tasks that were aborted.
        aborted_task_count: u32,
    },

    /// A task has acknowledged the cancellation signal.
    TaskCancelled {
        /// The execution ID.
        execution_id: String,
        /// The task identifier that was cancelled.
        task_id: String,
        /// Whether the task completed cleanup before stopping.
        cleanup_completed: bool,
        /// Duration the task was running before cancellation.
        running_duration_ms: u64,
    },

    /// All tasks have completed shutdown (graceful or immediate).
    ShutdownComplete {
        /// The execution ID.
        execution_id: String,
        /// The signal that was used.
        signal_used: String,
        /// Total number of tasks that were running when shutdown was requested.
        total_tasks: u32,
        /// Number of tasks that completed naturally.
        completed_tasks: u32,
        /// Number of tasks that were aborted/cancelled.
        cancelled_tasks: u32,
        /// Total duration of shutdown in milliseconds.
        shutdown_duration_ms: u64,
    },

    /// Graceful shutdown timed out — some tasks were force-aborted.
    ShutdownTimeout {
        /// The execution ID.
        execution_id: String,
        /// Timeout that was exceeded.
        timeout_secs: u64,
        /// Number of tasks that were still running when the timeout was reached.
        pending_tasks: u32,
        /// Number of tasks that were force-aborted.
        force_aborted: u32,
    },

    /// A cleanup handler failed during cancellation (non-fatal).
    CleanupFailure {
        /// The execution ID.
        execution_id: String,
        /// The task or resource that failed cleanup.
        task_id: String,
        /// What went wrong.
        error: String,
    },
}
