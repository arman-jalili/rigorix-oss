//! Factory interfaces for constructing EventBus instances.
//!
//! @canonical .pi/architecture/modules/event-system.md#bus
//! Implements: Contract Freeze — EventBusFactory trait
//! Issue: #46
//!
//! Factories encapsulate the construction of `EventBusService` instances,
//! allowing implementations to inject dependencies (tokio runtime, storage
//! backends) and apply default configurations without exposing construction
//! logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Default configuration is applied when not explicitly provided
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::event_system::domain::EventSystemError;

use super::dto::EventBusConfig;
use super::service::EventBusService;

/// Factory for constructing `EventBusService` instances.
///
/// Handles creation with default or explicit configuration, including
/// channel capacity and buffer limits.
#[async_trait]
pub trait EventBusFactory: Send + Sync {
    /// Create an `EventBusService` with explicit configuration.
    ///
    /// Returns a boxed `EventBusService` trait object ready for use.
    async fn create(
        &self,
        config: EventBusConfig,
    ) -> Result<Box<dyn EventBusService>, EventSystemError>;

    /// Create an `EventBusService` with default configuration.
    ///
    /// Uses `EventBusConfig::default()` (channel capacity: 1000, buffer: 10000).
    async fn create_default(&self) -> Result<Box<dyn EventBusService>, EventSystemError>;

    /// Create an `EventBusService` with a custom channel capacity.
    ///
    /// Convenience method for the most common customization.
    /// Validates that `channel_capacity` meets the minimum requirement.
    async fn create_with_capacity(
        &self,
        channel_capacity: usize,
    ) -> Result<Box<dyn EventBusService>, EventSystemError>;
}
