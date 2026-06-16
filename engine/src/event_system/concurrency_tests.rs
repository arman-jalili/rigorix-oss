//! Concurrent-safety tests for the event system module.
//!
//! Exercises concurrent publish patterns to verify the broadcast channel
//! and persisted buffer handle contention correctly.

#[cfg(test)]
mod tests {
    use crate::event_system::application::dto::{DrainPersistedInput, PublishEventInput};
    use crate::event_system::application::event_bus_factory_impl::EventBusFactoryImpl;
    use crate::event_system::application::factory::EventBusFactory;
    use crate::event_system::domain::ExecutionEvent;
    use uuid::Uuid;

    fn make_event(execution_id: Uuid, label: &str) -> ExecutionEvent {
        ExecutionEvent::PlanningStarted {
            execution_id,
            intent: format!("test: {}", label),
            timestamp: chrono::Utc::now(),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_publish_no_panic() {
        let factory = EventBusFactoryImpl;
        let _bus = factory.create_default().await.unwrap();
        let exec_id = Uuid::new_v4();
        let num_publishers = 5;
        let events_per_publisher = 20;

        let mut handles = Vec::new();
        for i in 0..num_publishers {
            handles.push(tokio::spawn(async move {
                let factory = EventBusFactoryImpl;
                let bus = factory.create_default().await.unwrap();
                for j in 0..events_per_publisher {
                    let event = make_event(exec_id, &format!("pub-{}-{}", i, j));
                    bus.publish(PublishEventInput { event }).await.unwrap();
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_publish_and_drain_consistency() {
        let factory = EventBusFactoryImpl;
        let bus = factory.create_default().await.unwrap();
        let exec_id = Uuid::new_v4();

        for i in 0..10 {
            let event = make_event(exec_id, &format!("batch-{}", i));
            bus.publish(PublishEventInput { event }).await.unwrap();
        }

        let drained = bus
            .drain_persisted(DrainPersistedInput { clear: true })
            .await
            .unwrap();
        assert_eq!(drained.events.len(), 10, "Should drain all 10 events");
        assert_eq!(drained.count, 10);
    }
}
