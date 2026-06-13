//! Data Transfer Objects for the Audit module.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — DTO schemas for build, send, queue operations
//! Issue: #13
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::audit::domain::{AuditEnvelope, ExecutionEventRef};

// ---------------------------------------------------------------------------
// Build Envelope DTOs
// ---------------------------------------------------------------------------

/// Input for building an audit envelope from execution data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildEnvelopeInput {
    /// Globally unique execution identifier.
    pub execution_id: uuid::Uuid,

    /// Template identifier that generated this execution.
    pub template_id: String,

    /// The planning prompt text used to generate the execution plan.
    /// Used to compute the planning hash for replay verification.
    pub planning_prompt: String,

    /// Ordered list of execution event references to include in the envelope.
    pub events: Vec<ExecutionEventRef>,

    /// Optional execution metadata key-value pairs.
    pub metadata: Option<HashMap<String, String>>,

    /// Whether to include HMAC signature (requires signing key config).
    pub sign: bool,
}

/// Output from building an audit envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildEnvelopeOutput {
    /// The constructed audit envelope.
    pub envelope: AuditEnvelope,

    /// Whether the envelope was signed.
    pub signed: bool,

    /// Number of events included in the envelope.
    pub event_count: usize,
}

// ---------------------------------------------------------------------------
// Send Envelope DTOs
// ---------------------------------------------------------------------------

/// Input for sending an audit envelope to the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEnvelopeInput {
    /// The audit envelope to send.
    pub envelope: AuditEnvelope,

    /// Target backend URL. Overrides the configured default if set.
    pub backend_url: Option<String>,

    /// Request timeout in seconds.
    pub timeout_secs: Option<u64>,
}

/// Output from sending an audit envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEnvelopeOutput {
    /// Whether delivery was successful.
    pub success: bool,

    /// HTTP status code from the backend (if received).
    pub http_status: Option<u16>,

    /// Duration of the send attempt in milliseconds.
    pub duration_ms: u64,

    /// Backend URL that was contacted.
    pub backend_url: String,
}

// ---------------------------------------------------------------------------
// Deliver with Retry DTOs
// ---------------------------------------------------------------------------

/// Input for delivering an envelope with retry logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverEnvelopeInput {
    /// The audit envelope to deliver.
    pub envelope: AuditEnvelope,

    /// Maximum number of retry attempts.
    pub max_retries: u32,

    /// Base delay in seconds for exponential backoff.
    /// Actual delay = base * 2^attempt with jitter.
    pub backoff_base_secs: u64,

    /// Maximum delay in seconds between retries.
    pub backoff_max_secs: u64,
}

/// Output from delivering with retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverEnvelopeOutput {
    /// Whether delivery was ultimately successful.
    pub success: bool,

    /// Number of attempts made.
    pub attempts: u32,

    /// Total time spent in milliseconds.
    pub total_duration_ms: u64,

    /// Final HTTP status (if any).
    pub last_http_status: Option<u16>,

    /// Error detail from the last failed attempt.
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Queue DTOs
// ---------------------------------------------------------------------------

/// Input for enqueuing a failed envelope for retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueInput {
    /// The envelope that failed delivery.
    pub envelope: AuditEnvelope,

    /// Reason for the failure.
    pub failure_reason: String,

    /// How many retries have already been attempted.
    pub retry_count: u32,

    /// Maximum retries before dropping.
    pub max_retries: u32,
}

/// Output from an enqueue/dequeue operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueOutput {
    /// The envelope (for dequeue) or confirmation (for enqueue).
    pub envelope: Option<AuditEnvelope>,

    /// Whether the operation succeeded.
    pub success: bool,

    /// Reason if operation failed.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Envelope status update DTO
// ---------------------------------------------------------------------------

/// Input for recording the delivery status of an envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordDeliveryInput {
    /// The execution ID for correlation.
    pub execution_id: uuid::Uuid,

    /// Whether delivery was successful.
    pub success: bool,

    /// HTTP status code (if any).
    pub http_status: Option<u16>,

    /// Error detail (if failed).
    pub error_detail: Option<String>,

    /// Duration in milliseconds.
    pub duration_ms: u64,
}
