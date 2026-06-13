//! Audit error types.
//!
//! @canonical .pi/architecture/modules/error-handling.md#audit
//! Implements: Contract Freeze — AuditError enum
//! Issue: #13
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `AuditError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during audit envelope creation and delivery.
#[derive(Debug, Error)]
pub enum AuditError {
    /// Failed to send envelope to audit backend.
    #[error("Failed to send audit envelope: {detail} (attempt {attempt}/{max_retries})")]
    SendFailed {
        /// Human-readable error description.
        detail: String,
        /// Which attempt this was (1-indexed).
        attempt: u32,
        /// Maximum retry attempts configured.
        max_retries: u32,
        /// HTTP status code if applicable.
        http_status: Option<u16>,
    },

    /// Circuit breaker is open — request rejected without attempting.
    #[error("Circuit breaker is open for audit backend {backend_url}")]
    CircuitBreakerOpen {
        /// The audit backend URL that is being circuit-broken.
        backend_url: String,
        /// When the circuit breaker opened (Unix timestamp).
        opened_at: i64,
        /// How long until the next half-open probe (seconds).
        retry_after_secs: u64,
    },

    /// Failed to serialize audit envelope to JSON.
    #[error("Failed to serialize audit envelope: {detail}")]
    SerializationFailed {
        /// The serialization error details.
        detail: String,
    },

    /// HMAC signature verification failed.
    #[error("Audit envelope HMAC signature verification failed")]
    SignatureMismatch {
        /// Expected signature (truncated for display).
        expected_prefix: String,
        /// Received signature (truncated for display).
        received_prefix: String,
    },

    /// Audit queue is full — cannot enqueue more failed deliveries.
    #[error("Audit delivery queue is full (capacity: {capacity}, pending: {pending})")]
    QueueFull {
        /// Maximum queue capacity.
        capacity: u32,
        /// Current number of pending items.
        pending: u32,
    },

    /// Audit backend configuration is invalid or missing.
    #[error("Audit backend not configured: missing {missing_field}")]
    NotConfigured {
        /// Which configuration field is missing.
        missing_field: String,
    },

    /// An internal error occurred (e.g. lock poisoned, channel closed).
    #[error("Internal audit error: {detail}")]
    Internal {
        /// Error detail for diagnostics.
        detail: String,
    },
}
