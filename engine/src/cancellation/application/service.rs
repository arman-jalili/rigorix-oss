//! Service interfaces (use cases) for the Cancellation bounded context.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CancellationService, CleanupHandler traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for cancellation
//! management, signal propagation, and cleanup coordination. All methods
//! are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::cancellation::domain::{CancellationError, ShutdownSignal};

use super::dto::{
    CancelExecutionInput, CancelExecutionOutput, ShutdownInput, ShutdownOutput,
    ShutdownStatusOutput,
};

/// Central cancellation service for managing execution lifecycle.
///
/// Orchestrates the full cancellation workflow: requesting shutdown at
/// various levels, propagating signals to concurrent tasks, coordinating
/// cleanup handlers, and reporting shutdown status.
///
/// Implementations wrap a `CancellationManager` and expose its operations
/// through typed DTOs.
#[async_trait]
pub trait CancellationService: Send + Sync {
    /// Request graceful shutdown of an execution.
    ///
    /// Running tasks are allowed to finish naturally. No new tasks are
    /// started. Emits `CancellationEvent::CancellationRequested` with
    /// `ShutdownSignal::Graceful`.
    ///
    /// Returns `CancellationError::AlreadyCancelling` if cancellation is
    /// already in progress.
    async fn request_graceful_shutdown(
        &self,
        input: CancelExecutionInput,
    ) -> Result<CancelExecutionOutput, CancellationError>;

    /// Request immediate abort of an execution.
    ///
    /// All in-flight work is aborted immediately via task abort mechanisms
    /// (e.g., `JoinSet::abort()`). Cleanup handlers SHOULD still run.
    /// Emits `CancellationEvent::CancellationRequested` with
    /// `ShutdownSignal::Immediate`.
    ///
    /// Returns `CancellationError::AlreadyCancelling` if cancellation is
    /// already in progress.
    async fn request_immediate_abort(
        &self,
        input: CancelExecutionInput,
    ) -> Result<CancelExecutionOutput, CancellationError>;

    /// Wait for all running tasks to complete (graceful shutdown).
    ///
    /// Blocks until either all tasks complete or the timeout is reached.
    /// If the timeout is exceeded, remaining tasks are force-aborted
    /// and `CancellationError::ShutdownTimeout` is returned.
    async fn await_shutdown(
        &self,
        input: ShutdownInput,
    ) -> Result<ShutdownOutput, CancellationError>;

    /// Check whether cancellation has been requested.
    fn is_cancelled(&self) -> bool;

    /// Get the current shutdown signal level.
    ///
    /// Returns `None` if no shutdown has been requested.
    fn current_signal(&self) -> Option<ShutdownSignal>;

    /// Get current shutdown status.
    async fn status(&self) -> ShutdownStatusOutput;

    /// Subscribe to shutdown signals via a watch channel.
    ///
    /// Returns a receiver that yields `ShutdownSignal` values.
    /// The receiver immediately yields the current signal if one is active.
    fn subscribe(&self) -> tokio::sync::watch::Receiver<ShutdownSignal>;

    /// Get a reference to the underlying `CancellationToken`.
    ///
    /// Long-running tasks can use this token with `tokio::select!` or
    /// `.cancelled()` to be notified of cancellation.
    fn cancellation_token(&self) -> tokio_util::sync::CancellationToken;
}

/// Handler for task-level cleanup during cancellation.
///
/// Components that hold resources (file handles, network connections,
/// budget reservations) implement this trait to perform cleanup when
/// cancellation is signalled.
///
/// # Contract
/// - `cleanup()` MUST NOT block indefinitely (use internal timeouts)
/// - `cleanup()` MUST be idempotent (may be called multiple times)
/// - Failures SHOULD be logged but MUST NOT cause cascading failures
#[async_trait]
pub trait CleanupHandler: Send + Sync {
    /// Perform cleanup for a cancelled task or resource.
    ///
    /// Called during shutdown after the task has been stopped.
    /// Returns an error if cleanup fails (non-fatal, logged).
    async fn cleanup(&self, task_id: &str) -> Result<(), CancellationError>;

    /// Release any resources held by this handler.
    ///
    /// Called during final shutdown phase. After this call, the handler
    /// should be considered disposed.
    async fn release(&self, task_id: &str) -> Result<(), CancellationError>;
}
