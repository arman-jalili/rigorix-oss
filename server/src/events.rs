//! Server push channel — `GET /events` (SSE) (#888 OSS-C2, ADR-0001 D8).
//!
//! Emits the three ADR-0001 D8 events — `approval_required`, `run_progress`,
//! `policy_changed` — whose payloads reuse `envelope.json` event refs
//! (`event_type`, `status`, `payload.step_name`, …). Clients that cannot hold
//! a stream fall back to polling `rigorix.audit.read`.
//!
//! Transport guarantees:
//! - **Replay**: a subscriber may send `Last-Event-ID` (header) or
//!   `?last_event_id=` (query) — later events are replayed from a bounded ring
//!   buffer before the live stream.
//! - **Heartbeat**: the SSE stream carries a keep-alive comment.
//! - **Backpressure**: the live broadcast channel is bounded; a slow consumer
//!   that lags past the buffer drops events (it never stalls execution —
//!   mirroring `AuditEnvelopeDropped`) and can reconnect with `Last-Event-ID`
//!   to replay from the ring buffer.
//!
//! > **Note:** SDK-C1 (`schemas/api/events.json`) is not yet published. This
//! > module is faithful to ADR-0001 D8 (event names + envelope event-ref
//! > payloads); when C1 lands, only the payload schema validation is added.
//!
//! The legacy MCP SSE endpoint is **not** resurrected (GAP-A-10).

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::ServerState;

/// The three ADR-0001 D8 SSE event names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A run paused for human approval.
    ApprovalRequired,
    /// Progress on a running execution.
    RunProgress,
    /// Policy changed for the org/repo.
    PolicyChanged,
}

impl EventKind {
    /// The SSE `event:` name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApprovalRequired => "approval_required",
            Self::RunProgress => "run_progress",
            Self::PolicyChanged => "policy_changed",
        }
    }
}

/// One SSE event, carrying a monotonic id (for `Last-Event-ID` replay), the
/// event kind, and an `envelope.json` event-ref-shaped payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerEvent {
    /// Monotonic id (SSE `id:`), used for Last-Event-ID replay.
    pub id: u64,
    /// The event kind (SSE `event:`).
    pub event: EventKind,
    /// Envelope event-ref-shaped payload (`event_type`, `status`, `payload`, …).
    pub data: Value,
}

/// Default number of events retained for `Last-Event-ID` replay.
pub const DEFAULT_REPLAY_CAPACITY: usize = 256;
/// Default live broadcast buffer per subscriber (backpressure bound).
pub const DEFAULT_BROADCAST_CAPACITY: usize = 64;

/// In-process push hub: bounded replay ring + bounded live broadcast.
pub struct EventHub {
    next_id: AtomicU64,
    replay_capacity: usize,
    replay: Mutex<VecDeque<ServerEvent>>,
    tx: broadcast::Sender<ServerEvent>,
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHub {
    /// Create a hub with the default capacities.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_REPLAY_CAPACITY, DEFAULT_BROADCAST_CAPACITY)
    }

    /// Create a hub with explicit replay + broadcast capacities.
    pub fn with_capacity(replay_capacity: usize, broadcast_capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(broadcast_capacity.max(1));
        Self {
            next_id: AtomicU64::new(0),
            replay_capacity: replay_capacity.max(1),
            replay: Mutex::new(VecDeque::with_capacity(replay_capacity.max(1))),
            tx,
        }
    }

    /// Publish an event; returns its assigned id.
    pub fn publish(&self, event: EventKind, data: Value) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let server_event = ServerEvent { id, event, data };

        {
            let mut replay = self.replay.lock().expect("event replay lock");
            replay.push_back(server_event.clone());
            while replay.len() > self.replay_capacity {
                replay.pop_front();
            }
        }

        // No subscriber is not an error — events are retained for replay.
        let _ = self.tx.send(server_event);
        id
    }

    /// Subscribe with optional `Last-Event-ID`.
    ///
    /// Subscribes to the live channel **before** reading the replay buffer so
    /// no event can slip between replay and live. Returns the replay events
    /// (id strictly greater than `last_event_id`) and the live receiver.
    pub fn subscribe(
        &self,
        last_event_id: Option<u64>,
    ) -> (Vec<ServerEvent>, broadcast::Receiver<ServerEvent>) {
        let receiver = self.tx.subscribe();
        let replay: Vec<ServerEvent> = {
            let replay = self.replay.lock().expect("event replay lock");
            replay
                .iter()
                .filter(|event| last_event_id.is_none_or(|last| event.id > last))
                .cloned()
                .collect()
        };
        (replay, receiver)
    }

    /// Number of live subscribers (test/introspection helper).
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

/// `GET /events` query parameters.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventsQuery {
    /// Replay events after this id (alternative to the `Last-Event-ID` header).
    pub last_event_id: Option<u64>,
}

/// `GET /events` — SSE stream of the three D8 events.
pub async fn events_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(query): Query<EventsQuery>,
) -> Response {
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .or(query.last_event_id);

    let (replay, receiver) = state.events.subscribe(last_event_id);

    // Replay first, then live. A lagged live receiver drops the missed events
    // (bounded buffer) and the client can reconnect with Last-Event-ID.
    let replay_stream = tokio_stream::iter(replay.into_iter().map(to_sse_event));
    let live_stream = BroadcastStream::new(receiver)
        .filter_map(|result| async move { result.ok() })
        .map(to_sse_event);
    let stream = replay_stream
        .chain(live_stream)
        .map(Ok::<Event, Infallible>);

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}

fn to_sse_event(event: ServerEvent) -> Event {
    let data = serde_json::to_string(&event.data).unwrap_or_else(|_| "{}".to_string());
    Event::default()
        .id(event.id.to_string())
        .event(event.event.as_str())
        .data(data)
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_kind_names_match_d8() {
        assert_eq!(EventKind::ApprovalRequired.as_str(), "approval_required");
        assert_eq!(EventKind::RunProgress.as_str(), "run_progress");
        assert_eq!(EventKind::PolicyChanged.as_str(), "policy_changed");
    }

    #[test]
    fn publish_assigns_monotonic_ids_and_replays_after_last_event_id() {
        let hub = EventHub::new();
        let a = hub.publish(
            EventKind::RunProgress,
            json!({"event_type": "node_completed"}),
        );
        let b = hub.publish(
            EventKind::PolicyChanged,
            json!({"event_type": "policy_changed"}),
        );
        let _c = hub.publish(
            EventKind::ApprovalRequired,
            json!({"event_type": "approval_requested"}),
        );
        assert_eq!((a, b), (1, 2));

        let (replay, _rx) = hub.subscribe(Some(b));
        assert_eq!(replay.len(), 1, "only events after Last-Event-ID replay");
        assert_eq!(replay[0].id, 3);

        let (all, _rx) = hub.subscribe(None);
        assert_eq!(all.len(), 3, "no Last-Event-ID replays everything retained");
        assert_eq!(all[0].id, 1);
    }

    #[test]
    fn replay_buffer_is_bounded() {
        let hub = EventHub::with_capacity(2, 4);
        for _ in 0..5 {
            hub.publish(EventKind::RunProgress, json!({}));
        }
        let (replay, _rx) = hub.subscribe(None);
        assert_eq!(replay.len(), 2, "oldest events are evicted");
        assert_eq!(replay[0].id, 4);
        assert_eq!(replay[1].id, 5);
    }

    #[test]
    fn slow_consumer_lags_instead_of_stalling_execution() {
        let hub = EventHub::with_capacity(8, 2);
        let (_, mut rx) = hub.subscribe(None);
        for _ in 0..4 {
            hub.publish(EventKind::RunProgress, json!({}));
        }
        let err = rx.try_recv().expect_err("receiver must lag");
        assert!(matches!(err, broadcast::error::TryRecvError::Lagged(_)));
    }

    #[tokio::test]
    async fn events_handler_streams_text_event_stream() {
        use axum::extract::{Query, State};
        use axum::http::HeaderMap;

        // Minimal backend stub — the events handler only uses the hub.
        struct Stub;
        #[async_trait::async_trait]
        impl crate::backend::MethodBackend for Stub {
            async fn dispatch(
                &self,
                _method: &str,
                _params: Value,
            ) -> Result<Value, rigorix_mcp::host::error::HostError> {
                Ok(json!({}))
            }
        }

        let state = crate::state::ServerState::new(
            std::sync::Arc::new(Stub),
            std::sync::Arc::new(EventHub::new()),
        );
        state.events.publish(
            EventKind::RunProgress,
            json!({"event_type": "node_completed"}),
        );

        let response = events_handler(
            State(state),
            HeaderMap::new(),
            Query(EventsQuery::default()),
        )
        .await;
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("text/event-stream")
        );
    }
}
