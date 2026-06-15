//! Cancellation error types.
//!
//! @canonical .pi/architecture/modules/error-handling.md#cancellation
//! Implements: Contract Freeze — CancellationError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `CancellationError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during cancellation operations.
#[derive(Debug, Error)]
pub enum CancellationError {
    /// A cancellation was requested but a task was not found to cancel.
    #[error("Task not found for cancellation: {task_id}")]
    TaskNotFound {
        /// The identifier of the task that could not be found.
        task_id: String,
    },

    /// A task was already cancelled and cannot be cancelled again.
    #[error("Task is already cancelled: {task_id}")]
    AlreadyCancelled {
        /// The identifier of the already-cancelled task.
        task_id: String,
    },

    /// The cancellation manager has already been triggered.
    #[error("Cancellation already in progress with signal: {current_signal:?}")]
    AlreadyCancelling {
        /// The shutdown signal that is already in effect.
        current_signal: super::ShutdownSignal,
    },

    /// A watch channel receiver was dropped before receiving the signal.
    #[error("No subscribers available to receive shutdown signal")]
    NoSubscribers,

    /// Internal channel error during signal propagation.
    #[error("Internal cancellation channel error: {detail}")]
    ChannelError {
        /// Description of the channel error.
        detail: String,
    },

    /// Timeout reached while waiting for graceful shutdown to complete.
    #[error("Graceful shutdown timed out after {timeout_secs}s with {pending_tasks} tasks still running")]
    ShutdownTimeout {
        /// Timeout duration in seconds.
        timeout_secs: u64,
        /// Number of tasks still running when timeout was reached.
        pending_tasks: u32,
    },

    /// Cleanup handler failed during cancellation.
    #[error("Cleanup handler failed for task {task_id}: {detail}")]
    CleanupFailed {
        /// The task that failed during cleanup.
        task_id: String,
        /// What went wrong during cleanup.
        detail: String,
    },
}
impl CancellationError {
    pub fn is_retriable(&self) -> bool {
        false
    }
}
