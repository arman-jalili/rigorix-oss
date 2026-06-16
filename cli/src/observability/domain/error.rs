//! CLI-specific observability domain errors.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — ObservabilityCliError
//! Issue: issue-contract-freeze
//!
//! Errors that originate from CLI observability operations (tracing init,
//! health checks, metrics). These are distinct from engine-level errors.
//!
//! # Contract (Frozen)
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

/// Errors that can occur during CLI observability operations.
#[derive(Debug, Error)]
pub enum ObservabilityCliError {
    /// Failed to initialize the tracing subscriber.
    #[error("Failed to initialize tracing: {detail}")]
    TracingInitFailed {
        /// What went wrong during tracing initialization.
        detail: String,
    },

    /// Health check registration failed.
    #[error("Health check registration failed: {detail}")]
    HealthCheckRegistrationFailed {
        /// The health check name.
        check_name: String,
        /// What went wrong.
        detail: String,
    },

    /// Health check execution failed.
    #[error("Health check failed for '{check_name}': {detail}")]
    HealthCheckFailed {
        /// Which health check failed.
        check_name: String,
        /// The error detail.
        detail: String,
    },

    /// Metrics registration failed.
    #[error("Metrics registration failed: {detail}")]
    MetricsRegistrationFailed {
        /// The metric name.
        metric_name: String,
        /// What went wrong.
        detail: String,
    },

    /// An unexpected internal error occurred.
    #[error("Internal observability error: {detail}")]
    Internal {
        /// Description of the internal error.
        detail: String,
    },
}

impl ObservabilityCliError {
    /// Returns `true` if this error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(self, ObservabilityCliError::TracingInitFailed { .. })
    }
}
