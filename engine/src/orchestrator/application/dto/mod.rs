//! Data Transfer Objects for the Orchestrator module.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#dtos
//! Implements: Contract Freeze — DTO schemas for run, plan_only, cancel, status
//! Issue: #338
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

use crate::identity::domain::IdentityClaim;
use crate::orchestrator::domain::record::{ExecutionRecord, ExecutionStatus};

// ---------------------------------------------------------------------------
// Run DTOs
// ---------------------------------------------------------------------------

/// Input for a full orchestrator run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInput {
    /// The user's natural-language intent for execution.
    pub intent: String,

    /// Serialized configuration for the run.
    pub config: serde_json::Value,

    /// Repository root path.
    pub repo_root: String,

    /// Repository name for audit (e.g. "my-org/my-repo").
    pub repository: Option<String>,

    /// Author identity for audit (e.g. email or username).
    pub author: Option<String>,

    /// Attributed identity claim for the run author (see identity module).
    /// Supersedes the self-asserted `author` string when present — flows into
    /// the audit envelope's redacted `identity` block.
    #[serde(default)]
    pub identity: Option<IdentityClaim>,

    /// Optional enforcement preset override.
    pub enforcement_preset: Option<String>,
}

/// Output from a full orchestrator run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutput {
    /// The execution ID assigned to this run.
    pub execution_id: uuid::Uuid,

    /// The complete execution record with all metadata.
    pub record: ExecutionRecord,
}

// ---------------------------------------------------------------------------
// Plan Only DTOs
// ---------------------------------------------------------------------------

/// Input for a plan-only operation (no execution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOnlyInput {
    /// The user's natural-language intent for planning.
    pub intent: String,

    /// Serialized configuration.
    pub config: serde_json::Value,

    /// Repository root path.
    pub repo_root: String,
}

/// Output from a plan-only operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOnlyOutput {
    /// Plan result from the planning pipeline.
    pub plan: serde_json::Value,

    /// The proposed TaskGraph structure.
    pub graph: serde_json::Value,

    /// Structured sequence-policy findings from R2 plan-time evaluation.
    ///
    /// Populated when a sequence-policy service is configured and a rule
    /// matched a `promote` sequence — surfaced to plan consumers (e.g. MCP
    /// `rigorix_validate_plan`) so the gating decision is visible **before**
    /// a run starts. Empty when no service is configured or no rule matched.
    /// (A matched `deny` rule refuses the plan instead — it is reported as an
    /// `OrchestratorError::SequencePolicyDenied`, never as a silent pass.)
    #[serde(default)]
    pub sequence_findings: Vec<SequencePolicyFinding>,
}

/// One matched sequence from plan-time evaluation (R2), surfaced to plan
/// preview consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencePolicyFinding {
    /// Stable id of the rule that matched.
    pub rule_id: String,
    /// Name of the later matched step that the rule gates.
    pub later_step: String,
    /// Action applied to the later step: `"promote"` (the step is built
    /// `requires_approval = true`) or `"deny"`.
    pub action: String,
}

// ---------------------------------------------------------------------------
// Run/Plan From Template DTOs — pre-resolved templates, skip intent→plan pipeline
// ---------------------------------------------------------------------------

/// A single step from a pre-resolved plan template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStepDef {
    pub name: String,
    pub tool: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub requires_approval: bool,
    pub timeout_secs: Option<u64>,
    /// Whether to run scored evaluation on this step's output.
    #[serde(default)]
    pub evaluate_score: bool,
}

/// Input for a full orchestrator run from a pre-resolved template.
///
/// Skips the planning pipeline (intent→classify→match→generate) because
/// the steps are already concrete. Everything else — state persistence,
/// DAG execution, quality gates, policy engine, audit dispatch — is
/// identical to a normal `run()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFromTemplateInput {
    /// Pre-resolved steps to execute.
    pub steps: Vec<TemplateStepDef>,

    /// Repository root path.
    pub repo_root: String,

    /// Optional execution ID (generated if not provided).
    pub execution_id: Option<uuid::Uuid>,

    /// Template name for audit and record metadata.
    pub template_name: String,

    /// Repository name for audit (e.g. "my-org/my-repo").
    pub repository: Option<String>,

    /// Author identity for audit (e.g. email or username).
    pub author: Option<String>,

    /// Optional enforcement preset override.
    pub enforcement_preset: Option<String>,
}

/// Input for a plan-from-template operation (no execution).
///
/// Builds a TaskGraph from pre-resolved steps and returns the graph
/// structure without executing anything. Used for preview/validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFromTemplateInput {
    /// Pre-resolved steps.
    pub steps: Vec<TemplateStepDef>,

    /// Repository root path.
    pub repo_root: String,

    /// Template name for metadata.
    pub template_name: String,
}

// ---------------------------------------------------------------------------
// Cancel DTOs
// ---------------------------------------------------------------------------

/// Input for cancelling a running execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelInput {
    /// The execution ID to cancel.
    pub execution_id: uuid::Uuid,

    /// Optional reason for cancellation.
    pub reason: Option<String>,
}

/// Output from a cancel operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOutput {
    /// The execution ID that was cancelled.
    pub execution_id: uuid::Uuid,

    /// Whether the execution was successfully aborted.
    pub aborted: bool,

    /// How many DAG nodes were cancelled mid-execution.
    pub nodes_cancelled: u32,
}

// ---------------------------------------------------------------------------
// Approval DTOs
// ---------------------------------------------------------------------------

/// Input for approving steps of a paused execution.
///
/// Grants human sign-off for steps that declared `requires_approval: true`
/// and resumes the paused execution once approval is recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveExecutionInput {
    /// The execution ID to approve steps for.
    pub execution_id: uuid::Uuid,

    /// Step (node) names to approve.
    pub step_names: Vec<String>,

    // ── ADR-011 approval binding (optional) ───────────────────────────────
    // With the binding enabled the approving identity is a required captured
    // fact (R3). When omitted the orchestrator falls back to the run's own
    // author identity if one was recorded; otherwise the engine denies.
    /// Human identity subject approving (see identity module).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approver_id: Option<String>,
    /// Role / policy id (captured fact, not a judgment).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    /// IdP token/claims presented at approval (credential-substitution check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_claims_ref: Option<String>,
}

/// Output from approving steps of a paused execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveExecutionOutput {
    /// The execution ID.
    pub execution_id: uuid::Uuid,

    /// Step names that were approved.
    pub approved: Vec<String>,

    /// Step names that could not be found in this execution.
    pub not_found: Vec<String>,

    /// Step names still awaiting approval after this call.
    pub still_pending: Vec<String>,

    /// Whether the paused execution resumed after approval.
    pub resumed: bool,
}

// ---------------------------------------------------------------------------
// Status DTOs
// ---------------------------------------------------------------------------

/// Output from a status query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusOutput {
    /// The current execution ID.
    pub execution_id: uuid::Uuid,

    /// Current execution status.
    pub status: ExecutionStatus,

    /// Per-node state information.
    pub nodes: Vec<NodeState>,
}

/// State of a single DAG node from a status query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    /// Unique identifier of the DAG node.
    pub node_id: String,

    /// Human-readable node name.
    pub node_name: String,

    /// Current status of this node.
    pub status: String,

    /// Human-readable status message.
    pub message: Option<String>,
}
