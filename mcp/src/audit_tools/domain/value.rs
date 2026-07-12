//! Value objects for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#value-objects
//! Implements: Contract Freeze — AuditFilter, AuditSummary, AuditEnvelope,
//! TopFailure, TopTemplate, ExecutionEvent, EventStatus
//!
//! Value objects are immutable, interchangeable, and defined by their attributes,
//! not identity. They carry validation in their constructors and are serializable
//! for API transmission.
//!
//! # Contract (Frozen)
//!
//! - All value objects are immutable (no pub fields, no setters)
//! - All value objects implement PartialEq based on ALL fields
//! - Constructors validate invariants — return Result<_, Error> on failure
//! - All types derive Serialize + Deserialize for JSON transmission
//! - No behavior beyond field accessors and validation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::execution_tools::domain::value::ExecutionStatus;

// ---------------------------------------------------------------------------
// AuditEnvelope — external audit record (MCP-facing)
// ---------------------------------------------------------------------------

/// External-facing audit envelope returned to MCP clients.
///
/// Contains execution metadata, step results, token usage, event history,
/// and an HMAC integrity signature. Differs from the engine-internal
/// AuditEnvelope — this is the MCP-consumable view.
///
/// # Contract (Frozen)
///
/// - `execution_id` is always present (UUID v4)
/// - `status` reflects the final execution state
/// - `steps` mirrors plan step order with per-step outcomes
/// - `hmac` is validated by rigorix-engine, not the gateway
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEnvelope {
    /// Globally unique execution identifier.
    execution_id: Uuid,

    /// Overall execution status (Completed, Failed, etc.).
    status: ExecutionStatus,

    /// Optional template name that generated the execution plan.
    #[serde(default)]
    template_name: Option<String>,

    /// Timestamp when execution started.
    started_at: DateTime<Utc>,

    /// Timestamp when execution completed.
    completed_at: DateTime<Utc>,

    /// Total execution duration in milliseconds.
    duration_ms: u64,

    /// Per-step results in plan execution order.
    steps: Vec<ExecutionStep>,

    /// Optional total token usage across all steps.
    #[serde(default)]
    tokens_used: Option<u64>,

    /// HMAC signature for envelope integrity verification.
    hmac: String,

    /// Ordered list of execution events captured during this run.
    events: Vec<ExecutionEvent>,
}

impl AuditEnvelope {
    /// Create a new AuditEnvelope.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        execution_id: Uuid,
        status: ExecutionStatus,
        template_name: Option<String>,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        duration_ms: u64,
        steps: Vec<ExecutionStep>,
        tokens_used: Option<u64>,
        hmac: String,
        events: Vec<ExecutionEvent>,
    ) -> Self {
        Self {
            execution_id,
            status,
            template_name,
            started_at,
            completed_at,
            duration_ms,
            steps,
            tokens_used,
            hmac,
            events,
        }
    }

    /// Execution ID.
    pub fn execution_id(&self) -> Uuid {
        self.execution_id
    }

    /// Execution status.
    pub fn status(&self) -> &ExecutionStatus {
        &self.status
    }

    /// Optional template name.
    pub fn template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }

    /// Execution start timestamp.
    pub fn started_at(&self) -> &DateTime<Utc> {
        &self.started_at
    }

    /// Execution completion timestamp.
    pub fn completed_at(&self) -> &DateTime<Utc> {
        &self.completed_at
    }

    /// Duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Per-step results.
    pub fn steps(&self) -> &[ExecutionStep] {
        &self.steps
    }

    /// Optional token usage.
    pub fn tokens_used(&self) -> Option<u64> {
        self.tokens_used
    }

    /// HMAC signature.
    pub fn hmac(&self) -> &str {
        &self.hmac
    }

    /// Execution events.
    pub fn events(&self) -> &[ExecutionEvent] {
        &self.events
    }
}

// ---------------------------------------------------------------------------
// ExecutionStep — outcome of a single execution step (MCP-facing)
// ---------------------------------------------------------------------------

/// Result of a single execution step, as returned to MCP clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Step name (matches plan step name).
    step_name: String,

    /// Whether the step succeeded.
    success: bool,

    /// Optional error message if step failed.
    #[serde(default)]
    error: Option<String>,

    /// Step output data (tool-specific).
    #[serde(default)]
    output: serde_json::Value,

    /// Duration of this step in milliseconds.
    duration_ms: u64,
}

impl ExecutionStep {
    /// Create a new ExecutionStep.
    pub fn new(
        step_name: String,
        success: bool,
        error: Option<String>,
        output: serde_json::Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            step_name,
            success,
            error,
            output,
            duration_ms,
        }
    }

    /// Step name.
    pub fn step_name(&self) -> &str {
        &self.step_name
    }

    /// Whether the step succeeded.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Optional error message.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Step output.
    pub fn output(&self) -> &serde_json::Value {
        &self.output
    }

    /// Duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

// ---------------------------------------------------------------------------
// ExecutionEvent — an event captured during execution
// ---------------------------------------------------------------------------

/// An event that occurred during execution (typed, timestamped).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// Machine-readable event type (e.g. "task_completed", "tool_executed").
    event_type: String,

    /// Human-readable event summary for audit review.
    summary: String,

    /// ISO 8601 timestamp of when the event occurred.
    occurred_at: DateTime<Utc>,

    /// Correlation ID linking this event across services.
    #[serde(default)]
    correlation_id: Option<Uuid>,

    /// Whether this event represents a success or failure.
    status: EventStatus,
}

impl ExecutionEvent {
    /// Create a new ExecutionEvent.
    pub fn new(
        event_type: String,
        summary: String,
        occurred_at: DateTime<Utc>,
        correlation_id: Option<Uuid>,
        status: EventStatus,
    ) -> Self {
        Self {
            event_type,
            summary,
            occurred_at,
            correlation_id,
            status,
        }
    }

    /// Event type.
    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    /// Event summary.
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// When the event occurred.
    pub fn occurred_at(&self) -> &DateTime<Utc> {
        &self.occurred_at
    }

    /// Optional correlation ID.
    pub fn correlation_id(&self) -> Option<Uuid> {
        self.correlation_id
    }

    /// Event status.
    pub fn status(&self) -> &EventStatus {
        &self.status
    }
}

// ---------------------------------------------------------------------------
// EventStatus — status of an execution event
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// AuditFilter — criteria for listing audit records
// ---------------------------------------------------------------------------

/// Criteria for filtering audit records when listing.
///
/// All fields are optional — unset fields are not filtered on.
///
/// # Contract (Frozen)
///
/// - `limit` defaults to 50 if not specified
/// - Results are ordered by completion time (newest first)
/// - Empty filter returns all records (up to limit)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditFilter {
    /// Filter by execution status (Completed, Failed, etc.).
    #[serde(default)]
    status: Option<ExecutionStatus>,

    /// Include records completed on or after this timestamp.
    #[serde(default)]
    since: Option<DateTime<Utc>>,

    /// Include records completed on or before this timestamp.
    #[serde(default)]
    until: Option<DateTime<Utc>>,

    /// Filter by template name (exact match).
    #[serde(default)]
    template_name: Option<String>,

    /// Maximum number of records to return (default: 50).
    #[serde(default = "default_limit")]
    limit: usize,

    /// Number of records to skip (for pagination).
    #[serde(default)]
    offset: Option<usize>,
}

fn default_limit() -> usize {
    50
}

impl AuditFilter {
    /// Create a new AuditFilter with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an AuditFilter with all fields specified.
    #[allow(clippy::too_many_arguments)]
    pub fn with_all(
        status: Option<ExecutionStatus>,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
        template_name: Option<String>,
        limit: usize,
        offset: Option<usize>,
    ) -> Self {
        Self {
            status,
            since,
            until,
            template_name,
            limit,
            offset,
        }
    }

    /// Filter by execution status.
    pub fn status(&self) -> Option<&ExecutionStatus> {
        self.status.as_ref()
    }

    /// Earliest completion time filter.
    pub fn since(&self) -> Option<&DateTime<Utc>> {
        self.since.as_ref()
    }

    /// Latest completion time filter.
    pub fn until(&self) -> Option<&DateTime<Utc>> {
        self.until.as_ref()
    }

    /// Template name filter.
    pub fn template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }

    /// Maximum number of records to return.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Number of records to skip.
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }
}

impl Default for AuditFilter {
    fn default() -> Self {
        Self {
            status: None,
            since: None,
            until: None,
            template_name: None,
            limit: 50,
            offset: None,
        }
    }
}

// ---------------------------------------------------------------------------
// AuditSummary — aggregate audit statistics
// ---------------------------------------------------------------------------

/// Aggregate audit statistics over a time range.
///
/// Computed from all audit records within `since` to `until`.
///
/// # Contract (Frozen)
///
/// - `success_rate` is a float between 0.0 and 1.0
/// - `top_failures` and `top_templates` are ordered by count (descending)
/// - `total_duration_ms` sums all execution durations in the range
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditSummary {
    /// Start of the time range.
    since: DateTime<Utc>,

    /// End of the time range.
    until: DateTime<Utc>,

    /// Total number of executions in the time range.
    total_executions: u64,

    /// Number of executions that completed successfully.
    success_count: u64,

    /// Number of executions that failed.
    failure_count: u64,

    /// Success rate as a float between 0.0 and 1.0.
    success_rate: f64,

    /// Sum of all execution durations in milliseconds.
    total_duration_ms: u64,

    /// Total tokens consumed across all executions (if tracked).
    #[serde(default)]
    total_tokens: Option<u64>,

    /// Most frequently failing templates or error patterns.
    top_failures: Vec<TopFailure>,

    /// Most frequently used templates.
    top_templates: Vec<TopTemplate>,
}

impl AuditSummary {
    /// Create a new AuditSummary.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        since: DateTime<Utc>,
        until: DateTime<Utc>,
        total_executions: u64,
        success_count: u64,
        failure_count: u64,
        success_rate: f64,
        total_duration_ms: u64,
        total_tokens: Option<u64>,
        top_failures: Vec<TopFailure>,
        top_templates: Vec<TopTemplate>,
    ) -> Self {
        Self {
            since,
            until,
            total_executions,
            success_count,
            failure_count,
            success_rate,
            total_duration_ms,
            total_tokens,
            top_failures,
            top_templates,
        }
    }

    /// Start of the time range.
    pub fn since(&self) -> &DateTime<Utc> {
        &self.since
    }

    /// End of the time range.
    pub fn until(&self) -> &DateTime<Utc> {
        &self.until
    }

    /// Total executions.
    pub fn total_executions(&self) -> u64 {
        self.total_executions
    }

    /// Successful executions.
    pub fn success_count(&self) -> u64 {
        self.success_count
    }

    /// Failed executions.
    pub fn failure_count(&self) -> u64 {
        self.failure_count
    }

    /// Success rate (0.0 to 1.0).
    pub fn success_rate(&self) -> f64 {
        self.success_rate
    }

    /// Total duration in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_ms
    }

    /// Total tokens consumed.
    pub fn total_tokens(&self) -> Option<u64> {
        self.total_tokens
    }

    /// Top failure patterns.
    pub fn top_failures(&self) -> &[TopFailure] {
        &self.top_failures
    }

    /// Top templates.
    pub fn top_templates(&self) -> &[TopTemplate] {
        &self.top_templates
    }
}

// ---------------------------------------------------------------------------
// TopFailure — most frequent failure pattern
// ---------------------------------------------------------------------------

/// A frequently occurring failure pattern in audit execution summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopFailure {
    /// Human-readable description of the failure pattern.
    description: String,

    /// Number of times this failure pattern occurred.
    count: u64,

    /// Optional template name associated with the failures.
    #[serde(default)]
    template_name: Option<String>,
}

impl TopFailure {
    /// Create a new TopFailure.
    pub fn new(description: String, count: u64, template_name: Option<String>) -> Self {
        Self {
            description,
            count,
            template_name,
        }
    }

    /// Failure description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Occurrence count.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Optional associated template name.
    pub fn template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }
}

// ---------------------------------------------------------------------------
// TopTemplate — most frequently used template
// ---------------------------------------------------------------------------

/// A frequently used template in audit execution summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopTemplate {
    /// Template name.
    name: String,

    /// Number of times this template was executed.
    count: u64,

    /// Average execution duration for this template in milliseconds.
    avg_duration_ms: u64,
}

impl TopTemplate {
    /// Create a new TopTemplate.
    pub fn new(name: String, count: u64, avg_duration_ms: u64) -> Self {
        Self {
            name,
            count,
            avg_duration_ms,
        }
    }

    /// Template name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Execution count.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Average duration in milliseconds.
    pub fn avg_duration_ms(&self) -> u64 {
        self.avg_duration_ms
    }
}
