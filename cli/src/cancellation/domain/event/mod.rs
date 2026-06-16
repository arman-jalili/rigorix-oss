//! Event payload schemas for the CLI Cancellation module.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CancellationCliEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted by the CLI cancellation module whenever signals
//! are received or shutdown state changes. Consumers (output formatters,
//! TUI, loggers) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - All events are serializable for logging and CI/CD output

use serde::{Deserialize, Serialize};

/// Events emitted by the CLI Cancellation module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CancellationCliEvent {
    /// A single Ctrl+C was received — graceful shutdown initiated.
    GracefulShutdownRequested {
        /// The double-press window in seconds.
        grace_period_secs: u64,
    },

    /// A second Ctrl+C was received within the window — immediate abort.
    ImmediateShutdownRequested {
        /// Time elapsed between first and second signal in milliseconds.
        elapsed_ms: u64,
    },

    /// The grace period expired without a second signal.
    GracePeriodExpired,

    /// The signal handler was successfully installed.
    SignalHandlerInstalled,

    /// The signal handler installation failed.
    SignalHandlerInstallFailed {
        /// Error message describing the failure.
        error: String,
    },

    /// A shutdown signal was forwarded to the engine's cancellation service.
    ShutdownSignalForwarded {
        /// The shutdown level forwarded (graceful or immediate).
        level: String,
    },
}
