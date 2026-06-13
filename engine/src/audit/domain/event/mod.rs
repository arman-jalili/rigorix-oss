//! Event payload schemas for the Audit bounded context.
//!
//! @canonical .pi/architecture/decisions/ADR-005-event-bus-persistence.md
//! Implements: Contract Freeze — AuditEvent payload schemas
//! Issue: #13
//!
//! These events are emitted on the `EventBus` whenever audit envelopes are
//! created, sent, queued, or fail delivery. Consumers (console, TUI, alerting)
//! subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use serde::{Deserialize, Serialize};

/// Events emitted by the Audit module.
///
/// Wrapped in `ExecutionEvent::Audit(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    /// An audit envelope was successfully delivered to the backend.
    EnvelopeDelivered {
        /// The execution ID this envelope belongs to.
        execution_id: String,
        /// Delivery attempt number.
        attempt: u32,
        /// Duration of delivery in milliseconds.
        duration_ms: u64,
    },

    /// An audit envelope failed delivery and was queued for retry.
    EnvelopeQueued {
        /// The execution ID this envelope belongs to.
        execution_id: String,
        /// Why delivery failed.
        reason: String,
        /// Number of pending retries so far.
        retry_count: u32,
        /// Maximum retries before dropping.
        max_retries: u32,
    },

    /// An audit envelope was permanently dropped after exhausting retries.
    EnvelopeDropped {
        /// The execution ID this envelope belongs to.
        execution_id: String,
        /// Total delivery attempts made.
        attempts: u32,
        /// Final error detail.
        reason: String,
    },

    /// Circuit breaker state changed.
    CircuitBreakerStateChanged {
        /// The backend URL affected.
        backend_url: String,
        /// Previous state.
        from_state: String,
        /// New state.
        to_state: String,
    },

    /// Audit envelope was created but sending is disabled.
    EnvelopeCreated {
        /// The execution ID this envelope belongs to.
        execution_id: String,
        /// Reason audit is disabled (if applicable).
        reason: Option<String>,
    },
}
