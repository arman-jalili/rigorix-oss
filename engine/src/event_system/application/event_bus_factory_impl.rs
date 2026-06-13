//! Implementation of `EventBusFactory`.
//!
//! @canonical .pi/architecture/modules/event-system.md#bus
//! Implements: EventBusFactory trait — constructs EventBusService instances
//! Issue: #47
//!
//! Provides factory methods for creating `EventBusService` instances with
//! default or explicit configuration, including validation of minimum
//! capacity requirements.

use async_trait::async_trait;

use crate::event_system::domain::EventSystemError;

use super::dto::EventBusConfig;
use super::event_bus_service_impl::EventBusServiceImpl;
use super::factory::EventBusFactory;
use super::service::EventBusService;

/// Minimum allowed channel capacity.
const MIN_CHANNEL_CAPACITY: usize = 16;

/// Minimum allowed buffer capacity.
const MIN_BUFFER_CAPACITY: usize = 64;

/// Factory for constructing `EventBusService` instances.
///
/// Validates configuration parameters before creating instances.
pub struct EventBusFactoryImpl;

#[async_trait]
impl EventBusFactory for EventBusFactoryImpl {
    async fn create(
        &self,
        config: EventBusConfig,
    ) -> Result<Box<dyn EventBusService>, EventSystemError> {
        if config.channel_capacity < MIN_CHANNEL_CAPACITY {
            return Err(EventSystemError::CapacityTooLow {
                capacity: config.channel_capacity,
                minimum: MIN_CHANNEL_CAPACITY,
            });
        }

        if config.buffer_capacity < MIN_BUFFER_CAPACITY {
            return Err(EventSystemError::CapacityTooLow {
                capacity: config.buffer_capacity,
                minimum: MIN_BUFFER_CAPACITY,
            });
        }

        Ok(Box::new(EventBusServiceImpl::new(config)))
    }

    async fn create_default(&self) -> Result<Box<dyn EventBusService>, EventSystemError> {
        Ok(Box::new(EventBusServiceImpl::default()))
    }

    async fn create_with_capacity(
        &self,
        channel_capacity: usize,
    ) -> Result<Box<dyn EventBusService>, EventSystemError> {
        if channel_capacity < MIN_CHANNEL_CAPACITY {
            return Err(EventSystemError::CapacityTooLow {
                capacity: channel_capacity,
                minimum: MIN_CHANNEL_CAPACITY,
            });
        }

        Ok(Box::new(EventBusServiceImpl::new(EventBusConfig {
            channel_capacity,
            buffer_capacity: MIN_BUFFER_CAPACITY * 10,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_default() {
        let factory = EventBusFactoryImpl;
        let bus = factory.create_default().await.unwrap();
        let status = bus
            .event_count()
            .await
            .unwrap();
        assert_eq!(status.total, 0);
    }

    #[tokio::test]
    async fn test_create_with_config() {
        let factory = EventBusFactoryImpl;
        let bus = factory
            .create(EventBusConfig {
                channel_capacity: 256,
                buffer_capacity: 1024,
            })
            .await
            .unwrap();
        // Bus should be functional
        let publish_result = bus
            .publish(super::super::dto::PublishEventInput {
                event: crate::event_system::domain::ExecutionEvent::ExecutionCompleted {
                    execution_id: uuid::Uuid::new_v4(),
                    duration_ms: 100,
                    nodes_executed: 3,
                    timestamp: chrono::Utc::now(),
                },
            })
            .await;
        assert!(publish_result.is_ok());
    }

    #[tokio::test]
    async fn test_create_with_capacity() {
        let factory = EventBusFactoryImpl;
        let bus = factory.create_with_capacity(500).await.unwrap();
        let status = bus
            .status(super::super::dto::EventBusStatusInput {
                include_subscriber_details: false,
            })
            .await
            .unwrap();
        assert_eq!(status.channel_capacity, 500);
    }

    #[tokio::test]
    async fn test_create_rejects_low_channel_capacity() {
        let factory = EventBusFactoryImpl;
        let result = factory.create_with_capacity(4).await;
        match result {
            Err(EventSystemError::CapacityTooLow { capacity, minimum }) => {
                assert_eq!(capacity, 4);
                assert_eq!(minimum, 16);
            }
            _ => panic!("Expected CapacityTooLow error"),
        }
    }

    #[tokio::test]
    async fn test_create_rejects_low_buffer_capacity() {
        let factory = EventBusFactoryImpl;
        let result = factory
            .create(EventBusConfig {
                channel_capacity: 100,
                buffer_capacity: 10,
            })
            .await;
        match result {
            Err(EventSystemError::CapacityTooLow { capacity, minimum }) => {
                assert_eq!(capacity, 10);
                assert_eq!(minimum, 64);
            }
            _ => panic!("Expected CapacityTooLow error"),
        }
    }
}
