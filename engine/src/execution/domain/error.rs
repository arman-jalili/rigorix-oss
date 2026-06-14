//! Execution error types.
//!
//! @canonical .pi/architecture/modules/error-handling.md#execution
//! Implements: Contract Freeze — ExecutionError enum
//! Issue: #186
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `ExecutionError` is the single error type for execution operations
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during task execution.
///
/// These errors cover the lifecycle of executing individual tasks/nodes
/// in the DAG engine. They are distinct from graph construction errors
/// (handled by `DagError`) and policy enforcement errors (handled by
/// `EnforcementError`).
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// A task failed during execution.
    ///
    /// The task started but encountered an unrecoverable error. This differs
    /// from `RetryLimitExceeded` in that this is a single-attempt failure,
    /// not an exhaustion of retries.
    #[error("Task '{task_id}' failed: {message}")]
    TaskFailed {
        /// The ID of the task that failed.
        task_id: String,
        /// Human-readable description of the failure.
        message: String,
        /// The failure type classification, if available.
        failure_type: Option<String>,
    },

    /// A task or execution timed out.
    ///
    /// The task did not complete within the configured timeout period.
    #[error("Execution timed out after {timeout_secs}s for task '{task_id}'")]
    Timeout {
        /// The ID of the task that timed out.
        task_id: String,
        /// The timeout duration in seconds.
        timeout_secs: u64,
        /// How long the task was running before timeout (seconds).
        elapsed_secs: u64,
    },

    /// The execution engine has not been initialized.
    ///
    /// An operation was attempted before the execution engine was fully
    /// configured and started.
    #[error("Execution engine not initialized: {detail}")]
    NotInitialized {
        /// Details about what is missing.
        detail: String,
    },

    /// A task is already running and cannot be started again.
    #[error("Task '{task_id}' is already running")]
    AlreadyRunning {
        /// The ID of the task that is already running.
        task_id: String,
    },

    /// The current execution plan requires re-planning.
    ///
    /// This is not a failure but a signal that the execution engine
    /// needs to go back to the planning phase. This can occur when
    /// a task's output invalidates the current plan (e.g., new
    /// information discovered during execution).
    #[error("Execution requires re-planning: {reason}")]
    RequiresReplan {
        /// Why re-planning is needed.
        reason: String,
        /// The task that triggered the re-planning need.
        trigger_task_id: Option<String>,
    },

    /// A fallback handler was required but failed.
    ///
    /// When a primary task fails and a fallback is configured, this
    /// error is raised if the fallback also fails.
    #[error("Fallback execution failed for task '{task_id}': {message}")]
    FallbackRequired {
        /// The ID of the primary task that failed.
        task_id: String,
        /// Human-readable description of the fallback failure.
        message: String,
        /// The fallback strategy that was attempted.
        strategy: String,
    },
}
