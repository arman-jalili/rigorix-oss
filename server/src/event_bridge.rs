//! Engine event bus → server `EventHub` bridge (ADR-0001 D8, #900).
//!
//! @canonical .pi/architecture/modules/rigorix-server.md#push-channel-get-events-adr-0001-d8
//!
//! The `GET /events` SSE transport (#894) is only useful if something publishes
//! into it. This module subscribes to the engine [`EventBusService`] the host
//! composition already builds (the SAME bus the orchestrator/executor publish
//! into — never a second source) and forwards each run-relevant
//! [`ExecutionEvent`] into the server's [`EventHub`].
//!
//! [ADR-0001 D8] fixes exactly three push names; the variant → name mapping is
//! **complete and frozen** in `schemas/api/events.json` (`x-engine-event-mapping`)
//! — a variant that maps to `null` (audit-delivery/circuit-breaker events) is
//! intentionally not pushed. [`kind_for`] is an exhaustive match, so adding an
//! engine variant is a compile error here until it is classified.
//!
//! Payloads **reuse the envelope event-ref shape** ([`ExecutionEventRef`]:
//! `event_type`, `summary`, `occurred_at`, `status`, `payload`); there is no
//! second dialect. Parameter values are never pushed (SpanPrivacy) — only step /
//! node / rule identities.
//!
//! Backpressure mirrors the engine: the bridge is a normal broadcast subscriber,
//! so a slow bridge lags (dropped events are logged, never a stall); the hub's
//! own broadcast is bounded too. Execution is never blocked by a slow SSE client.
//!
//! [ADR-0001 D8]: https://github.com/arman-jalili/rigorix-sdk/blob/main/docs/adr/ADR-0001-native-api-catalog.md

use std::sync::Arc;

use rigorix_engine::audit::domain::{EventStatus, ExecutionEventRef};
use rigorix_engine::event_system::application::service::EventBusService;
use rigorix_engine::event_system::domain::ExecutionEvent;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::events::{EventHub, EventKind};

/// Map an engine event to one of the three ADR-0001 D8 push names.
///
/// Exhaustive over [`ExecutionEvent`] — this is the `x-engine-event-mapping`
/// table from `schemas/api/events.json`. `None` = intentionally not pushed
/// (audit-delivery + circuit-breaker events).
pub fn kind_for(event: &ExecutionEvent) -> Option<EventKind> {
    match event {
        // ── run_progress ────────────────────────────────────────────────
        ExecutionEvent::PlanningStarted { .. }
        | ExecutionEvent::PlanningCompleted { .. }
        | ExecutionEvent::NodeStarted { .. }
        | ExecutionEvent::NodeCompleted { .. }
        | ExecutionEvent::NodeFailed { .. }
        | ExecutionEvent::NodeRetrying { .. }
        | ExecutionEvent::ToolExecuted { .. }
        | ExecutionEvent::ExecutionCompleted { .. }
        | ExecutionEvent::ExecutionFailed { .. }
        | ExecutionEvent::ExecutionCancelled { .. }
        | ExecutionEvent::BudgetWarning { .. } => Some(EventKind::RunProgress),

        // ── approval_required ───────────────────────────────────────────
        ExecutionEvent::ApprovalRecorded { .. } | ExecutionEvent::IntentMismatchDetected { .. } => {
            Some(EventKind::ApprovalRequired)
        }

        // ── policy_changed ──────────────────────────────────────────────
        ExecutionEvent::SequenceRuleMatched { .. }
        | ExecutionEvent::SequencePolicyDenied { .. }
        | ExecutionEvent::SequencePolicyConfigError { .. }
        | ExecutionEvent::RequirementUnmet { .. }
        | ExecutionEvent::RequirementPromoted { .. }
        | ExecutionEvent::ScopeViolationRecorded { .. } => Some(EventKind::PolicyChanged),

        // ── not pushed (audit delivery / circuit breaker) ───────────────
        ExecutionEvent::AuditEnvelopeDelivered { .. }
        | ExecutionEvent::AuditEnvelopeQueued { .. }
        | ExecutionEvent::AuditEnvelopeDropped { .. }
        | ExecutionEvent::AuditEnvelopeCreated { .. }
        | ExecutionEvent::CircuitBreakerStateChanged { .. } => None,
    }
}

/// Project an engine event into the envelope event-ref shape pushed as `data`.
///
/// `summary` and `payload` carry identities (step/node/rule), never parameter
/// values.
pub fn to_event_ref(event: &ExecutionEvent) -> ExecutionEventRef {
    let (status, summary, payload) = describe(event);
    ExecutionEventRef {
        event_type: event.event_type_name().to_string(),
        summary,
        occurred_at: event.timestamp().to_owned(),
        correlation_id: Some(*event.execution_id()),
        status,
        payload: Some(payload),
    }
}

/// `(status, human summary, redacted structured payload)` for one event.
fn describe(event: &ExecutionEvent) -> (EventStatus, String, Value) {
    match event {
        ExecutionEvent::PlanningStarted { .. } => (
            EventStatus::Success,
            "execution planning started".to_string(),
            json!({}),
        ),
        ExecutionEvent::PlanningCompleted { template_id, .. } => (
            EventStatus::Success,
            format!("planning completed: template '{template_id}'"),
            json!({ "template_id": template_id }),
        ),
        ExecutionEvent::NodeStarted {
            node_id, node_name, ..
        } => (
            EventStatus::Success,
            format!("step '{node_name}' started"),
            json!({ "step_name": node_name, "node_id": node_id }),
        ),
        ExecutionEvent::NodeCompleted {
            node_id, node_name, ..
        } => (
            EventStatus::Success,
            format!("step '{node_name}' completed"),
            json!({ "step_name": node_name, "node_id": node_id }),
        ),
        ExecutionEvent::NodeFailed {
            node_id, attempt, ..
        } => (
            EventStatus::Failure,
            format!("node '{node_id}' failed (attempt {attempt})"),
            json!({ "node_id": node_id, "attempt": attempt }),
        ),
        ExecutionEvent::NodeRetrying {
            node_id, attempt, ..
        } => (
            EventStatus::Skipped,
            format!("node '{node_id}' retrying (attempt {attempt})"),
            json!({ "node_id": node_id, "attempt": attempt }),
        ),
        ExecutionEvent::ToolExecuted {
            node_id,
            tool,
            skipped,
            ..
        } => (
            if *skipped {
                EventStatus::Skipped
            } else {
                EventStatus::Success
            },
            if *skipped {
                format!("tool '{tool}' skipped by policy")
            } else {
                format!("tool '{tool}' executed")
            },
            json!({ "node_id": node_id, "tool": tool, "skipped": skipped }),
        ),
        ExecutionEvent::ExecutionCompleted {
            duration_ms,
            nodes_executed,
            ..
        } => (
            EventStatus::Success,
            format!("execution completed ({nodes_executed} nodes, {duration_ms} ms)"),
            json!({ "nodes_executed": nodes_executed, "duration_ms": duration_ms }),
        ),
        ExecutionEvent::ExecutionFailed { .. } => (
            EventStatus::Failure,
            "execution failed".to_string(),
            json!({}),
        ),
        ExecutionEvent::ExecutionCancelled { .. } => (
            EventStatus::Cancelled,
            "execution cancelled".to_string(),
            json!({}),
        ),
        ExecutionEvent::BudgetWarning {
            resource,
            used,
            limit,
            ..
        } => (
            EventStatus::Success,
            format!("budget warning: {resource} {used}/{limit}"),
            json!({ "resource": resource, "used": used, "limit": limit }),
        ),

        // ── approval_required ───────────────────────────────────────────
        ExecutionEvent::ApprovalRecorded {
            node_id, step_name, ..
        } => (
            EventStatus::Success,
            format!("approval recorded for step '{step_name}'"),
            json!({ "step_name": step_name, "node_id": node_id }),
        ),
        ExecutionEvent::IntentMismatchDetected {
            node_id, step_name, ..
        } => (
            EventStatus::Failure,
            format!("intent mismatch on step '{step_name}' — re-approval required"),
            json!({ "step_name": step_name, "node_id": node_id }),
        ),

        // ── policy_changed ──────────────────────────────────────────────
        ExecutionEvent::ScopeViolationRecorded {
            node_id,
            step_name,
            out_of_scope,
            ..
        } => (
            EventStatus::Failure,
            format!("scope violation on step '{step_name}'"),
            json!({ "step_name": step_name, "node_id": node_id, "out_of_scope": out_of_scope }),
        ),
        ExecutionEvent::SequenceRuleMatched {
            rule_id,
            action,
            later_step,
            ..
        } => (
            EventStatus::Skipped,
            format!("sequence rule '{rule_id}' {action} step '{later_step}'"),
            json!({ "rule_id": rule_id, "action": action, "step_name": later_step }),
        ),
        ExecutionEvent::SequencePolicyDenied {
            rule_id,
            later_step,
            ..
        } => (
            EventStatus::Failure,
            format!("sequence rule '{rule_id}' denied step '{later_step}'"),
            json!({ "rule_id": rule_id, "action": "deny", "step_name": later_step }),
        ),
        ExecutionEvent::SequencePolicyConfigError { .. } => (
            EventStatus::Failure,
            "sequence policy config error".to_string(),
            json!({}),
        ),
        ExecutionEvent::RequirementUnmet {
            requirement_id,
            step,
            unmet,
            ..
        } => (
            EventStatus::Failure,
            format!("requirement '{requirement_id}' unmet for step '{step}'"),
            json!({ "requirement_id": requirement_id, "action": "deny", "step_name": step, "unmet": unmet }),
        ),
        ExecutionEvent::RequirementPromoted {
            requirement_id,
            step,
            unmet,
            ..
        } => (
            EventStatus::Skipped,
            format!("requirement '{requirement_id}' promoted step '{step}'"),
            json!({ "requirement_id": requirement_id, "action": "promote", "step_name": step, "unmet": unmet }),
        ),

        // ── not pushed — describe conservatively for completeness ───────
        ExecutionEvent::AuditEnvelopeDelivered { .. }
        | ExecutionEvent::AuditEnvelopeQueued { .. }
        | ExecutionEvent::AuditEnvelopeDropped { .. }
        | ExecutionEvent::AuditEnvelopeCreated { .. }
        | ExecutionEvent::CircuitBreakerStateChanged { .. } => (
            EventStatus::Success,
            "audit delivery event".to_string(),
            json!({}),
        ),
    }
}

/// Serialize an engine event into the JSON `data` payload for the hub.
pub fn event_payload(event: &ExecutionEvent) -> Value {
    serde_json::to_value(to_event_ref(event)).unwrap_or(Value::Null)
}

/// Forward engine events into `hub` until the bus closes.
///
/// A lagged receiver (slow hub) drops the missed events and logs the gap —
/// execution is never back-pressured. Clients resync via `Last-Event-ID` /
/// `rigorix.audit.read`.
pub async fn forward(mut rx: broadcast::Receiver<ExecutionEvent>, hub: Arc<EventHub>) {
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Some(kind) = kind_for(&event) {
                    hub.publish(kind, event_payload(&event));
                }
            }
            Err(broadcast::error::RecvError::Lagged(dropped)) => {
                tracing::warn!(
                    dropped,
                    "server event bridge lagged — dropped SSE events; clients can replay via \
                     Last-Event-ID / rigorix.audit.read (mirrors AuditEnvelopeDropped)"
                );
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::debug!("engine event bus closed — server event bridge stopping");
                break;
            }
        }
    }
}

/// Subscribe to the engine bus and spawn the forwarding task.
///
/// Returns the join handle so the caller (composition root) can keep it alive
/// for the process lifetime.
pub fn spawn_event_bridge(
    bus: Arc<dyn EventBusService>,
    hub: Arc<EventHub>,
) -> tokio::task::JoinHandle<()> {
    let rx = bus.subscribe_receiver();
    tokio::spawn(forward(rx, hub))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rigorix_engine::event_system::application::dto::EventBusConfig;
    use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use uuid::Uuid;

    fn event(event_type: &str) -> ExecutionEvent {
        let execution_id = Uuid::new_v4();
        let timestamp = chrono::Utc::now();
        match event_type {
            "planning_started" => ExecutionEvent::PlanningStarted {
                execution_id,
                intent: "run".to_string(),
                timestamp,
            },
            "node_completed" => ExecutionEvent::NodeCompleted {
                execution_id,
                node_id: "n1".to_string(),
                node_name: "validate".to_string(),
                duration_ms: 5,
                output: json!({}),
                timestamp,
            },
            "approval_recorded" => ExecutionEvent::ApprovalRecorded {
                execution_id,
                node_id: "n2".to_string(),
                step_name: "deploy".to_string(),
                intent_hash: "abc".to_string(),
                approver_id: "user@org".to_string(),
                authority: None,
                decided_at: timestamp,
                decision_context_ref: None,
                timestamp,
            },
            "sequence_policy_denied" => ExecutionEvent::SequencePolicyDenied {
                execution_id,
                rule_id: "no-remove-then-add".to_string(),
                later_step: "add".to_string(),
                reason: "remove-then-add".to_string(),
                timestamp,
            },
            "audit_envelope_created" => ExecutionEvent::AuditEnvelopeCreated {
                execution_id,
                timestamp,
            },
            other => panic!("unhandled test event type {other}"),
        }
    }

    /// The mapping table is complete for the three D8 names (ADR-0001 D8 /
    /// `schemas/api/events.json` `x-engine-event-mapping`).
    #[test]
    fn mapping_matches_the_frozen_d8_table() {
        assert_eq!(
            kind_for(&event("planning_started")),
            Some(EventKind::RunProgress)
        );
        assert_eq!(
            kind_for(&event("node_completed")),
            Some(EventKind::RunProgress)
        );
        assert_eq!(
            kind_for(&event("approval_recorded")),
            Some(EventKind::ApprovalRequired)
        );
        assert_eq!(
            kind_for(&event("sequence_policy_denied")),
            Some(EventKind::PolicyChanged)
        );
        assert_eq!(kind_for(&event("audit_envelope_created")), None);
    }

    /// The pushed payload is the envelope event-ref shape (never a second
    /// dialect), with identities but no parameter values.
    #[test]
    fn payload_reuses_envelope_event_ref_shape() {
        let payload = serde_json::to_value(to_event_ref(&event("node_completed"))).unwrap();
        assert_eq!(payload["event_type"], "node_completed");
        assert_eq!(payload["status"], "Success");
        assert_eq!(payload["payload"]["step_name"], "validate");
        assert!(payload.get("occurred_at").is_some());
        assert!(payload.get("correlation_id").is_some());
    }

    /// A run through the real engine bus delivers a live `run_progress` event
    /// to a hub subscriber (AC #1) — and approval/policy events map (AC #2).
    #[tokio::test]
    async fn bridge_forwards_live_events_to_the_hub() {
        use rigorix_engine::event_system::application::dto::PublishEventInput;

        let bus: Arc<dyn EventBusService> =
            Arc::new(EventBusServiceImpl::new(EventBusConfig::default()));
        let hub = Arc::new(EventHub::new());
        let _bridge = spawn_event_bridge(Arc::clone(&bus), Arc::clone(&hub));
        // Give the spawned task a moment to subscribe.
        tokio::task::yield_now().await;

        let (_replay, mut rx) = hub.subscribe(None);

        bus.publish(PublishEventInput {
            event: event("node_completed"),
        })
        .await
        .expect("publish");
        bus.publish(PublishEventInput {
            event: event("approval_recorded"),
        })
        .await
        .expect("publish");
        bus.publish(PublishEventInput {
            event: event("sequence_policy_denied"),
        })
        .await
        .expect("publish");
        // Not pushed.
        bus.publish(PublishEventInput {
            event: event("audit_envelope_created"),
        })
        .await
        .expect("publish");

        let first = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("live event within timeout")
            .expect("recv");
        assert_eq!(first.event, EventKind::RunProgress);
        assert_eq!(first.data["event_type"], "node_completed");

        let second = rx.recv().await.expect("recv");
        assert_eq!(second.event, EventKind::ApprovalRequired);

        let third = rx.recv().await.expect("recv");
        assert_eq!(third.event, EventKind::PolicyChanged);
        assert_eq!(third.data["payload"]["rule_id"], "no-remove-then-add");

        // The audit-delivery event was not pushed (bounded, no spurious frame).
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
                .await
                .is_err(),
            "audit-delivery events must not be pushed"
        );
    }

    /// AC #5: publishing with no subscribers neither grows unboundedly nor
    /// panics — the hub keeps a bounded replay ring.
    #[tokio::test]
    async fn hub_without_subscribers_stays_bounded() {
        use rigorix_engine::event_system::application::dto::PublishEventInput;

        let bus: Arc<dyn EventBusService> =
            Arc::new(EventBusServiceImpl::new(EventBusConfig::default()));
        let hub = Arc::new(EventHub::with_capacity(4, 8));
        let _bridge = spawn_event_bridge(Arc::clone(&bus), Arc::clone(&hub));
        tokio::task::yield_now().await;

        for _ in 0..10 {
            bus.publish(PublishEventInput {
                event: event("node_completed"),
            })
            .await
            .expect("publish");
        }
        // Allow the bridge to drain into the hub.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(hub.subscriber_count(), 0, "no SSE subscribers");
        let (replay, _rx) = hub.subscribe(None);
        assert!(
            replay.len() <= 4,
            "replay ring stays bounded: {} events",
            replay.len()
        );
    }
}
