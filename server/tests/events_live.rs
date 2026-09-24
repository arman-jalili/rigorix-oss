//! Live `GET /events` wiring (#900 / ADR-0001 D8) — engine event bus → hub.
//!
//! Exercises the same seam the server's `main` composes: a real
//! `EventBusServiceImpl` bridged into an `EventHub`. A connected SSE subscriber
//! (modelled by `EventHub::subscribe`) receives the three catalog events live,
//! in publish order, with envelope event-ref payloads. Transport framing
//! (Last-Event-ID/heartbeat/backpressure) is covered by `src/events.rs`; this
//! suite covers the publisher that was missing.

use std::sync::Arc;
use std::time::Duration;

use rigorix_engine::event_system::application::dto::{EventBusConfig, PublishEventInput};
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::event_system::application::service::EventBusService;
use rigorix_engine::event_system::domain::ExecutionEvent;
use rigorix_server::event_bridge::spawn_event_bridge;
use rigorix_server::events::{EventHub, EventKind};
use serde_json::json;
use uuid::Uuid;

fn bus() -> Arc<dyn EventBusService> {
    Arc::new(EventBusServiceImpl::new(EventBusConfig::default()))
}

fn exec() -> Uuid {
    Uuid::new_v4()
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

async fn publish(bus: &Arc<dyn EventBusService>, event: ExecutionEvent) {
    bus.publish(PublishEventInput { event })
        .await
        .expect("publish");
}

#[tokio::test]
async fn run_progress_is_delivered_live_and_in_order() {
    let bus = bus();
    let hub = Arc::new(EventHub::new());
    let _bridge = spawn_event_bridge(Arc::clone(&bus), Arc::clone(&hub));
    tokio::task::yield_now().await;
    let (_, mut rx) = hub.subscribe(None);

    let id = exec();
    let ts = now();
    publish(
        &bus,
        ExecutionEvent::PlanningStarted {
            execution_id: id,
            intent: "do the thing".into(),
            timestamp: ts,
        },
    )
    .await;
    publish(
        &bus,
        ExecutionEvent::NodeStarted {
            execution_id: id,
            node_id: "n1".into(),
            node_name: "validate".into(),
            timestamp: ts,
        },
    )
    .await;
    publish(
        &bus,
        ExecutionEvent::NodeCompleted {
            execution_id: id,
            node_id: "n1".into(),
            node_name: "validate".into(),
            duration_ms: 4,
            output: json!({}),
            timestamp: ts,
        },
    )
    .await;
    publish(
        &bus,
        ExecutionEvent::ExecutionCompleted {
            execution_id: id,
            duration_ms: 42,
            nodes_executed: 1,
            timestamp: ts,
        },
    )
    .await;

    let mut seen = Vec::new();
    for _ in 0..4 {
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("live event within timeout")
            .expect("recv");
        assert_eq!(event.event, EventKind::RunProgress);
        seen.push(event.data["event_type"].as_str().unwrap().to_string());
        // Envelope event-ref shape (never a second dialect).
        assert!(event.data.get("occurred_at").is_some());
        assert!(event.data.get("status").is_some());
    }
    assert_eq!(
        seen,
        vec![
            "planning_started",
            "node_started",
            "node_completed",
            "execution_completed"
        ],
        "per-execution order is preserved"
    );
}

#[tokio::test]
async fn approval_pause_and_policy_denial_are_delivered() {
    let bus = bus();
    let hub = Arc::new(EventHub::new());
    let _bridge = spawn_event_bridge(Arc::clone(&bus), Arc::clone(&hub));
    tokio::task::yield_now().await;
    let (_, mut rx) = hub.subscribe(None);

    let id = exec();
    let ts = now();
    publish(
        &bus,
        ExecutionEvent::SequencePolicyDenied {
            execution_id: id,
            rule_id: "no-remove-then-add".into(),
            later_step: "registration_add".into(),
            reason: "remove-then-add".into(),
            timestamp: ts,
        },
    )
    .await;
    publish(
        &bus,
        ExecutionEvent::ApprovalRecorded {
            execution_id: id,
            node_id: "n9".into(),
            step_name: "payout".into(),
            intent_hash: "abc".into(),
            approver_id: "operator@corp".into(),
            authority: Some("role:operator".into()),
            decided_at: ts,
            decision_context_ref: None,
            timestamp: ts,
        },
    )
    .await;

    let policy = rx.recv().await.expect("recv");
    assert_eq!(policy.event, EventKind::PolicyChanged);
    assert_eq!(policy.data["event_type"], "sequence_policy_denied");
    assert_eq!(policy.data["payload"]["rule_id"], "no-remove-then-add");
    assert_eq!(policy.data["payload"]["step_name"], "registration_add");

    let approval = rx.recv().await.expect("recv");
    assert_eq!(approval.event, EventKind::ApprovalRequired);
    assert_eq!(approval.data["event_type"], "approval_recorded");
    assert_eq!(approval.data["payload"]["step_name"], "payout");
}

/// AC #3: a slow/absent SSE consumer never back-pressures execution. Publishing
/// a burst into the engine bus completes promptly even though nothing reads the
/// hub; the hub's own broadcast is bounded (drops, never stalls).
#[tokio::test]
async fn slow_subscriber_does_not_stall_execution() {
    let bus = bus();
    // Tiny hub broadcast so a non-reading subscriber would lag quickly.
    let hub = Arc::new(EventHub::with_capacity(4, 2));
    let _bridge = spawn_event_bridge(Arc::clone(&bus), Arc::clone(&hub));
    let (_replay, _slow_subscriber) = hub.subscribe(None); // never read
    tokio::task::yield_now().await;

    let id = exec();
    let ts = now();
    let publish_all = async {
        for i in 0..500 {
            publish(
                &bus,
                ExecutionEvent::NodeCompleted {
                    execution_id: id,
                    node_id: format!("n{i}"),
                    node_name: format!("step{i}"),
                    duration_ms: 1,
                    output: json!({}),
                    timestamp: ts,
                },
            )
            .await;
        }
    };
    // If a slow SSE consumer could stall publishing, this would time out.
    tokio::time::timeout(Duration::from_secs(5), publish_all)
        .await
        .expect("publishing must not stall on a slow subscriber");
}
