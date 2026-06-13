//! ShutdownSignal value object.
//!
//! @canonical .pi/architecture/modules/cancellation.md#signal
//! Implements: Contract Freeze — ShutdownSignal enum
//! Issue: issue-contract-freeze
//!
//! Defines two shutdown levels for execution cancellation:
//! - `Graceful`: let running tasks finish, don't start new ones
//! - `Immediate`: abort all in-flight work immediately
//!
//! # Contract (Frozen)
//! - `ShutdownSignal` is a simple enum with two variants
//! - Used by `CancellationManager.request_shutdown()`
//! - Propagated via `tokio::sync::watch` channel to subscribers
//! - Implementations MUST NOT introduce new variants without architecture review

use serde::{Deserialize, Serialize};

/// Shutdown signal levels for execution cancellation.
///
/// Determines how aggressively running work is terminated when cancellation
/// is requested. The orchestrator and all concurrent tasks must respect this
/// signal level.
///
/// # Usage
/// - `Graceful`: Stop accepting new work, let in-flight tasks complete naturally
/// - `Immediate`: Abort all in-flight work via `JoinSet::abort()` or equivalent
///
/// # Cancellation Response Time (NFR-007)
/// The system SHALL support cancellation within 200ms of signal receipt,
/// regardless of which shutdown level is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShutdownSignal {
    /// Let running tasks finish naturally. No new tasks are started.
    /// Resources (file handles, network connections, etc.) are cleaned up
    /// by the running tasks themselves.
    Graceful,

    /// Abort all in-flight work immediately using task abort mechanisms
    /// (e.g., `JoinSet::abort()`). In-flight work may leave resources in
    /// an inconsistent state; cleanup handlers MUST be registered.
    Immediate,
}
