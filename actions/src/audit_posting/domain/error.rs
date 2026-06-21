//! Error types for the Audit Posting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — AuditPostingError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `AuditPostingError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

/// Errors that can occur during audit record posting.
#[derive(Debug, Error)]
pub enum AuditPostingError {
    /// Failed to sign the audit record.
    #[error("Failed to sign audit record: {detail}")]
    SigningFailed {
        /// Human-readable error description.
        detail: String,
    },

    /// Failed to post the audit record to the backend.
    #[error("Failed to post audit record: {detail} (attempt {attempt}/{max_retries})")]
    PostFailed {
        /// Human-readable error description.
        detail: String,
        /// Which attempt this was (1-indexed).
        attempt: u32,
        /// Maximum retry attempts configured.
        max_retries: u32,
        /// HTTP status code if applicable.
        http_status: Option<u16>,
    },

    /// Audit backend unavailable or unreachable.
    #[error("Audit backend unavailable at {backend_url}: {detail}")]
    BackendUnavailable {
        /// The backend URL that failed.
        backend_url: String,
        /// Error detail.
        detail: String,
        /// Whether the error is likely transient.
        is_transient: bool,
    },

    /// Failed to serialize audit record to JSON.
    #[error("Failed to serialize audit record: {detail}")]
    SerializationFailed {
        /// The serialization error details.
        detail: String,
    },

    /// HMAC signature verification failed on a retrieved record.
    #[error(
        "Audit record HMAC signature verification failed: expected {expected_prefix}, got {received_prefix}"
    )]
    SignatureMismatch {
        /// Expected signature prefix (truncated for display).
        expected_prefix: String,
        /// Received signature prefix (truncated for display).
        received_prefix: String,
    },

    /// Audit record queue is full.
    #[error("Audit record queue is full (capacity: {capacity}, pending: {pending})")]
    QueueFull {
        /// Maximum queue capacity.
        capacity: u32,
        /// Current number of pending records.
        pending: u32,
    },

    /// Audit posting configuration is invalid or missing.
    #[error("Audit posting not configured: missing {missing_field}")]
    NotConfigured {
        /// Which configuration field is missing.
        missing_field: String,
    },

    /// Record not found in the backend.
    #[error("Audit record not found: {execution_id}")]
    RecordNotFound {
        /// The execution ID that was not found.
        execution_id: uuid::Uuid,
    },

    /// Filesystem backend error (I/O, permissions, etc.).
    #[error("Filesystem audit backend error: {detail}")]
    FilesystemError {
        /// Error detail including file path if applicable.
        detail: String,
        /// The underlying OS error code, if any.
        os_error: Option<i32>,
    },

    /// HMAC signing key not available.
    #[error("HMAC signing key not available: {detail}")]
    KeyNotAvailable {
        /// Error detail.
        detail: String,
    },

    /// An internal error occurred (e.g. lock poisoned, channel closed).
    #[error("Internal audit posting error: {detail}")]
    Internal {
        /// Error detail for diagnostics.
        detail: String,
    },
}

impl AuditPostingError {
    /// Whether the error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            AuditPostingError::PostFailed { .. }
                | AuditPostingError::BackendUnavailable {
                    is_transient: true,
                    ..
                }
                | AuditPostingError::FilesystemError { .. }
        )
    }
}
