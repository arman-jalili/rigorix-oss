//! Service interfaces for the CLI Cancellation module.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — SignalHandler trait
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for CLI cancellation
//! and signal handling. All methods are async and return domain error types.
//! Implementations reside in the infrastructure layer.
//!
//! # Contract (Frozen)
//! - Every cancellation use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;

/// Represents a shutdown signal level from the user.
///
/// Two-level shutdown:
/// - `Graceful` — single Ctrl+C: finish current work, no new tasks
/// - `Immediate` — double Ctrl+C: abort all in-flight work
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownLevel {
    /// Graceful shutdown — finish current work, no new tasks.
    Graceful,
    /// Immediate abort — stop all in-flight work.
    Immediate,
}

/// Captures OS signals and converts them to structured shutdown requests.
///
/// Implementations register with the OS signal handler (SIGINT, SIGTERM)
/// and provide a channel receiver for the orchestrator to poll.
///
/// # Contract (Frozen)
/// - `install()` registers the OS signal handler and returns a receiver
/// - Double press detection uses a configurable window (default 2s)
/// - Forwards signals to the engine's `CancellationService`
/// - No signal handler state leaks between process invocations
#[async_trait]
pub trait SignalHandler: Send + Sync {
    /// Install OS signal handlers and return a receiver for shutdown signals.
    ///
    /// Must be called early in `main()`, before any async tasks are spawned.
    /// Returns a `tokio::sync::watch::Receiver` that yields `ShutdownLevel`.
    async fn install(&self) -> Result<tokio::sync::watch::Receiver<ShutdownLevel>, CliError>;

    /// Get the double-press window duration in seconds.
    fn double_press_window_secs(&self) -> u64;
}
