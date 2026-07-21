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
