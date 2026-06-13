//! Event system error types.
//!
//! @canonical .pi/architecture/modules/error-handling.md#event-system
//! Implements: Contract Freeze — EventSystemError enum
//! Issue: #46
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `EventSystemError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during event publishing, subscription, and draining.
#[derive(Debug, Error)]
pub enum EventSystemError {
    /// The broadcast channel is at capacity and a subscriber is too slow.
    ///
    /// The lagging subscriber missed events. The `lagged` field indicates
    /// how many events were dropped.
    #[error("Subscriber lagged by {lagged} events on the broadcast channel")]
    SubscriberLagged {
        /// Number of events that were dropped due to subscriber slowness.
        lagged: u64,
    },

    /// The broadcast channel has no active receivers.
    ///
    /// An event was published but no subscriber was listening.
    /// This is informational — events are still persisted.
    #[error("No active subscribers for published event (sequence: {sequence})")]
    NoSubscribers {
        /// Sequence number of the event that had no subscribers.
        sequence: u64,
    },

    /// Failed to serialize an event for persistence or transmission.
    #[error("Failed to serialize execution event: {detail}")]
    SerializationFailed {
        /// Serialization error detail.
        detail: String,
    },

    /// Failed to deserialize an event from storage.
    #[error("Failed to deserialize execution event: {detail}")]
    DeserializationFailed {
        /// Deserialization error detail.
        detail: String,
    },

    /// The event bus was already drained and cannot be drained again.
    #[error("Event bus already drained (events returned: {count})")]
    AlreadyDrained {
        /// Number of events that were returned from the first drain.
        count: u64,
    },

    /// The event bus capacity was configured below the minimum.
    #[error("Event bus capacity {capacity} is below minimum {minimum}")]
    CapacityTooLow {
        /// Configured capacity.
        capacity: usize,
        /// Minimum allowed capacity.
        minimum: usize,
    },

    /// An internal error occurred (e.g., lock poisoned, channel closed unexpectedly).
    #[error("Internal event system error: {detail}")]
    Internal {
        /// Error detail for diagnostics.
        detail: String,
    },
}
