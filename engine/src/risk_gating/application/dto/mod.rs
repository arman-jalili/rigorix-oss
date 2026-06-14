//! Data Transfer Objects for the Risk Gating module.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — DTO schemas for risk-gating operations
//! Issue: issue-contract-freeze
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

use crate::risk_gating::domain::risk_level::{GatingAction, RiskLevel};
use crate::risk_gating::domain::RiskConfig;

// ---------------------------------------------------------------------------
// Evaluate Gate DTOs
// ---------------------------------------------------------------------------

/// Input for evaluating the risk gate for a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateGateInput {
    /// The execution ID requesting the gate evaluation.
    pub execution_id: String,

    /// Identifier of the DAG node making the request.
    pub node_id: String,

    /// The name of the tool being called (e.g., "file_read", "run_command").
    pub tool: String,

    /// The arguments being passed to the tool (for context-aware classification).
    pub parameters: Option<serde_json::Value>,

    /// Whether this is a retry of a previously gated call.
    pub is_retry: bool,
}

/// Output from evaluating the risk gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateGateOutput {
    /// The risk level assigned to the tool.
    pub risk_level: RiskLevel,

    /// The gating action to apply.
    pub gating_action: GatingAction,

    /// Whether the tool is allowed to proceed based on current gate state.
    pub allowed: bool,

    /// Human-readable reason for the decision.
    pub reason: String,

    /// Whether this classification came from a configured override.
    pub from_override: bool,

    /// A unique identifier for this gate evaluation for tracking/resolution.
    pub gate_id: String,

    /// Active warnings that may be relevant.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Classify Tool DTOs
// ---------------------------------------------------------------------------

/// Input for classifying a tool (without gate evaluation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyToolInput {
    /// The name of the tool to classify.
    pub tool: String,

    /// Optional parameters for context-aware classification.
    pub parameters: Option<serde_json::Value>,
}

/// Output from classifying a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyToolOutput {
    /// The risk level assigned.
    pub risk_level: RiskLevel,

    /// Human-readable reason for the classification.
    pub reason: String,

    /// Whether this came from a configured override.
    pub from_override: bool,
}

// ---------------------------------------------------------------------------
// Resolve Gate DTOs
// ---------------------------------------------------------------------------

/// Input for resolving a pending gate (approve or reject).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveGateInput {
    /// The execution ID.
    pub execution_id: String,

    /// The gate ID returned from `evaluate_gate`.
    pub gate_id: String,

    /// Whether to approve (true) or reject (false) the gated operation.
    pub approved: bool,

    /// Optional reason from the user for the decision.
    pub reason: Option<String>,
}

/// Output from resolving a gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveGateOutput {
    /// The gate ID that was resolved.
    pub gate_id: String,

    /// Whether the gate was approved.
    pub approved: bool,

    /// Whether the tool can now proceed.
    pub can_proceed: bool,
}

// ---------------------------------------------------------------------------
// Get Config DTOs
// ---------------------------------------------------------------------------

/// Output from getting the current risk configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfigOutput {
    /// The current risk configuration.
    pub config: RiskConfig,

    /// Number of active tool overrides.
    pub override_count: u32,
}

// ---------------------------------------------------------------------------
// Override Tool DTOs
// ---------------------------------------------------------------------------

/// Input for overriding a tool's risk level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideToolInput {
    /// The execution ID.
    pub execution_id: String,

    /// The name of the tool to override.
    pub tool: String,

    /// The new risk level.
    pub new_level: RiskLevel,

    /// Optional reason for the override.
    pub reason: Option<String>,
}

/// Output from overriding a tool's risk level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideToolOutput {
    /// The tool that was overridden.
    pub tool: String,

    /// The new risk level.
    pub new_level: RiskLevel,

    /// The previous risk level, if one existed.
    pub previous_level: Option<RiskLevel>,

    /// Whether the override was applied successfully.
    pub applied: bool,
}

// ---------------------------------------------------------------------------
// Reload Config DTOs
// ---------------------------------------------------------------------------

/// Output from reloading risk configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadConfigOutput {
    /// Whether the reload was successful.
    pub success: bool,

    /// Summary of the loaded configuration.
    pub config_summary: RiskConfigSummary,
}

/// Summary of risk configuration after load/reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfigSummary {
    /// Number of tool overrides loaded.
    pub override_count: u32,

    /// Gating policy flags.
    pub auto_confirm_low: bool,
    pub require_review_medium: bool,
    pub dry_run_high: bool,
}

// ---------------------------------------------------------------------------
// Gate State DTOs
// ---------------------------------------------------------------------------

/// A snapshot of a pending gate's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingGate {
    /// Unique gate identifier.
    pub gate_id: String,
    /// The execution ID.
    pub execution_id: String,
    /// The node ID that requested the gate.
    pub node_id: String,
    /// The tool being gated.
    pub tool: String,
    /// The risk level that triggered the gate.
    pub risk_level: RiskLevel,
    /// The gating action required.
    pub action: GatingAction,
    /// ISO 8601 timestamp when the gate was created.
    pub created_at: String,
    /// Whether the gate has been resolved.
    pub resolved: bool,
}

/// Status of risk gating for an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStatus {
    /// Whether any gates are currently pending.
    pub has_pending_gates: bool,
    /// List of pending gates.
    pub pending_gates: Vec<PendingGate>,
    /// Total gates resolved in this execution.
    pub total_resolved: u32,
    /// Total gates approved.
    pub total_approved: u32,
    /// Total gates rejected.
    pub total_rejected: u32,
}
