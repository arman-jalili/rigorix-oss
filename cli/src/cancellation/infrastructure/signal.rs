//! Signal handling interface for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — SignalHandler trait
//! Issue: issue-contract-freeze
//!
//! Captures Ctrl+C (SIGINT) and manages two-level shutdown:
//! - Single press → graceful shutdown (finish current node)
//! - Double press within 2s → immediate abort (abort all in-flight)
//!
//! Per ADR-007, the CLI is ephemeral — signal handling is per-process.
//!
//! # Contract (Frozen)
//! - `install()` registers the OS signal handler and returns a receiver
//! - Double press detection uses a configurable window (default 2s)
//! - Forwards signals to the engine's `CancellationService`
//! - No signal handler state leaks between process invocations

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;

/// Represents a shutdown signal level from the user.
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
