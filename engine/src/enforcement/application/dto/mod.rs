//! Data Transfer Objects for the Enforcement module.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — DTO schemas for enforcement operations
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

use crate::enforcement::domain::{EnforcementConfig, ToolRiskLevel};

// ---------------------------------------------------------------------------
// Evaluate Tool Call DTOs
// ---------------------------------------------------------------------------

/// Input for evaluating whether a tool call is allowed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateToolCallInput {
    /// The execution ID requesting the tool.
    pub execution_id: String,

    /// Identifier of the DAG node making the request.
    pub node_id: String,

    /// The name of the tool being called (e.g., "bash", "write", "read").
    pub tool: String,

    /// The arguments being passed to the tool (for context-aware evaluation).
    pub arguments: Option<serde_json::Value>,

    /// Whether this is a retry of a previously failed tool call.
    pub is_retry: bool,

    /// The current attempt number (1-indexed).
    pub attempt: u32,
}

/// Output from evaluating a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateToolCallOutput {
    /// Whether the tool call is allowed to proceed.
    pub allowed: bool,

    /// The reason for the decision (if blocked or warned).
    pub reason: Option<String>,

    /// The risk level assigned to this tool.
    pub risk_level: ToolRiskLevel,

    /// Whether user confirmation is required before execution.
    pub requires_confirmation: bool,

    /// Whether the tool should be executed in dry-run mode.
    pub dry_run: bool,

    /// Current budget status for this tool's associated resource, if any.
    pub budget_status: Option<BudgetSnapshot>,

    /// Active warnings that may be relevant to this decision.
    pub active_warnings: Vec<String>,
}

/// A snapshot of a resource budget at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    /// The resource name.
    pub resource: String,
    /// Current usage.
    pub used: u64,
    /// Hard limit.
    pub limit: u64,
    /// Usage as a fraction of the limit (0.0–1.0).
    pub usage_ratio: f64,
    /// Whether the soft warning threshold has been crossed.
    pub warning_active: bool,
}

// ---------------------------------------------------------------------------
// Track Resource Usage DTOs
// ---------------------------------------------------------------------------

/// Input for tracking resource consumption after a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackResourceUsageInput {
    /// The execution ID.
    pub execution_id: String,

    /// The resource to update (e.g., "tokens", "tool_calls", "execution_time_ms").
    pub resource: String,

    /// The amount to add to current usage.
    /// Must be >= 0. Use 0 to report no consumption.
    pub amount: u64,

    /// Additional context about this resource usage (e.g., token breakdown).
    pub context: Option<serde_json::Value>,
}

/// Output from tracking resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackResourceUsageOutput {
    /// Previous usage value before the update.
    pub previous_usage: u64,

    /// Current usage value after the update.
    pub current_usage: u64,

    /// The hard limit for this resource.
    pub limit: u64,

    /// Whether the soft warning threshold was crossed by this update.
    pub warning_threshold_crossed: bool,

    /// Whether the hard limit was exceeded by this update.
    pub limit_exceeded: bool,
}

// ---------------------------------------------------------------------------
// Get Budget Status DTOs
// ---------------------------------------------------------------------------

/// Input for querying budget status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBudgetStatusInput {
    /// The execution ID to query budgets for.
    pub execution_id: String,

    /// Optional filter for specific resources. If empty, all budgets are returned.
    pub resources: Option<Vec<String>>,
}

/// Output from querying budget status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBudgetStatusOutput {
    /// The execution ID.
    pub execution_id: String,

    /// Current budget status for all tracked resources.
    pub budgets: Vec<ResourceBudgetStatus>,

    /// Whether any warnings are currently active.
    pub has_warnings: bool,

    /// Whether any hard limits have been reached.
    pub has_exceeded_limits: bool,
}

/// Status of a single resource budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudgetStatus {
    /// The resource name.
    pub resource: String,
    /// Current usage.
    pub used: u64,
    /// Hard limit.
    pub limit: u64,
    /// Usage as a fraction of the limit (0.0–1.0).
    pub usage_ratio: f64,
    /// The soft warning threshold fraction.
    pub warning_threshold: f64,
    /// Whether the warning threshold has been crossed.
    pub warning_active: bool,
    /// Whether the hard limit has been reached or exceeded.
    pub limit_reached: bool,
}

// ---------------------------------------------------------------------------
// Check Execution Limits DTOs
// ---------------------------------------------------------------------------

/// Input for checking execution limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckExecutionLimitsInput {
    /// The execution ID to check limits for.
    pub execution_id: String,
}

/// Output from checking execution limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckExecutionLimitsOutput {
    /// The execution ID.
    pub execution_id: String,

    /// List of limits that have been reached or exceeded.
    pub limits_reached: Vec<LimitStatus>,

    /// Whether any limit has been reached (shortcut for !limits_reached.is_empty()).
    pub has_reached_limit: bool,

    /// Whether execution should be terminated due to a hard limit.
    pub should_terminate: bool,
}

/// Status of a single execution limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitStatus {
    /// The limit type (e.g., "max_tool_calls", "max_tokens", "max_execution_time").
    pub limit_type: String,
    /// Current value.
    pub current: u64,
    /// Maximum allowed value.
    pub max: u64,
    /// Whether this limit is a hard limit (terminates execution).
    pub is_hard_limit: bool,
    /// Whether this is a soft limit (warning only).
    pub is_soft_limit: bool,
}

// ---------------------------------------------------------------------------
// Reload Config DTOs
// ---------------------------------------------------------------------------

/// Output from reloading enforcement configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadConfigOutput {
    /// Whether the reload was successful.
    pub success: bool,

    /// Summary of the loaded configuration.
    pub config_summary: ConfigSummary,
}

/// Summary of enforcement configuration after load/reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    /// The enforcement preset profile loaded.
    pub preset: String,

    /// Number of resource budgets defined.
    pub budget_count: u32,

    /// Number of tool policies defined.
    pub policy_count: u32,

    /// The execution limits that were applied.
    pub limits: ExecutionLimitsSummary,
}

/// Summary of execution limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLimitsSummary {
    pub max_tool_calls: u64,
    pub max_execution_time_secs: u64,
    pub max_tokens: u64,
    pub max_retries_per_node: u32,
    pub max_concurrent_tools: u32,
}

// ---------------------------------------------------------------------------
// Active Warnings
// ---------------------------------------------------------------------------

/// An active budget warning that hasn't yet been resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveWarning {
    /// The resource that triggered the warning.
    pub resource: String,
    /// Current usage.
    pub used: u64,
    /// The hard limit.
    pub limit: u64,
    /// The soft threshold that was crossed.
    pub threshold: f64,
    /// ISO 8601 timestamp when the warning was first triggered.
    pub triggered_at: String,
}

// ---------------------------------------------------------------------------
// Init DTOs
// ---------------------------------------------------------------------------

/// Input for initializing the ExecutionEnforcer with a configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitEnforcerInput {
    /// The execution ID to initialize enforcement for.
    pub execution_id: String,

    /// The enforcement configuration to use.
    pub config: EnforcementConfig,

    /// Whether to start with all budgets at zero (fresh execution).
    /// If false, budgets may be restored from a previous state.
    pub fresh_start: bool,
}

/// Output from initializing the ExecutionEnforcer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitEnforcerOutput {
    /// The execution ID.
    pub execution_id: String,
    /// Number of resource budgets initialized.
    pub budget_count: u32,
    /// Number of tool policies loaded.
    pub policy_count: u32,
}
