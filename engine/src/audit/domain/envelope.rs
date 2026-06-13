//! AuditEnvelope domain entity.
//!
//! @canonical .pi/architecture/modules/audit.md#envelope
//! Implements: Contract Freeze — AuditEnvelope value object with execution metadata
//! Issue: #13
//!
//! Typed envelope containing execution audit data. Carries execution metadata,
//! planning hash for replay verification, and an optional HMAC signature for
//! integrity protection.
//!
//! # Contract (Frozen)
//! - `AuditEnvelope` is the value object for all audit records
//! - All fields are public for direct construction by the application layer
//! - Construction happens via `AuditEnvelopeFactory`
//! - Signature is optional — populated when HMAC signing is configured

use serde::{Deserialize, Serialize};

/// Typed envelope containing execution audit data.
///
/// Built at execution completion by the orchestration layer and sent to
/// the configured audit backend via `AuditSender`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEnvelope {
    /// Globally unique execution identifier (UUID v4).
    pub execution_id: uuid::Uuid,

    /// Timestamp when the execution completed (ISO 8601 / UTC).
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Identifier of the template that generated this execution plan.
    pub template_id: String,

    /// Hash of the planning prompt used for replay reproducibility.
    ///
    /// Allows verifying that the same input produces the same plan.
    pub planning_hash: String,

    /// The ordered list of execution events captured during this run.
    pub events: Vec<ExecutionEventRef>,

    /// HMAC signature for envelope integrity verification.
    ///
    /// `None` if HMAC signing is not configured.
    pub signature: Option<String>,
}

/// A reference to an execution event included in the audit envelope.
///
/// Contains the event type, timestamp, and a correlation identifier
/// but not the full event payload (to keep envelope size manageable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEventRef {
    /// Machine-readable event type (e.g. "task_completed", "tool_executed").
    pub event_type: String,

    /// Human-readable event summary for audit review.
    pub summary: String,

    /// ISO 8601 timestamp of when the event occurred.
    pub occurred_at: chrono::DateTime<chrono::Utc>,

    /// Correlation ID linking this event across services.
    pub correlation_id: Option<uuid::Uuid>,

    /// Whether this event represents a success or failure.
    pub status: EventStatus,
}

/// Status of an execution event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventStatus {
    /// Event completed successfully.
    Success,
    /// Event failed with an error.
    Failure,
    /// Event was skipped (e.g. due to conditionals).
    Skipped,
    /// Event was cancelled.
    Cancelled,
}

/// Circuit breaker state machine for resilient HTTP delivery.
///
/// Follows the standard closed → open → half-open → closed pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    /// Normal operation — requests pass through.
    Closed,
    /// Failure threshold exceeded — requests are rejected immediately.
    Open,
    /// Probing — a single test request is allowed through.
    HalfOpen,
}
