//! Repository interfaces for the Event System.
//!
//! @canonical .pi/architecture/modules/event-system.md
//! Implements: Contract Freeze — PersistedEventRepository trait
//! Issue: #46
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use in-memory storage, filesystem persistence,
//! or database backends without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::event_system::domain::{EventSystemError, PersistedEvent};

use crate::event_system::application::dto::{EventBusConfig, QueryEventsInput};

/// Repository for persisting and retrieving execution events.
///
/// Provides append-only storage of persisted events with sequential
/// ordering and filtering capabilities. The primary implementation
/// uses in-memory `Vec<PersistedEvent>` with `Mutex` synchronization.
///
/// # Security
/// - Implementations MUST NOT log sensitive event payload data
/// - All inputs must be validated against size limits
#[async_trait]
pub trait PersistedEventRepository: Send + Sync {
    /// Persist an execution event.
    ///
    /// Appends the event to the in-memory store with the next
    /// monotonic sequence number. Returns the assigned sequence number.
    async fn save(&self, event: &PersistedEvent) -> Result<u64, EventSystemError>;

    /// Retrieve all persisted events in sequence order.
    ///
    /// Returns events ordered by their monotonic sequence number (ascending).
    async fn find_all(&self) -> Result<Vec<PersistedEvent>, EventSystemError>;

    /// Query persisted events with optional filters.
    ///
    /// Supports filtering by execution ID, event type, sequence range,
    /// and timestamp range. Results are returned in sequence order.
    /// `limit` caps the number of results.
    async fn query(&self, input: &QueryEventsInput) -> Result<Vec<PersistedEvent>, EventSystemError>;

    /// Drain all persisted events, clearing the buffer.
    ///
    /// Returns all events in sequence order and empties the store.
    /// Returns `AlreadyDrained` error if the store is already empty
    /// and was previously drained.
    async fn drain(&self) -> Result<Vec<PersistedEvent>, EventSystemError>;

    /// Get the total number of persisted events.
    async fn count(&self) -> Result<u64, EventSystemError>;

    /// Get the current sequence number (0 if empty).
    async fn current_sequence(&self) -> Result<u64, EventSystemError>;

    /// Prune events older than the given timestamp.
    ///
    /// Returns the number of events removed.
    async fn prune(&self, older_than: chrono::DateTime<chrono::Utc>) -> Result<u64, EventSystemError>;

    /// Clear all persisted events.
    ///
    /// Returns the number of events removed.
    async fn clear(&self) -> Result<u64, EventSystemError>;

    /// Check whether the repository has been drained.
    async fn is_drained(&self) -> Result<bool, EventSystemError>;

    /// Configure the repository (capacity limits, etc.).
    ///
    /// Must be called before any save/drain operations.
    async fn configure(&self, config: &EventBusConfig) -> Result<(), EventSystemError>;
}
