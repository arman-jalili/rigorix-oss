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

    /// Source/environment that triggered this execution
    /// (e.g. "rigorix_action", "rigorix_cli").
    pub source: Option<String>,

    /// Repository the execution ran against (e.g. "my-org/my-repo").
    pub repository: Option<String>,

    /// Identity of the user or bot that triggered the execution.
    pub author: Option<String>,

    /// Total number of LLM tokens consumed during this execution.
    ///
    /// Used for cost estimation and AI ROI analytics in the Enterprise dashboard.
    pub total_tokens: u32,

    /// Total wall-clock duration of the execution in milliseconds.
    ///
    /// Used for performance analytics and time-based cost estimation.
    pub duration_ms: u64,

    /// Git commit hash of the repository at the time of execution.
    ///
    /// Used for compliance provenance chains.
    pub git_commit: Option<String>,

    /// Git branch name at the time of execution.
    ///
    /// Used for compliance provenance chains.
    pub git_branch: Option<String>,

    /// LLM model version used for planning (e.g. "claude-sonnet-4-20250514").
    /// `None` when no LLM was used or the model version was not captured.
    pub model_version: Option<String>,

    /// The planning prompt text (opt-in, privacy-sensitive).
    ///
    /// Only populated when prompt content capture is enabled in configuration.
    /// Used for compliance provenance and audit evidence.
    pub planning_prompt: Option<String>,

    /// File paths changed or created during this execution (if available).
    ///
    /// Used for compliance provenance chains to track which files were
    /// modified by AI-generated output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_paths: Vec<String>,

    /// The ordered list of execution events captured during this run.
    pub events: Vec<ExecutionEventRef>,

    /// Scoring results from scored_evaluation nodes, keyed by node_id.
    ///
    /// Populated when scored evaluation nodes are present in the DAG.
    /// Used for compliance provenance and quality audit evidence.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub scoring_results: std::collections::HashMap<String, ScoringResultRef>,

    /// HMAC signature for envelope integrity verification.
    ///
    /// `None` if HMAC signing is not configured.
    pub signature: Option<String>,
}

/// A reference to a scoring result included in the audit envelope.
///
/// Contains the pass/fail status, backend metadata, and dimension scores
/// but not the full raw response (to keep envelope size manageable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResultRef {
    /// Whether all scoring dimensions passed.
    pub passed: bool,

    /// Name of the scoring backend that produced this result.
    pub backend: String,

    /// Map of dimension name to score dimension reference.
    pub dimensions: std::collections::HashMap<String, ScoreDimensionRef>,

    /// Duration of the evaluation in milliseconds.
    pub duration_ms: u64,
}

/// A reference to a single scoring dimension within a scoring result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDimensionRef {
    /// The achieved score (0.0–1.0).
    pub score: f64,

    /// The maximum possible score.
    pub max: f64,

    /// Human-readable label for this dimension.
    pub label: String,

    /// Whether this dimension passed.
    pub passed: bool,
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

    /// Optional JSON payload with event-specific details.
    ///
    /// May contain per-node output, tool usage data, budget warning details,
    /// or error information depending on the event type.
    /// Omitted when empty to keep envelope size manageable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
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
