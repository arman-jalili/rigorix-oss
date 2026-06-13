//! EnforcementConfig domain entity.
//!
//! @canonical .pi/architecture/modules/enforcement.md#config
//! Implements: Contract Freeze — EnforcementConfig aggregate with budgets, limits, policies
//! Issue: issue-contract-freeze
//!
//! Defines the resource budgets, execution limits, and policy rules that the
//! ExecutionEnforcer uses to gate tool calls and track resource consumption
//! during execution.
//!
//! # Contract (Frozen)
//! - `EnforcementConfig` is the root aggregate for all enforcement settings
//! - Loaded from `Config.enforcement` (the `EnforcementPreset` selects which
//!   config profile to use)
//! - All fields are public for direct access by application services
//! - Construction happens via the EnforcerFactory or from serialized config

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enforcement configuration aggregate.
///
/// Defines the resource budgets, execution limits, and policy rules that
/// govern tool call evaluation and resource tracking during execution.
/// Selected by the `EnforcementPreset` in the top-level `Config`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnforcementConfig {
    /// Resource budgets keyed by resource name.
    ///
    /// Each entry defines a named budget (e.g., "tokens", "tool_calls",
    /// "execution_time_ms") with a soft threshold (warning) and a hard limit.
    pub budgets: HashMap<String, ResourceBudget>,

    /// Execution limits that constrain overall execution behavior.
    pub execution_limits: ExecutionLimits,

    /// Per-tool policy overrides keyed by tool name.
    ///
    /// If a tool is not listed, the default policy applies.
    pub tool_policies: HashMap<String, ToolPolicy>,

    /// Default policy applied to tools without a specific override.
    pub default_tool_policy: ToolPolicy,

    /// The enforcement preset that selected this configuration.
    pub preset: EnforcementPresetProfile,
}

impl Default for EnforcementConfig {
    fn default() -> Self {
        Self {
            budgets: HashMap::new(),
            execution_limits: ExecutionLimits::default(),
            tool_policies: HashMap::new(),
            default_tool_policy: ToolPolicy::default(),
            preset: EnforcementPresetProfile::Standard,
        }
    }
}

/// A resource budget with soft warning threshold and hard limit.
///
/// - `soft_warning_threshold`: Percentage (0.0–1.0) at which a
///   `BudgetWarning` event is emitted. Example: 0.8 = warn at 80% usage.
/// - `hard_limit`: Absolute maximum. When reached, the executing tool call
///   is blocked and an enforcement action is taken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceBudget {
    /// The name of the resource being tracked (e.g., "tokens", "tool_calls").
    pub resource: String,

    /// Soft warning threshold as a fraction of the limit (0.0–1.0).
    /// When usage crosses this threshold, a `BudgetWarning` event is emitted.
    pub soft_warning_threshold: f64,

    /// Hard limit for this resource. When reached, enforcement actions are taken.
    pub hard_limit: u64,

    /// Current usage of this resource (runtime state, updated by the enforcer).
    #[serde(default)]
    pub current_usage: u64,
}

/// Limits that constrain overall execution behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionLimits {
    /// Maximum number of tool calls allowed across the entire execution.
    pub max_tool_calls: u64,

    /// Maximum total execution time in seconds.
    pub max_execution_time_secs: u64,

    /// Maximum total LLM tokens consumed (input + output).
    pub max_tokens: u64,

    /// Maximum number of retries per node.
    pub max_retries_per_node: u32,

    /// Maximum number of concurrent tool executions.
    pub max_concurrent_tools: u32,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_tool_calls: 500,
            max_execution_time_secs: 3600,
            max_tokens: 100_000,
            max_retries_per_node: 3,
            max_concurrent_tools: 10,
        }
    }
}

/// Policy rules for a specific tool (or default policy).
///
/// Determines whether a tool call is allowed, requires review, or is
/// blocked entirely based on its risk level and current budget state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPolicy {
    /// Whether this tool is allowed to execute at all.
    pub allowed: bool,

    /// The risk level assigned to this tool.
    pub risk_level: ToolRiskLevel,

    /// If true, this tool requires explicit user confirmation before execution.
    pub requires_confirmation: bool,

    /// If true, this tool is executed in dry-run mode (no side effects).
    pub dry_run: bool,

    /// Optional maximum number of times this tool can be called.
    pub max_calls: Option<u64>,

    /// Optional budget name that this tool consumes from.
    /// If `None`, no budget tracking is applied to this tool.
    pub budget_key: Option<String>,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            allowed: true,
            risk_level: ToolRiskLevel::Medium,
            requires_confirmation: false,
            dry_run: false,
            max_calls: None,
            budget_key: None,
        }
    }
}

/// Risk level assigned to a tool or operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolRiskLevel {
    /// Read-only operations with no side effects.
    Low,
    /// Operations that modify state but are reversible.
    Medium,
    /// Operations that have irreversible side effects.
    High,
    /// Operations that could cause data loss or security issues.
    Critical,
}

/// Enforcement preset profile — the concrete values selected by
/// `EnforcementPreset` (defined in `crate::configuration::domain`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EnforcementPresetProfile {
    /// Standard safety limits — suitable for normal operation.
    Standard,
    /// Stricter limits — suitable for production or untrusted code.
    Strict,
    /// Maximum safety limits — suitable for high-risk operations.
    Maximum,
}

impl Default for EnforcementPresetProfile {
    fn default() -> Self {
        Self::Standard
    }
}
