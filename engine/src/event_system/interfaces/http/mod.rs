//! HTTP API contracts for Event System endpoints.
//!
//! @canonical .pi/architecture/modules/event-system.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #46
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::event_system::application::dto::{
    DrainPersistedInput, DrainPersistedOutput, EventBusStatus, EventCountOutput,
    PublishEventInput, PublishEventOutput, QueryEventsOutput,
};

use crate::event_system::domain::{ExecutionEvent, PersistedEvent};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All event system endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/events";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/events/publish
// ---------------------------------------------------------------------------

/// POST /api/v1/events/publish
///
/// Publish an execution event to the event bus.
///
/// **Request:** `PublishEventRequest`
/// **Response:** `201 Created` with `PublishEventResponse`
pub const PUBLISH_EVENT_PATH: &str = "/api/v1/events/publish";
pub const PUBLISH_EVENT_METHOD: &str = "POST";

/// Request body for POST /api/v1/events/publish.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEventRequest {
    /// The execution event to publish.
    #[serde(flatten)]
    pub event: ExecutionEvent,
}

impl From<PublishEventRequest> for PublishEventInput {
    fn from(req: PublishEventRequest) -> Self {
        Self { event: req.event }
    }
}

/// Response body for POST /api/v1/events/publish.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEventResponse {
    pub success: bool,
    pub sequence: u64,
    pub subscriber_count: usize,
    pub had_laggers: bool,
}

impl From<PublishEventOutput> for PublishEventResponse {
    fn from(output: PublishEventOutput) -> Self {
        Self {
            success: true,
            sequence: output.sequence,
            subscriber_count: output.subscriber_count,
            had_laggers: output.had_laggers,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/events/subscribe
// ---------------------------------------------------------------------------

/// POST /api/v1/events/subscribe
///
/// Subscribe to receive future execution events.
/// The subscriber receives events via a long-lived connection (SSE or WebSocket).
///
/// **Request:** `SubscribeRequest`
/// **Response:** `200 OK` with `SubscribeResponse` (followed by SSE stream)
pub const SUBSCRIBE_PATH: &str = "/api/v1/events/subscribe";
pub const SUBSCRIBE_METHOD: &str = "POST";

/// Request body for POST /api/v1/events/subscribe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    /// Optional subscriber identifier for diagnostics.
    pub subscriber_name: Option<String>,

    /// Whether to receive past persisted events first (replay), then live events.
    pub replay_persisted: Option<bool>,
}

/// Response body for POST /api/v1/events/subscribe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeResponse {
    pub success: bool,
    pub subscriber_name: String,
    pub active_subscriber_count: usize,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/events/drain
// ---------------------------------------------------------------------------

/// POST /api/v1/events/drain
///
/// Drain all persisted events from the bus buffer.
/// Designed to be called once at execution end.
///
/// **Request:** `DrainRequest`
/// **Response:** `200 OK` with `DrainResponse`
pub const DRAIN_EVENTS_PATH: &str = "/api/v1/events/drain";
pub const DRAIN_EVENTS_METHOD: &str = "POST";

/// Request body for POST /api/v1/events/drain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainRequest {
    /// Whether to clear the buffer after draining (default: true).
    pub clear: Option<bool>,
}

impl From<DrainRequest> for DrainPersistedInput {
    fn from(req: DrainRequest) -> Self {
        Self {
            clear: req.clear.unwrap_or(true),
        }
    }
}

/// Response body for POST /api/v1/events/drain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainResponse {
    pub success: bool,
    pub events: Vec<PersistedEvent>,
    pub count: u64,
    pub cleared: bool,
}

impl From<DrainPersistedOutput> for DrainResponse {
    fn from(output: DrainPersistedOutput) -> Self {
        Self {
            success: true,
            events: output.events,
            count: output.count,
            cleared: output.cleared,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/events
// ---------------------------------------------------------------------------

/// GET /api/v1/events
///
/// Query persisted events with optional filters.
///
/// **Query Parameters:**
/// - `execution_id` (optional): Filter by execution ID
/// - `event_type` (optional): Filter by event type (e.g. "planning_started")
/// - `after_sequence` (optional): Events after this sequence number
/// - `limit` (optional): Maximum number of events to return (default: 100)
///
/// **Response:** `200 OK` with `QueryEventsResponse`
pub const QUERY_EVENTS_PATH: &str = "/api/v1/events";
pub const QUERY_EVENTS_METHOD: &str = "GET";

/// Response body for GET /api/v1/events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEventsResponse {
    pub success: bool,
    pub events: Vec<PersistedEvent>,
    pub total: u64,
    pub has_more: bool,
}

impl From<QueryEventsOutput> for QueryEventsResponse {
    fn from(output: QueryEventsOutput) -> Self {
        Self {
            success: true,
            events: output.events,
            total: output.total,
            has_more: output.has_more,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/events/status
// ---------------------------------------------------------------------------

/// GET /api/v1/events/status
///
/// Get the current event bus status (counts, subscribers, capacity).
///
/// **Response:** `200 OK` with `EventBusStatusResponse`
pub const EVENT_BUS_STATUS_PATH: &str = "/api/v1/events/status";
pub const EVENT_BUS_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/events/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusStatusResponse {
    pub success: bool,
    pub persisted_count: u64,
    pub current_sequence: u64,
    pub active_subscriber_count: usize,
    pub channel_capacity: usize,
    pub buffer_capacity: usize,
    pub total_events_published: u64,
    pub total_events_drained: u64,
}

impl From<(EventBusStatus, EventCountOutput)> for EventBusStatusResponse {
    fn from((status, counts): (EventBusStatus, EventCountOutput)) -> Self {
        Self {
            success: true,
            persisted_count: status.persisted_count,
            current_sequence: status.current_sequence,
            active_subscriber_count: status.active_subscriber_count,
            channel_capacity: status.channel_capacity,
            buffer_capacity: status.buffer_capacity,
            total_events_published: counts.total,
            total_events_drained: counts.drained,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/events/clear
// ---------------------------------------------------------------------------

/// POST /api/v1/events/clear
///
/// Clear all persisted events from the bus buffer.
///
/// **Response:** `200 OK` with `ClearEventsResponse`
pub const CLEAR_EVENTS_PATH: &str = "/api/v1/events/clear";
pub const CLEAR_EVENTS_METHOD: &str = "POST";

/// Response body for POST /api/v1/events/clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearEventsResponse {
    pub success: bool,
    pub cleared: u64,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Event System API endpoints.
///
/// All 4xx/5xx responses use this format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// HTTP status code.
    pub status: u16,
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Detailed error context (optional, may include field-level errors).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available).
    pub request_id: Option<String>,
}

/// Standardized error codes for Event System API.
pub mod error_codes {
    /// Subscriber lagged and missed events.
    pub const SUBSCRIBER_LAGGED: &str = "EVENT_SUBSCRIBER_LAGGED";
    /// Event serialization failed.
    pub const SERIALIZATION_FAILED: &str = "EVENT_SERIALIZATION_FAILED";
    /// Event deserialization failed.
    pub const DESERIALIZATION_FAILED: &str = "EVENT_DESERIALIZATION_FAILED";
    /// Event bus was already drained.
    pub const ALREADY_DRAINED: &str = "EVENT_ALREADY_DRAINED";
    /// Channel capacity below minimum.
    pub const CAPACITY_TOO_LOW: &str = "EVENT_CAPACITY_TOO_LOW";
    /// No active subscribers.
    pub const NO_SUBSCRIBERS: &str = "EVENT_NO_SUBSCRIBERS";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "EVENT_INTERNAL_ERROR";
}

/// HTTP status code mappings for Event System errors.
pub mod status_codes {
    pub const SUBSCRIBER_LAGGED: u16 = 409;
    pub const SERIALIZATION_FAILED: u16 = 500;
    pub const DESERIALIZATION_FAILED: u16 = 500;
    pub const ALREADY_DRAINED: u16 = 409;
    pub const CAPACITY_TOO_LOW: u16 = 400;
    pub const NO_SUBSCRIBERS: u16 = 200;
    pub const INTERNAL_ERROR: u16 = 500;
}
