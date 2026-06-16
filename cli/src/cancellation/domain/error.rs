//! CLI-specific cancellation domain errors.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CancellationCliError
//! Issue: issue-contract-freeze
//!
//! Errors that originate from CLI cancellation operations (signal handling,
//! shutdown coordination). These are distinct from the engine's `CancellationError`
//! — they cover CLI-level concerns like signal handler installation failures.
//!
//! # Contract (Frozen)
//! - `CancellationCliError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Maps to `CliError` at the boundary layer

use thiserror::Error;

/// Errors that can occur during CLI cancellation operations.
#[derive(Debug, Error)]
pub enum CancellationCliError {
    /// Failed to install the OS signal handler.
    #[error("Failed to install signal handler: {detail}")]
    SignalInstallFailed {
        /// What went wrong during signal handler setup.
        detail: String,
    },

    /// The signal handler was already installed.
    #[error("Signal handler already installed")]
    AlreadyInstalled,

    /// Failed to propagate the shutdown signal to components.
    #[error("Failed to propagate shutdown signal: {detail}")]
    SignalPropagationFailed {
        /// What went wrong during signal propagation.
        detail: String,
    },

    /// A requested operation is not valid in the current shutdown state.
    #[error("Operation not allowed in current shutdown state: {state}")]
    InvalidState {
        /// The current shutdown state that prevented the operation.
        state: String,
    },

    /// An unexpected internal error occurred.
    #[error("Internal cancellation error: {detail}")]
    Internal {
        /// Description of the internal error.
        detail: String,
    },
}

impl CancellationCliError {
    /// Returns `true` if this error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(self, CancellationCliError::SignalInstallFailed { .. })
    }
}
