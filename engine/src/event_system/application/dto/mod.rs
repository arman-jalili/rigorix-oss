//! Data Transfer Objects for the Event System module.
//!
//! @canonical .pi/architecture/modules/event-system.md
//! Implements: Contract Freeze — DTO schemas for publish, subscribe, drain operations
//! Issue: #46
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::event_system::domain::{ExecutionEvent, PersistedEvent};

// ---------------------------------------------------------------------------
// Publish Event DTOs
// ---------------------------------------------------------------------------

/// Input for publishing an execution event to the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEventInput {
    /// The execution event to publish.
    pub event: ExecutionEvent,
}

/// Output from publishing an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEventOutput {
    /// The sequence number assigned to this event (1-indexed, monotonic).
    pub sequence: u64,

    /// Number of active subscribers that received this event.
    pub subscriber_count: usize,

    /// Whether any subscribers lagged (and thus missed the event).
    pub had_laggers: bool,
}

// ---------------------------------------------------------------------------
// Subscribe DTOs
// ---------------------------------------------------------------------------

/// Input for subscribing to the event bus.
///
/// Events published after subscription will be received.
/// Past events are not replayed — use `drain_persisted` for that.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeInput {
    /// Optional subscriber identifier for diagnostics.
    pub subscriber_name: Option<String>,
}

/// Output from subscribing to the event bus.
///
/// The receiver can be polled for incoming `ExecutionEvent` values.
/// Since the receiver is a framework-specific type (tokio::sync::broadcast::Receiver),
/// it is returned as a boxed, dynamically-dispatched stream at the API layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeOutput {
    /// Whether the subscription was successful.
    pub success: bool,

    /// Subscriber identifier for diagnostics.
    pub subscriber_name: String,

    /// Number of currently active subscribers.
    pub active_subscriber_count: usize,
}

// ---------------------------------------------------------------------------
// Drain Persisted DTOs
// ---------------------------------------------------------------------------

/// Input for draining persisted events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainPersistedInput {
    /// Whether to clear the persisted buffer after draining.
    /// If false, events are returned but remain in the buffer.
    pub clear: bool,
}

/// Output from draining persisted events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainPersistedOutput {
    /// The persisted events in sequence order.
    pub events: Vec<PersistedEvent>,

    /// Total number of events drained.
    pub count: u64,

    /// Whether the buffer was cleared after draining.
    pub cleared: bool,
}

// ---------------------------------------------------------------------------
// Event Bus Status DTOs
// ---------------------------------------------------------------------------

/// Input for querying event bus status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusStatusInput {
    /// Whether to include subscriber details in the response.
    pub include_subscriber_details: bool,
}

/// General event bus status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusStatus {
    /// Current number of persisted events.
    pub persisted_count: u64,

    /// Current sequence number (0 if no events published).
    pub current_sequence: u64,

    /// Number of active subscribers.
    pub active_subscriber_count: usize,

    /// Capacity of the broadcast channel.
    pub channel_capacity: usize,

    /// Capacity of the persisted event buffer.
    pub buffer_capacity: usize,
}

// ---------------------------------------------------------------------------
// Event Bus Configuration DTO
// ---------------------------------------------------------------------------

/// Configuration input for creating an EventBus instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusConfig {
    /// Capacity of the tokio broadcast channel (affects backpressure).
    /// Default: 1000
    pub channel_capacity: usize,

    /// Maximum number of events to persist in the in-memory buffer.
    /// Default: 10000
    pub buffer_capacity: usize,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1000,
            buffer_capacity: 10000,
        }
    }
}

// ---------------------------------------------------------------------------
// Event Query DTOs
// ---------------------------------------------------------------------------

/// Input for querying persisted events with optional filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEventsInput {
    /// Filter by execution ID.
    pub execution_id: Option<uuid::Uuid>,

    /// Filter by event type (variant name, e.g. "planning_started").
    pub event_type: Option<String>,

    /// Filter events after this sequence number.
    pub after_sequence: Option<u64>,

    /// Maximum number of events to return.
    pub limit: Option<u32>,

    /// Filter events after this timestamp.
    pub after_timestamp: Option<DateTime<Utc>>,

    /// Filter events before this timestamp.
    pub before_timestamp: Option<DateTime<Utc>>,
}

/// Output from querying persisted events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEventsOutput {
    /// Matching events in sequence order.
    pub events: Vec<PersistedEvent>,

    /// Total number of matching events.
    pub total: u64,

    /// Whether there are more events beyond the returned page.
    pub has_more: bool,
}

// ---------------------------------------------------------------------------
// Event Count DTO
// ---------------------------------------------------------------------------

/// Output from event count query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCountOutput {
    /// Total number of events published since bus creation.
    pub total: u64,

    /// Number of persisted events currently in buffer.
    pub persisted: u64,

    /// Number of events drained (consumed and removed from buffer).
    pub drained: u64,
}
