//! Event payload schemas for the Audit Posting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — AuditPostingEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted whenever audit records are created, posted,
//! queued, or fail delivery. Consumers (console, TUI, alerting) subscribe
//! to these event types via the EventBus.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use serde::{Deserialize, Serialize};

/// Events emitted by the Audit Posting module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditPostingEvent {
    /// An audit record was successfully posted to the backend.
    RecordPosted {
        /// The execution ID this record belongs to.
        execution_id: String,
        /// Posting attempt number.
        attempt: u32,
        /// Duration of the post in milliseconds.
        duration_ms: u64,
        /// Target backend URL.
        backend_url: String,
    },

    /// An audit record failed to post and was queued for retry.
    RecordQueuedForRetry {
        /// The execution ID this record belongs to.
        execution_id: String,
        /// Why posting failed.
        reason: String,
        /// Number of pending retries so far.
        retry_count: u32,
        /// Maximum retries before dropping.
        max_retries: u32,
    },

    /// An audit record was permanently dropped after exhausting retries.
    RecordDropped {
        /// The execution ID this record belongs to.
        execution_id: String,
        /// Total posting attempts made.
        attempts: u32,
        /// Final error detail.
        reason: String,
    },

    /// An audit record was created.
    RecordCreated {
        /// The execution ID this record belongs to.
        execution_id: String,
        /// The execution mode.
        mode: String,
        /// Whether the record was signed.
        signed: bool,
    },

    /// An audit record was loaded from the filesystem backend.
    RecordLoaded {
        /// The execution ID this record belongs to.
        execution_id: String,
        /// Whether signature verification succeeded.
        signature_valid: bool,
    },

    /// HMAC signing key was rotated or refreshed.
    SigningKeyRotated {
        /// Key identifier for the new key.
        key_id: String,
        /// Timestamp of the rotation.
        rotated_at: chrono::DateTime<chrono::Utc>,
    },

    /// An error occurred during audit posting (non-fatal warning).
    PostingWarning {
        /// The execution ID, if available.
        execution_id: Option<String>,
        /// Warning message.
        message: String,
    },
}
