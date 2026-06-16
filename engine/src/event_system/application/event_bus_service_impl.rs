//! Implementation of `EventBusService`.
//!
//! @canonical .pi/architecture/modules/event-system.md#bus
//! Implements: EventBusService trait — pub-sub with synchronous in-memory persistence
//! Issue: #47
//!
//! Central pub-sub event bus backed by `tokio::sync::broadcast` for real-time
//! delivery and `Arc<Mutex<Vec<PersistedEvent>>>` for synchronous in-memory
//! persistence. Provides monotonic sequence numbers for exact replay ordering.

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::event_system::domain::{EventSystemError, ExecutionEvent, PersistedEvent};

use super::dto::{
    DrainPersistedInput, DrainPersistedOutput, EventBusConfig, EventBusStatus, EventBusStatusInput,
    EventCountOutput, PublishEventInput, PublishEventOutput, QueryEventsInput, QueryEventsOutput,
    SubscribeInput, SubscribeOutput,
};
use super::service::EventBusService;

/// Implementation of `EventBusService`.
///
/// Provides the central pub-sub event bus with:
/// - Real-time event delivery via tokio broadcast channel
/// - Synchronous in-memory persistence with monotonic sequence numbers
/// - Drain-at-end support for ExecutionRecord population
/// - Query/filter capabilities on persisted events
pub struct EventBusServiceImpl {
    /// Tokio broadcast sender for real-time event delivery to subscribers.
    sender: broadcast::Sender<ExecutionEvent>,

    /// In-memory persisted event buffer with synchronous Mutex locking.
    persisted: Arc<Mutex<Vec<PersistedEvent>>>,

    /// Monotonically increasing sequence counter (1-indexed).
    sequence: AtomicU64,

    /// Number of events drained from the buffer (for statistics).
    drained_count: AtomicU64,

    /// Whether the buffer has been drained (prevents double-drain).
    drained: AtomicBool,

    /// Configuration for capacity limits.
    config: EventBusConfig,
}

impl EventBusServiceImpl {
    /// Create a new EventBus service with the given configuration.
    pub fn new(config: EventBusConfig) -> Self {
        let (sender, _) = broadcast::channel(config.channel_capacity);

        Self {
            sender,
            persisted: Arc::new(Mutex::new(Vec::with_capacity(config.buffer_capacity))),
            sequence: AtomicU64::new(0),
            drained_count: AtomicU64::new(0),
            drained: AtomicBool::new(false),
            config,
        }
    }

    /// Create a new EventBus service with default configuration.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(EventBusConfig::default())
    }

    /// Get the next monotonic sequence number.
    #[tracing::instrument(skip_all)]
    fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get the current sequence number without incrementing.
    #[tracing::instrument(skip_all)]
    fn current_sequence(&self) -> u64 {
        self.sequence.load(Ordering::SeqCst)
    }

    /// Subscribe to the broadcast channel without registering a named subscriber.
    ///
    /// Returns a receiver and the count of active subscribers.
    #[tracing::instrument(skip_all)]
    fn raw_subscribe(&self) -> (broadcast::Receiver<ExecutionEvent>, usize) {
        let rx = self.sender.subscribe();
        let count = self.sender.receiver_count();
        (rx, count)
    }
}

#[async_trait]
impl EventBusService for EventBusServiceImpl {
    async fn publish(
        &self,
        input: PublishEventInput,
    ) -> Result<PublishEventOutput, EventSystemError> {
        let sequence = self.next_sequence();

        // Build the persisted event wrapper
        let persisted = PersistedEvent {
            sequence,
            event: input.event.clone(),
        };

        // Synchronously persist to in-memory buffer
        {
            let mut buffer = self.persisted.lock().await;
            if buffer.len() >= self.config.buffer_capacity {
                // Buffer full — remove oldest event to make room
                buffer.remove(0);
            }
            buffer.push(persisted);
        }

        // Broadcast to all active subscribers (non-blocking)
        let subscriber_count = self.sender.receiver_count();
        let send_result = self.sender.send(input.event);
        let had_laggers = send_result.is_err();

        Ok(PublishEventOutput {
            sequence,
            subscriber_count,
            had_laggers,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn subscribe(&self, input: SubscribeInput) -> Result<SubscribeOutput, EventSystemError> {
        let subscriber_name = input
            .subscriber_name
            .unwrap_or_else(|| format!("subscriber-{}", uuid::Uuid::new_v4()));

        let (_rx, active_count) = self.raw_subscribe();

        Ok(SubscribeOutput {
            success: true,
            subscriber_name,
            active_subscriber_count: active_count,
        })
    }

    async fn drain_persisted(
        &self,
        input: DrainPersistedInput,
    ) -> Result<DrainPersistedOutput, EventSystemError> {
        // Check if already drained
        if self.drained.load(Ordering::SeqCst) {
            return Err(EventSystemError::AlreadyDrained {
                count: self.drained_count.load(Ordering::SeqCst),
            });
        }

        let mut buffer = self.persisted.lock().await;
        let events = if input.clear {
            let events = buffer.clone();
            buffer.clear();
            self.drained.store(true, Ordering::SeqCst);
            events
        } else {
            buffer.clone()
        };

        let count = events.len() as u64;
        self.drained_count.store(count, Ordering::SeqCst);

        Ok(DrainPersistedOutput {
            events,
            count,
            cleared: input.clear,
        })
    }

    async fn query_events(
        &self,
        input: QueryEventsInput,
    ) -> Result<QueryEventsOutput, EventSystemError> {
        let buffer = self.persisted.lock().await;

        let filtered: Vec<PersistedEvent> = buffer
            .iter()
            .filter(|pe| {
                // Filter by execution_id
                if let Some(eid) = &input.execution_id {
                    match &pe.event {
                        ExecutionEvent::PlanningStarted { execution_id, .. }
                        | ExecutionEvent::PlanningCompleted { execution_id, .. }
                        | ExecutionEvent::NodeStarted { execution_id, .. }
                        | ExecutionEvent::NodeCompleted { execution_id, .. }
                        | ExecutionEvent::NodeFailed { execution_id, .. }
                        | ExecutionEvent::NodeRetrying { execution_id, .. }
                        | ExecutionEvent::ToolExecuted { execution_id, .. }
                        | ExecutionEvent::ExecutionCompleted { execution_id, .. }
                        | ExecutionEvent::ExecutionFailed { execution_id, .. }
                        | ExecutionEvent::ExecutionCancelled { execution_id, .. }
                        | ExecutionEvent::BudgetWarning { execution_id, .. } => {
                            if execution_id != eid {
                                return false;
                            }
                        }
                    }
                }

                true
            })
            .filter(|pe| {
                // Filter by sequence
                if let Some(after) = input.after_sequence
                    && pe.sequence <= after {
                        return false;
                    }
                true
            })
            .filter(|pe| {
                // Filter by event type (variant tag)
                if let Some(ref event_type) = input.event_type {
                    let variant_name = match &pe.event {
                        ExecutionEvent::PlanningStarted { .. } => "planning_started",
                        ExecutionEvent::PlanningCompleted { .. } => "planning_completed",
                        ExecutionEvent::NodeStarted { .. } => "node_started",
                        ExecutionEvent::NodeCompleted { .. } => "node_completed",
                        ExecutionEvent::NodeFailed { .. } => "node_failed",
                        ExecutionEvent::NodeRetrying { .. } => "node_retrying",
                        ExecutionEvent::ToolExecuted { .. } => "tool_executed",
                        ExecutionEvent::ExecutionCompleted { .. } => "execution_completed",
                        ExecutionEvent::ExecutionFailed { .. } => "execution_failed",
                        ExecutionEvent::ExecutionCancelled { .. } => "execution_cancelled",
                        ExecutionEvent::BudgetWarning { .. } => "budget_warning",
                    };
                    if variant_name != event_type {
                        return false;
                    }
                }
                true
            })
            .filter(|pe| {
                // Filter by timestamp range
                if let Some(after) = &input.after_timestamp {
                    let ts = match &pe.event {
                        ExecutionEvent::PlanningStarted { timestamp, .. } => timestamp,
                        ExecutionEvent::PlanningCompleted { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeStarted { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeCompleted { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeFailed { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeRetrying { timestamp, .. } => timestamp,
                        ExecutionEvent::ToolExecuted { timestamp, .. } => timestamp,
                        ExecutionEvent::ExecutionCompleted { timestamp, .. } => timestamp,
                        ExecutionEvent::ExecutionFailed { timestamp, .. } => timestamp,
                        ExecutionEvent::ExecutionCancelled { timestamp, .. } => timestamp,
                        ExecutionEvent::BudgetWarning { timestamp, .. } => timestamp,
                    };
                    if ts < after {
                        return false;
                    }
                }
                true
            })
            .filter(|pe| {
                if let Some(before) = &input.before_timestamp {
                    let ts = match &pe.event {
                        ExecutionEvent::PlanningStarted { timestamp, .. } => timestamp,
                        ExecutionEvent::PlanningCompleted { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeStarted { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeCompleted { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeFailed { timestamp, .. } => timestamp,
                        ExecutionEvent::NodeRetrying { timestamp, .. } => timestamp,
                        ExecutionEvent::ToolExecuted { timestamp, .. } => timestamp,
                        ExecutionEvent::ExecutionCompleted { timestamp, .. } => timestamp,
                        ExecutionEvent::ExecutionFailed { timestamp, .. } => timestamp,
                        ExecutionEvent::ExecutionCancelled { timestamp, .. } => timestamp,
                        ExecutionEvent::BudgetWarning { timestamp, .. } => timestamp,
                    };
                    if ts > before {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let limit = input.limit.unwrap_or(100) as usize;
        let total = filtered.len() as u64;
        let has_more = filtered.len() > limit;
        let events = filtered.into_iter().take(limit).collect();

        Ok(QueryEventsOutput {
            events,
            total,
            has_more,
        })
    }

    async fn status(
        &self,
        _input: EventBusStatusInput,
    ) -> Result<EventBusStatus, EventSystemError> {
        let buffer = self.persisted.lock().await;
        Ok(EventBusStatus {
            persisted_count: buffer.len() as u64,
            current_sequence: self.current_sequence(),
            active_subscriber_count: self.sender.receiver_count(),
            channel_capacity: self.config.channel_capacity,
            buffer_capacity: self.config.buffer_capacity,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn event_count(&self) -> Result<EventCountOutput, EventSystemError> {
        let buffer = self.persisted.lock().await;
        Ok(EventCountOutput {
            total: self.current_sequence(),
            persisted: buffer.len() as u64,
            drained: self.drained_count.load(Ordering::SeqCst),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // Helper to create a sample event for testing
    #[tracing::instrument(skip_all)]
    fn sample_event(execution_id: uuid::Uuid) -> ExecutionEvent {
        ExecutionEvent::NodeStarted {
            execution_id,
            node_id: "node-1".to_string(),
            node_name: "Test Node".to_string(),
            timestamp: Utc::now(),
        }
    }

    #[tracing::instrument(skip_all)]
    fn sample_event_completed(execution_id: uuid::Uuid) -> ExecutionEvent {
        ExecutionEvent::ExecutionCompleted {
            execution_id,
            duration_ms: 1000,
            nodes_executed: 5,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_publish_increments_sequence() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        let result = bus
            .publish(PublishEventInput {
                event: sample_event(eid),
            })
            .await
            .unwrap();

        assert_eq!(result.sequence, 1);

        let result2 = bus
            .publish(PublishEventInput {
                event: sample_event(eid),
            })
            .await
            .unwrap();

        assert_eq!(result2.sequence, 2);
    }

    #[tokio::test]
    async fn test_publish_persists_event() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        let status = bus
            .status(EventBusStatusInput {
                include_subscriber_details: false,
            })
            .await
            .unwrap();
        assert_eq!(status.persisted_count, 1);
    }

    #[tokio::test]
    async fn test_drain_persisted_returns_events_in_order() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        bus.publish(PublishEventInput {
            event: sample_event_completed(eid),
        })
        .await
        .unwrap();

        let output = bus
            .drain_persisted(DrainPersistedInput { clear: true })
            .await
            .unwrap();

        assert_eq!(output.count, 2);
        assert_eq!(output.events[0].sequence, 1);
        assert_eq!(output.events[1].sequence, 2);
        assert!(output.cleared);
    }

    #[tokio::test]
    async fn test_drain_clears_buffer() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        bus.drain_persisted(DrainPersistedInput { clear: true })
            .await
            .unwrap();

        let status = bus
            .status(EventBusStatusInput {
                include_subscriber_details: false,
            })
            .await
            .unwrap();
        assert_eq!(status.persisted_count, 0);
    }

    #[tokio::test]
    async fn test_double_drain_returns_error() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        bus.drain_persisted(DrainPersistedInput { clear: true })
            .await
            .unwrap();

        let result = bus
            .drain_persisted(DrainPersistedInput { clear: true })
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EventSystemError::AlreadyDrained { .. }
        ));
    }

    #[tokio::test]
    async fn test_subscriber_receives_event() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        // Subscribe before publishing
        let sub = bus
            .subscribe(SubscribeInput {
                subscriber_name: Some("test-sub".to_string()),
            })
            .await
            .unwrap();
        assert!(sub.success);

        // Get a receiver from the broadcast channel directly
        let mut rx = bus.sender.subscribe();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        // Subscriber should receive the event
        let received = rx.recv().await.unwrap();
        match received {
            ExecutionEvent::NodeStarted { execution_id, .. } => {
                assert_eq!(execution_id, eid);
            }
            _ => panic!("Expected NodeStarted event"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers_all_receive() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        let mut rx1 = bus.sender.subscribe();
        let mut rx2 = bus.sender.subscribe();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        // Both subscribers receive the event
        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_query_events_by_execution_id() {
        let bus = EventBusServiceImpl::default();
        let eid1 = uuid::Uuid::new_v4();
        let eid2 = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid1),
        })
        .await
        .unwrap();

        bus.publish(PublishEventInput {
            event: sample_event(eid2),
        })
        .await
        .unwrap();

        let query = QueryEventsInput {
            execution_id: Some(eid1),
            event_type: None,
            after_sequence: None,
            limit: None,
            after_timestamp: None,
            before_timestamp: None,
        };

        let result = bus.query_events(query).await.unwrap();
        assert_eq!(result.total, 1);
    }

    #[tokio::test]
    async fn test_query_events_by_type() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        bus.publish(PublishEventInput {
            event: sample_event_completed(eid),
        })
        .await
        .unwrap();

        let query = QueryEventsInput {
            execution_id: None,
            event_type: Some("node_started".to_string()),
            after_sequence: None,
            limit: None,
            after_timestamp: None,
            before_timestamp: None,
        };

        let result = bus.query_events(query).await.unwrap();
        assert_eq!(result.total, 1);

        let first = &result.events[0];
        match &first.event {
            ExecutionEvent::NodeStarted { .. } => {} // expected
            _ => panic!("Expected NodeStarted event"),
        }
    }

    #[tokio::test]
    async fn test_query_events_after_sequence() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        bus.publish(PublishEventInput {
            event: sample_event_completed(eid),
        })
        .await
        .unwrap();

        let query = QueryEventsInput {
            execution_id: None,
            event_type: None,
            after_sequence: Some(1),
            limit: None,
            after_timestamp: None,
            before_timestamp: None,
        };

        let result = bus.query_events(query).await.unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.events[0].sequence, 2);
    }

    #[tokio::test]
    async fn test_event_count() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        let counts = bus.event_count().await.unwrap();
        assert_eq!(counts.total, 1);
        assert_eq!(counts.persisted, 1);
        assert_eq!(counts.drained, 0);

        bus.drain_persisted(DrainPersistedInput { clear: true })
            .await
            .unwrap();

        let counts = bus.event_count().await.unwrap();
        assert_eq!(counts.total, 1);
        assert_eq!(counts.persisted, 0);
        assert_eq!(counts.drained, 1);
    }

    #[tokio::test]
    async fn test_status_reports_correctly() {
        let bus = EventBusServiceImpl::new(EventBusConfig {
            channel_capacity: 500,
            buffer_capacity: 5000,
        });

        let status = bus
            .status(EventBusStatusInput {
                include_subscriber_details: false,
            })
            .await
            .unwrap();

        assert_eq!(status.persisted_count, 0);
        assert_eq!(status.channel_capacity, 500);
        assert_eq!(status.buffer_capacity, 5000);
    }

    #[tokio::test]
    async fn test_publish_with_no_subscribers_succeeds() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        // No subscribers — event should still be persisted
        let result = bus
            .publish(PublishEventInput {
                event: sample_event(eid),
            })
            .await
            .unwrap();

        assert_eq!(result.subscriber_count, 0);
        assert_eq!(result.sequence, 1);

        let status = bus
            .status(EventBusStatusInput {
                include_subscriber_details: false,
            })
            .await
            .unwrap();
        assert_eq!(status.persisted_count, 1);
    }

    #[tokio::test]
    async fn test_drain_without_clear_keeps_events() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        bus.publish(PublishEventInput {
            event: sample_event(eid),
        })
        .await
        .unwrap();

        // Drain without clearing
        let output = bus
            .drain_persisted(DrainPersistedInput { clear: false })
            .await
            .unwrap();
        assert_eq!(output.count, 1);
        assert!(!output.cleared);

        // Events should still be in buffer
        let status = bus
            .status(EventBusStatusInput {
                include_subscriber_details: false,
            })
            .await
            .unwrap();
        assert_eq!(status.persisted_count, 1);
    }

    #[tokio::test]
    async fn test_query_events_limit() {
        let bus = EventBusServiceImpl::default();
        let eid = uuid::Uuid::new_v4();

        // Publish 3 events
        for _ in 0..3 {
            bus.publish(PublishEventInput {
                event: sample_event(eid),
            })
            .await
            .unwrap();
        }

        let query = QueryEventsInput {
            execution_id: None,
            event_type: None,
            after_sequence: None,
            limit: Some(2),
            after_timestamp: None,
            before_timestamp: None,
        };

        let result = bus.query_events(query).await.unwrap();
        assert_eq!(result.events.len(), 2);
        assert!(result.has_more);
    }
}
