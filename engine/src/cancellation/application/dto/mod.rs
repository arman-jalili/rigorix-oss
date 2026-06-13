//! Data Transfer Objects for the Cancellation module.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — DTO schemas for cancellation operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::cancellation::domain::ShutdownSignal;

// ---------------------------------------------------------------------------
// Cancel Execution DTOs
// ---------------------------------------------------------------------------

/// Input for requesting cancellation of an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelExecutionInput {
    /// The execution identifier to cancel.
    pub execution_id: String,

    /// Human-readable reason for cancellation.
    pub reason: Option<String>,

    /// Source of the cancellation request.
    ///
    /// Known sources: "user", "signal", "enforcement", "timeout"
    pub source: String,
}

/// Output from requesting cancellation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelExecutionOutput {
    /// Whether the cancellation request was accepted.
    pub accepted: bool,

    /// The signal that was sent.
    pub signal: ShutdownSignal,

    /// Number of tasks that will be affected.
    pub affected_tasks: u32,

    /// Whether cancellation was already in progress.
    pub was_already_cancelling: bool,
}

// ---------------------------------------------------------------------------
// Shutdown DTOs
// ---------------------------------------------------------------------------

/// Input for awaiting shutdown completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownInput {
    /// The execution ID to await shutdown for.
    pub execution_id: String,

    /// Maximum time to wait for graceful shutdown in seconds.
    ///
    /// After this timeout, remaining tasks are force-aborted.
    /// Must be > 0. Default: 30.
    pub timeout_secs: u64,

    /// Whether to force-abort remaining tasks after timeout.
    /// If false, returns `ShutdownTimeout` error without aborting.
    pub force_abort_on_timeout: bool,
}

/// Output from awaiting shutdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownOutput {
    /// The execution ID.
    pub execution_id: String,

    /// The signal that was used for shutdown.
    pub signal_used: ShutdownSignal,

    /// Total number of tasks when shutdown was requested.
    pub total_tasks: u32,

    /// Number of tasks that completed naturally.
    pub completed_tasks: u32,

    /// Number of tasks that were cancelled/aborted.
    pub cancelled_tasks: u32,

    /// Total time spent in shutdown in milliseconds.
    pub shutdown_duration_ms: u64,

    /// Whether the shutdown was forced (aborted remaining tasks).
    pub forced: bool,

    /// Whether cleanup handlers were invoked successfully.
    pub cleanup_success: bool,
}

// ---------------------------------------------------------------------------
// Status DTOs
// ---------------------------------------------------------------------------

/// Output for querying shutdown status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownStatusOutput {
    /// Whether cancellation has been requested.
    pub is_cancelled: bool,

    /// The current shutdown signal, if any.
    pub current_signal: Option<ShutdownSignal>,

    /// Number of tasks currently running.
    pub running_tasks: u32,

    /// Number of tasks that have completed.
    pub completed_tasks: u32,

    /// Number of tasks that have been cancelled.
    pub cancelled_tasks: u32,

    /// Whether shutdown is complete.
    pub shutdown_complete: bool,

    /// Elapsed time since cancellation was requested, in milliseconds.
    pub elapsed_since_request_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Task Registration DTOs
// ---------------------------------------------------------------------------

/// Input for registering a task with the cancellation manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterTaskInput {
    /// The execution ID.
    pub execution_id: String,

    /// Unique task identifier within this execution.
    pub task_id: String,

    /// Optional description of the task.
    pub description: Option<String>,
}

/// Output from registering a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterTaskOutput {
    /// Whether registration was accepted.
    pub accepted: bool,
    /// Current cancellation state at registration time.
    pub is_already_cancelled: bool,
}

/// Input for notifying completion of a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyTaskCompleteInput {
    /// The execution ID.
    pub execution_id: String,

    /// The task identifier that completed.
    pub task_id: String,

    /// Whether the task completed normally or was cancelled.
    pub was_cancelled: bool,

    /// Duration the task was running in milliseconds.
    pub running_duration_ms: u64,

    /// Whether cleanup was performed successfully.
    pub cleanup_success: bool,
}
