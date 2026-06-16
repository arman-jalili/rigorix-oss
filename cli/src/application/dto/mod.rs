//! Data Transfer Objects for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CLI DTO schemas
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for CLI service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for CI/CD output)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types

use serde::{Deserialize, Serialize};

use crate::domain::event::SessionOutcome;

// ---------------------------------------------------------------------------
// CLI Run DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix run` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInput {
    /// The natural language intent or template ID to execute.
    ///
    /// Examples: "add endpoint", "fix bug", or a registered template ID.
    pub intent: String,

    /// Whether to output the plan as JSON without executing.
    pub dry_run: bool,

    /// Whether to skip risk gating confirmation prompts.
    ///
    /// Useful for CI/CD pipelines where no interactive prompts are possible.
    pub skip_confirmations: bool,

    /// Whether to skip budget pre-checks.
    pub skip_budget_check: bool,
}

/// Output from the `rigorix run` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutput {
    /// The execution session ID.
    pub session_id: String,

    /// The final outcome of the run.
    pub outcome: SessionOutcome,

    /// Summary of node execution results.
    pub summary: ExecutionSummary,
}

// ---------------------------------------------------------------------------
// CLI Plan DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix plan` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanInput {
    /// The natural language intent or template ID to plan.
    pub intent: String,
}

/// A single node in the plan DAG preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNodePreview {
    /// The node identifier.
    pub id: String,
    /// Human-readable label for the node.
    pub label: String,
    /// The tool type assigned to this node.
    pub tool: String,
    /// List of node IDs that this node depends on.
    pub depends_on: Vec<String>,
    /// Estimated cost in tokens.
    pub estimated_tokens: Option<u64>,
    /// Estimated cost in LLM calls.
    pub estimated_calls: Option<u32>,
}

/// Output from the `rigorix plan` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOutput {
    /// The template ID that was matched.
    pub template_id: String,
    /// The template name.
    pub template_name: String,
    /// Confidence score (0.0–1.0) of the template match.
    pub confidence: f64,
    /// The DAG nodes in execution order.
    pub nodes: Vec<PlanNodePreview>,
    /// Total estimated token cost.
    pub total_estimated_tokens: u64,
    /// Total estimated LLM calls.
    pub total_estimated_calls: u32,
    /// Whether budget limits would be exceeded.
    pub budget_exceeded: bool,
    /// Whether the plan is valid and ready to execute.
    pub is_valid: bool,
}

// ---------------------------------------------------------------------------
// CLI Generate DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix generate` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateInput {
    /// The natural language description of the template to generate.
    pub intent: String,

    /// Print generated template to stdout instead of saving.
    pub stdout: bool,

    /// Validate only — don't generate.
    pub dry_run: bool,
}

/// Output from the `rigorix generate` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateOutput {
    /// The generated template ID.
    pub template_id: String,
    /// Path where the template was saved (empty if `--stdout`).
    pub saved_path: Option<String>,
    /// Whether the template was persisted to disk.
    pub persisted: bool,
    /// The template TOML content (for --stdout or --dry-run).
    pub content: String,
}

// ---------------------------------------------------------------------------
// CLI Init DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix init` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitInput {
    /// Path where the `.rigorix/` directory should be created.
    ///
    /// Defaults to the current working directory.
    pub target_path: String,

    /// Whether to run in interactive mode with prompts.
    pub interactive: bool,

    /// API key to write into the config (interactive mode prompts for this).
    pub api_key: Option<String>,

    /// Enforcement preset to configure (interactive mode prompts for this).
    pub enforcement_preset: Option<String>,
}

/// Output from the `rigorix init` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitOutput {
    /// Path where `.rigorix/` was created.
    pub created_path: String,
    /// Files that were created.
    pub files_created: Vec<String>,
    /// Whether the API key was configured.
    pub api_key_configured: bool,
}

// ---------------------------------------------------------------------------
// CLI History DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix history` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryListInput {
    /// Maximum number of sessions to list.
    pub limit: Option<u32>,
    /// Filter by outcome status.
    pub status: Option<SessionOutcome>,
}

/// A single execution session summary for history listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// The session identifier.
    pub session_id: String,
    /// The command used to start the session.
    pub command: String,
    /// The template ID, if any.
    pub template_id: Option<String>,
    /// The final outcome.
    pub outcome: SessionOutcome,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Wall-clock timestamp.
    pub timestamp: String,
}

/// Output from the `rigorix history` list command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryListOutput {
    /// Session summaries ordered by timestamp descending.
    pub sessions: Vec<SessionSummary>,
    /// Total number of matching sessions.
    pub total: u32,
}

/// Input for `rigorix history show <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryShowInput {
    /// The session ID to show details for.
    pub session_id: String,
}

/// Output from `rigorix history show <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryShowOutput {
    /// Full session details.
    pub session: SessionSummary,
    /// Per-node execution results.
    pub nodes: Vec<NodeExecutionResult>,
}

/// Result of a single node execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionResult {
    /// The node identifier.
    pub node_id: String,
    /// Human-readable label.
    pub label: String,
    /// Whether the node completed successfully.
    pub success: bool,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Error message if the node failed.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// CLI Logs DTOs
// ---------------------------------------------------------------------------

/// Input for the `rigorix logs` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsInput {
    /// Session ID to replay logs from. If empty, follows live execution.
    pub session_id: Option<String>,

    /// Filter by event type (e.g., "node_failed").
    pub event_type: Option<String>,

    /// Filter by node ID.
    pub node_id: Option<String>,

    /// Filter by minimum severity.
    pub min_severity: Option<String>,

    /// Follow live execution events.
    pub follow: bool,

    /// Maximum number of events to return.
    pub limit: Option<u32>,
}

/// A single log entry from the event stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// The event type.
    pub event_type: String,
    /// The node ID, if applicable.
    pub node_id: Option<String>,
    /// Severity level.
    pub severity: String,
    /// Human-readable message.
    pub message: String,
    /// Wall-clock timestamp.
    pub timestamp: String,
    /// Structured metadata (JSON value).
    pub metadata: Option<serde_json::Value>,
}

/// Output from the `rigorix logs` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsOutput {
    /// The log entries.
    pub entries: Vec<LogEntry>,
    /// Total matching entries.
    pub total: u32,
}

// ---------------------------------------------------------------------------
// CLI Audit DTOs
// ---------------------------------------------------------------------------

/// Input for `rigorix audit list` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditListInput {
    /// Maximum number of audit envelopes to list.
    pub limit: Option<u32>,
}

/// Summary of an audit envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    /// The audit envelope ID.
    pub audit_id: String,
    /// The session ID.
    pub session_id: String,
    /// The planning hash (for plan diff comparisons).
    pub planning_hash: String,
    /// Wall-clock timestamp.
    pub timestamp: String,
}

/// Output from `rigorix audit list` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditListOutput {
    /// Audit summaries ordered by timestamp descending.
    pub audits: Vec<AuditSummary>,
    /// Total matching audits.
    pub total: u32,
}

/// Input for `rigorix audit show <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditShowInput {
    /// The audit envelope ID to show.
    pub audit_id: String,
}

/// Output from `rigorix audit show <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditShowOutput {
    /// The full audit envelope.
    pub audit: AuditSummary,
    /// The events in the envelope.
    pub events: Vec<serde_json::Value>,
}

/// Input for `rigorix audit diff <id1> <id2>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDiffInput {
    /// The first audit envelope ID.
    pub audit_id_1: String,
    /// The second audit envelope ID.
    pub audit_id_2: String,
}

/// Output from `rigorix audit diff` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDiffOutput {
    /// Whether the two plans are identical.
    pub identical: bool,
    /// The planning hash of the first audit.
    pub planning_hash_1: String,
    /// The planning hash of the second audit.
    pub planning_hash_2: String,
    /// Human-readable diff description.
    pub diff_description: String,
}

// ---------------------------------------------------------------------------
// Template List/Show DTOs
// ---------------------------------------------------------------------------

/// Input for `rigorix template list` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateListInput {
    // No fields needed — lists all available templates
}

/// Summary of a registered template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSummary {
    /// The template ID.
    pub id: String,
    /// The template name.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Whether this is a built-in template.
    pub built_in: bool,
}

/// Output from `rigorix template list` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateListOutput {
    /// Available templates.
    pub templates: Vec<TemplateSummary>,
    /// Total count.
    pub total: u32,
}

/// Input for `rigorix template show <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateShowInput {
    /// The template ID to show.
    pub template_id: String,
}

/// Output from `rigorix template show <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateShowOutput {
    /// The template TOML definition.
    pub content: String,
}

// ---------------------------------------------------------------------------
// Execution Summary DTO
// ---------------------------------------------------------------------------

/// Summary of node executions in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Total number of nodes in the plan.
    pub total_nodes: u32,
    /// Number of nodes that completed successfully.
    pub completed: u32,
    /// Number of nodes that failed.
    pub failed: u32,
    /// Number of nodes that were skipped.
    pub skipped: u32,
    /// Total duration in milliseconds.
    pub total_duration_ms: u64,
}
