//! HTTP API contracts for Audit endpoints.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #13
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

use crate::audit::application::dto::{
    BuildEnvelopeInput, BuildEnvelopeOutput, DeliverEnvelopeInput, DeliverEnvelopeOutput,
    SendEnvelopeInput, SendEnvelopeOutput,
};

use crate::audit::domain::{AuditEnvelope, CircuitBreakerState};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All audit endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/audit";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit/envelope
// ---------------------------------------------------------------------------

/// POST /api/v1/audit/envelope
///
/// Build an audit envelope from execution data and optionally send it.
///
/// **Request:** `BuildEnvelopeRequest`
/// **Response:** `201 Created` with `BuildEnvelopeResponse`
pub const BUILD_ENVELOPE_PATH: &str = "/api/v1/audit/envelope";
pub const BUILD_ENVELOPE_METHOD: &str = "POST";

/// Request body for POST /api/v1/audit/envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildEnvelopeRequest {
    /// Globally unique execution identifier.
    pub execution_id: uuid::Uuid,

    /// Template identifier that generated this execution.
    pub template_id: String,

    /// The planning prompt text for replay verification.
    pub planning_prompt: String,

    /// Ordered list of execution event references.
    pub events: Vec<ExecutionEventRefDto>,

    /// Optional execution metadata key-value pairs.
    pub metadata: Option<std::collections::HashMap<String, String>>,

    /// Whether to include HMAC signature.
    pub sign: Option<bool>,

    /// Whether to immediately send the envelope after building.
    pub send_immediately: Option<bool>,
}

impl From<BuildEnvelopeRequest> for BuildEnvelopeInput {
    fn from(req: BuildEnvelopeRequest) -> Self {
        Self {
            execution_id: req.execution_id,
            template_id: req.template_id,
            planning_prompt: req.planning_prompt,
            events: req.events.into_iter().map(Into::into).collect(),
            metadata: req.metadata,
            sign: req.sign.unwrap_or(false),
        }
    }
}

/// Response body for POST /api/v1/audit/envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildEnvelopeResponse {
    pub success: bool,
    pub envelope: AuditEnvelope,
    pub signed: bool,
    pub event_count: usize,
    pub delivery_status: Option<DeliverStatusDto>,
}

impl From<BuildEnvelopeOutput> for BuildEnvelopeResponse {
    fn from(output: BuildEnvelopeOutput) -> Self {
        Self {
            success: true,
            envelope: output.envelope,
            signed: output.signed,
            event_count: output.event_count,
            delivery_status: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit/envelope/send
// ---------------------------------------------------------------------------

/// POST /api/v1/audit/envelope/send
///
/// Send an existing audit envelope to the backend.
///
/// **Request:** `SendEnvelopeRequest`
/// **Response:** `200 OK` with `SendEnvelopeResponse`
pub const SEND_ENVELOPE_PATH: &str = "/api/v1/audit/envelope/send";
pub const SEND_ENVELOPE_METHOD: &str = "POST";

/// Request body for POST /api/v1/audit/envelope/send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEnvelopeRequest {
    pub envelope: AuditEnvelope,
    pub backend_url: Option<String>,
    pub timeout_secs: Option<u64>,
}

impl From<SendEnvelopeRequest> for SendEnvelopeInput {
    fn from(req: SendEnvelopeRequest) -> Self {
        Self {
            envelope: req.envelope,
            backend_url: req.backend_url,
            timeout_secs: req.timeout_secs,
        }
    }
}

/// Response body for POST /api/v1/audit/envelope/send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEnvelopeResponse {
    pub success: bool,
    pub http_status: Option<u16>,
    pub duration_ms: u64,
    pub backend_url: String,
}

impl From<SendEnvelopeOutput> for SendEnvelopeResponse {
    fn from(output: SendEnvelopeOutput) -> Self {
        Self {
            success: output.success,
            http_status: output.http_status,
            duration_ms: output.duration_ms,
            backend_url: output.backend_url,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit/envelope/deliver
// ---------------------------------------------------------------------------

/// POST /api/v1/audit/envelope/deliver
///
/// Deliver an envelope with retry logic and exponential backoff.
///
/// **Request:** `DeliverEnvelopeRequest`
/// **Response:** `200 OK` with `DeliverEnvelopeResponse`
pub const DELIVER_ENVELOPE_PATH: &str = "/api/v1/audit/envelope/deliver";
pub const DELIVER_ENVELOPE_METHOD: &str = "POST";

/// Request body for POST /api/v1/audit/envelope/deliver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverEnvelopeRequest {
    pub envelope: AuditEnvelope,
    pub max_retries: u32,
    pub backoff_base_secs: u64,
    pub backoff_max_secs: u64,
}

impl From<DeliverEnvelopeRequest> for DeliverEnvelopeInput {
    fn from(req: DeliverEnvelopeRequest) -> Self {
        Self {
            envelope: req.envelope,
            max_retries: req.max_retries,
            backoff_base_secs: req.backoff_base_secs,
            backoff_max_secs: req.backoff_max_secs,
        }
    }
}

/// Response body for POST /api/v1/audit/envelope/deliver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverEnvelopeResponse {
    pub success: bool,
    pub attempts: u32,
    pub total_duration_ms: u64,
    pub last_http_status: Option<u16>,
    pub last_error: Option<String>,
}

impl From<DeliverEnvelopeOutput> for DeliverEnvelopeResponse {
    fn from(output: DeliverEnvelopeOutput) -> Self {
        Self {
            success: output.success,
            attempts: output.attempts,
            total_duration_ms: output.total_duration_ms,
            last_http_status: output.last_http_status,
            last_error: output.last_error,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/audit/status
// ---------------------------------------------------------------------------

/// GET /api/v1/audit/status
///
/// Get the current audit system status (queue depth, circuit breaker state).
///
/// **Response:** `200 OK` with `AuditStatusResponse`
pub const AUDIT_STATUS_PATH: &str = "/api/v1/audit/status";
pub const AUDIT_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/audit/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatusResponse {
    pub pending_count: u32,
    pub circuit_breaker_state: CircuitBreakerState,
    pub backend_available: bool,
    pub total_envelopes_sent: u64,
    pub total_envelopes_failed: u64,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit/retry
// ---------------------------------------------------------------------------

/// POST /api/v1/audit/retry
///
/// Retry all pending envelopes in the delivery queue.
///
/// **Response:** `200 OK` with `RetryPendingResponse`
pub const RETRY_PENDING_PATH: &str = "/api/v1/audit/retry";
pub const RETRY_PENDING_METHOD: &str = "POST";

/// Response body for POST /api/v1/audit/retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPendingResponse {
    pub delivered: u32,
    pub still_pending: u32,
    pub dropped: u32,
}

// ---------------------------------------------------------------------------
// Shared DTOs
// ---------------------------------------------------------------------------

/// DTO for execution event references in request/response payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEventRefDto {
    /// Machine-readable event type.
    pub event_type: String,
    /// Human-readable event summary.
    pub summary: String,
    /// ISO 8601 timestamp.
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    /// Optional correlation ID.
    pub correlation_id: Option<uuid::Uuid>,
    /// Event status (success, failure, skipped, cancelled).
    pub status: String,
}

impl From<ExecutionEventRefDto> for crate::audit::domain::ExecutionEventRef {
    fn from(dto: ExecutionEventRefDto) -> Self {
        Self {
            event_type: dto.event_type,
            summary: dto.summary,
            occurred_at: dto.occurred_at,
            correlation_id: dto.correlation_id,
            status: match dto.status.to_lowercase().as_str() {
                "success" => crate::audit::domain::EventStatus::Success,
                "failure" => crate::audit::domain::EventStatus::Failure,
                "skipped" => crate::audit::domain::EventStatus::Skipped,
                "cancelled" => crate::audit::domain::EventStatus::Cancelled,
                _ => crate::audit::domain::EventStatus::Skipped,
            },
        }
    }
}

impl From<crate::audit::domain::ExecutionEventRef> for ExecutionEventRefDto {
    fn from(event: crate::audit::domain::ExecutionEventRef) -> Self {
        Self {
            event_type: event.event_type,
            summary: event.summary,
            occurred_at: event.occurred_at,
            correlation_id: event.correlation_id,
            status: match event.status {
                crate::audit::domain::EventStatus::Success => "success".to_string(),
                crate::audit::domain::EventStatus::Failure => "failure".to_string(),
                crate::audit::domain::EventStatus::Skipped => "skipped".to_string(),
                crate::audit::domain::EventStatus::Cancelled => "cancelled".to_string(),
            },
        }
    }
}

/// DTO for delivery status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverStatusDto {
    pub delivered: bool,
    pub attempts: u32,
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Audit API endpoints.
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

/// Standardized error codes for Audit API.
pub mod error_codes {
    /// Envelope send failed (backend error).
    pub const SEND_FAILED: &str = "AUDIT_SEND_FAILED";
    /// Circuit breaker is open.
    pub const CIRCUIT_BREAKER_OPEN: &str = "AUDIT_CIRCUIT_BREAKER_OPEN";
    /// Envelope serialization failed.
    pub const SERIALIZATION_FAILED: &str = "AUDIT_SERIALIZATION_FAILED";
    /// Signature verification failed.
    pub const SIGNATURE_MISMATCH: &str = "AUDIT_SIGNATURE_MISMATCH";
    /// Delivery queue is full.
    pub const QUEUE_FULL: &str = "AUDIT_QUEUE_FULL";
    /// Audit backend not configured.
    pub const NOT_CONFIGURED: &str = "AUDIT_NOT_CONFIGURED";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "AUDIT_INTERNAL_ERROR";
}

/// HTTP status code mappings for Audit errors.
pub mod status_codes {
    pub const SEND_FAILED: u16 = 502;
    pub const CIRCUIT_BREAKER_OPEN: u16 = 503;
    pub const SERIALIZATION_FAILED: u16 = 500;
    pub const SIGNATURE_MISMATCH: u16 = 400;
    pub const QUEUE_FULL: u16 = 429;
    pub const NOT_CONFIGURED: u16 = 503;
    pub const INTERNAL_ERROR: u16 = 500;
}
