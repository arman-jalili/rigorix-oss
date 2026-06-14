//! Service interfaces (use cases) for the Event System.
//!
//! @canonical .pi/architecture/modules/event-system.md#bus
//! Implements: Contract Freeze — EventBusService trait
//! Issue: #46
//!
//! This trait defines the application-level operations for the event bus:
//! publishing events, subscribing to events, draining persisted events,
//! querying event history, and checking bus status.
//!
//! All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::event_system::domain::EventSystemError;

use super::dto::{
    DrainPersistedInput, DrainPersistedOutput, EventBusStatus, EventBusStatusInput,
    EventCountOutput, PublishEventInput, PublishEventOutput, QueryEventsInput, QueryEventsOutput,
    SubscribeInput, SubscribeOutput,
};

/// Central event bus service for publishing and subscribing to execution events.
///
/// Orchestrates event delivery via a pub-sub model with synchronous in-memory
/// persistence. Events broadcast to all active subscribers and are simultaneously
/// persisted for drain-at-end retrieval.
///
/// # Performance
/// - `publish` is non-blocking on the broadcasting path
/// - Persistence is synchronous via `std::sync::Mutex` (no tokio spawn)
/// - Subscribers that fall behind will lag and miss events (channel capacity)
/// - At execution end, `drain_persisted()` produces a complete, ordered record
#[async_trait]
pub trait EventBusService: Send + Sync {
    /// Publish an execution event to all subscribers and persist it.
    ///
    /// The event is broadcast to all active subscribers via the tokio channel
    /// and synchronously written to the in-memory persisted buffer.
    /// Slow subscribers that cannot keep up will receive a `RecvError::Lagged`.
    ///
    /// Returns the assigned sequence number and delivery statistics.
    async fn publish(
        &self,
        input: PublishEventInput,
    ) -> Result<PublishEventOutput, EventSystemError>;

    /// Subscribe to receive future execution events.
    ///
    /// Creates a new subscriber that will receive all events published
    /// after the subscription is established. Past events are not replayed.
    ///
    /// The subscriber is identified by an optional name for diagnostics.
    /// The returned output confirms the subscription was registered.
    async fn subscribe(&self, input: SubscribeInput) -> Result<SubscribeOutput, EventSystemError>;

    /// Drain all persisted events from the buffer in sequence order.
    ///
    /// Returns all events in the order they were published (by monotonic sequence).
    /// After draining, the buffer is cleared (unless `clear: false` is specified).
    ///
    /// Designed to be called once at execution end to populate `ExecutionRecord`.
    /// Calling drain a second time returns `EventSystemError::AlreadyDrained`.
    async fn drain_persisted(
        &self,
        input: DrainPersistedInput,
    ) -> Result<DrainPersistedOutput, EventSystemError>;

    /// Query persisted events with optional filters.
    ///
    /// Supports filtering by execution ID, event type, sequence range,
    /// and timestamp range. Results are returned in sequence order.
    async fn query_events(
        &self,
        input: QueryEventsInput,
    ) -> Result<QueryEventsOutput, EventSystemError>;

    /// Get current event bus status (counts, subscribers, capacity).
    async fn status(&self, input: EventBusStatusInput) -> Result<EventBusStatus, EventSystemError>;

    /// Get event count statistics.
    async fn event_count(&self) -> Result<EventCountOutput, EventSystemError>;
}
