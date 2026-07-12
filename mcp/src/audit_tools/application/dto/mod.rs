//! Data Transfer Objects for the Audit Tools module.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#dto
//! Implements: Contract Freeze — all input/output DTO schemas
//!
//! DTOs define the input/output contracts for all service operations.
//! They carry documentation and validation metadata but no behavior.
//!
//! # Contract (Frozen)
//!
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ReadAudit DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix_read_audit` tool call.
///
/// Contains the execution ID to look up and an optional format preference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadAuditInput {
    /// The execution ID to query (UUID v4 string format).
    pub execution_id: String,

    /// Optional output format: "text" (default) or "json".
    pub format: Option<String>,
}

/// Output from `rigorix_read_audit` formatted as structured JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadAuditOutput {
    /// Unique execution identifier.
    pub execution_id: Uuid,

    /// Overall execution status.
    pub status: String,

    /// Template name, if available.
    #[serde(default)]
    pub template_name: Option<String>,

    /// Timestamp when execution started.
    pub started_at: String,

    /// Timestamp when execution completed.
    pub completed_at: String,

    /// Total execution duration in milliseconds.
    pub duration_ms: u64,

    /// Per-step results in plan order.
    pub steps: Vec<StepResultDto>,

    /// Optional token usage count.
    #[serde(default)]
    pub tokens_used: Option<u64>,

    /// URI to the persistent audit record.
    pub audit_uri: String,
}

// ---------------------------------------------------------------------------
// ListAudits DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix_list_audits` tool call.
///
/// All filter parameters are optional — unset fields are not filtered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAuditsInput {
    /// Filter by execution status ("Completed", "Failed", etc.).
    pub status: Option<String>,

    /// Include records on or after this ISO 8601 timestamp.
    pub since: Option<String>,

    /// Include records on or before this ISO 8601 timestamp.
    pub until: Option<String>,

    /// Filter by template name (exact match).
    pub template: Option<String>,

    /// Maximum number of records to return (default: 50, max: 200).
    pub limit: Option<u32>,
}

/// Output from `rigorix_list_audits` formatted as structured JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAuditsOutput {
    /// Total number of records matching the filter.
    pub total_count: usize,

    /// List of audit records (limited by `limit` parameter).
    pub audits: Vec<AuditSummaryItem>,
}

/// Summary item in a list of audit records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummaryItem {
    /// Unique execution identifier.
    pub execution_id: Uuid,

    /// Overall execution status.
    pub status: String,

    /// Template name, if available.
    #[serde(default)]
    pub template_name: Option<String>,

    /// Timestamp when execution started.
    pub started_at: String,

    /// Total execution duration in milliseconds.
    pub duration_ms: u64,

    /// URI to the persistent audit record.
    pub audit_uri: String,
}

// ---------------------------------------------------------------------------
// AuditSummary DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix_audit_summary` tool call.
///
/// Time range is optional — defaults to last 7 days.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummaryInput {
    /// Start of the time range (ISO 8601). Defaults to 7 days ago.
    pub since: Option<String>,

    /// End of the time range (ISO 8601). Defaults to current time.
    pub until: Option<String>,
}

/// Output from `rigorix_audit_summary` formatted as structured JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummaryOutput {
    /// Start of the time range.
    pub since: String,

    /// End of the time range.
    pub until: String,

    /// Total number of executions.
    pub total_executions: u64,

    /// Number of successful executions.
    pub success_count: u64,

    /// Number of failed executions.
    pub failure_count: u64,

    /// Success rate as a float (0.0 to 1.0).
    pub success_rate: f64,

    /// Total duration in milliseconds.
    pub total_duration_ms: u64,

    /// Total tokens consumed, if tracked.
    #[serde(default)]
    pub total_tokens: Option<u64>,

    /// Most frequent failure patterns.
    pub top_failures: Vec<TopFailureDto>,

    /// Most frequently used templates.
    pub top_templates: Vec<TopTemplateDto>,
}

/// DTO for a top failure pattern in audit summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopFailureDto {
    /// Human-readable failure description.
    pub description: String,

    /// Number of occurrences.
    pub count: u64,

    /// Optional associated template name.
    #[serde(default)]
    pub template_name: Option<String>,
}

/// DTO for a top template in audit summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopTemplateDto {
    /// Template name.
    pub name: String,

    /// Number of executions.
    pub count: u64,

    /// Average execution duration in milliseconds.
    pub avg_duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Shared DTOs
// ---------------------------------------------------------------------------

/// Per-step result DTO for audit responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResultDto {
    /// Step name.
    pub step_name: String,

    /// Whether the step succeeded.
    pub success: bool,

    /// Optional error message if step failed.
    #[serde(default)]
    pub error: Option<String>,

    /// Step output data.
    pub output: serde_json::Value,

    /// Duration in milliseconds.
    pub duration_ms: u64,
}
