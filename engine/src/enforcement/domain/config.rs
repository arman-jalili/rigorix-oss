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
        Self::standard()
    }
}

impl EnforcementConfig {
    // -----------------------------------------------------------------------
    // Preset Builders
    // -----------------------------------------------------------------------

    /// Build the Standard enforcement configuration.
    ///
    /// Suitable for normal operation. Provides reasonable safety limits
    /// without being overly restrictive.
    pub fn standard() -> Self {
        let mut budgets = HashMap::new();
        budgets.insert(
            "tokens".to_string(),
            ResourceBudget {
                resource: "tokens".to_string(),
                soft_warning_threshold: 0.8,
                hard_limit: 100_000,
                current_usage: 0,
            },
        );
        budgets.insert(
            "tool_calls".to_string(),
            ResourceBudget {
                resource: "tool_calls".to_string(),
                soft_warning_threshold: 0.8,
                hard_limit: 500,
                current_usage: 0,
            },
        );
        budgets.insert(
            "execution_time_ms".to_string(),
            ResourceBudget {
                resource: "execution_time_ms".to_string(),
                soft_warning_threshold: 0.85,
                hard_limit: 900_000, // 15 minutes
                current_usage: 0,
            },
        );

        let mut tool_policies = HashMap::new();
        tool_policies.insert(
            "bash".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::High,
                requires_confirmation: true,
                dry_run: false,
                max_calls: Some(100),
                budget_key: Some("tool_calls".to_string()),
            },
        );
        tool_policies.insert(
            "write".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::Medium,
                requires_confirmation: false,
                dry_run: false,
                max_calls: None,
                budget_key: Some("tool_calls".to_string()),
            },
        );
        tool_policies.insert(
            "read".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::Low,
                requires_confirmation: false,
                dry_run: false,
                max_calls: None,
                budget_key: None,
            },
        );

        Self {
            budgets,
            execution_limits: ExecutionLimits::default(),
            tool_policies,
            default_tool_policy: ToolPolicy::default(),
            preset: EnforcementPresetProfile::Standard,
        }
    }

    /// Build the Strict enforcement configuration.
    ///
    /// Suitable for production or untrusted code. Applies tighter
    /// budgets and more restrictive tool policies, requiring confirmation
    /// for medium-risk tools as well.
    pub fn strict() -> Self {
        let mut budgets = HashMap::new();
        budgets.insert(
            "tokens".to_string(),
            ResourceBudget {
                resource: "tokens".to_string(),
                soft_warning_threshold: 0.7,
                hard_limit: 50_000,
                current_usage: 0,
            },
        );
        budgets.insert(
            "tool_calls".to_string(),
            ResourceBudget {
                resource: "tool_calls".to_string(),
                soft_warning_threshold: 0.7,
                hard_limit: 200,
                current_usage: 0,
            },
        );
        budgets.insert(
            "execution_time_ms".to_string(),
            ResourceBudget {
                resource: "execution_time_ms".to_string(),
                soft_warning_threshold: 0.75,
                hard_limit: 600_000, // 10 minutes
                current_usage: 0,
            },
        );

        let mut tool_policies = HashMap::new();
        tool_policies.insert(
            "bash".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::Critical,
                requires_confirmation: true,
                dry_run: false,
                max_calls: Some(50),
                budget_key: Some("tool_calls".to_string()),
            },
        );
        tool_policies.insert(
            "write".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::High,
                requires_confirmation: true,
                dry_run: false,
                max_calls: Some(100),
                budget_key: Some("tool_calls".to_string()),
            },
        );
        tool_policies.insert(
            "read".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::Low,
                requires_confirmation: false,
                dry_run: false,
                max_calls: None,
                budget_key: None,
            },
        );

        Self {
            budgets,
            execution_limits: ExecutionLimits {
                max_tool_calls: 200,
                max_execution_time_secs: 1800,
                max_tokens: 50_000,
                max_retries_per_node: 2,
                max_concurrent_tools: 5,
            },
            tool_policies,
            default_tool_policy: ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::Medium,
                requires_confirmation: true,
                dry_run: false,
                max_calls: None,
                budget_key: None,
            },
            preset: EnforcementPresetProfile::Strict,
        }
    }

    /// Build the Maximum enforcement configuration.
    ///
    /// Suitable for high-risk operations. Applies the most restrictive
    /// budgets, limits, and policies. All state-modifying operations
    /// require confirmation and are heavily constrained.
    pub fn maximum() -> Self {
        let mut budgets = HashMap::new();
        budgets.insert(
            "tokens".to_string(),
            ResourceBudget {
                resource: "tokens".to_string(),
                soft_warning_threshold: 0.5,
                hard_limit: 20_000,
                current_usage: 0,
            },
        );
        budgets.insert(
            "tool_calls".to_string(),
            ResourceBudget {
                resource: "tool_calls".to_string(),
                soft_warning_threshold: 0.5,
                hard_limit: 50,
                current_usage: 0,
            },
        );
        budgets.insert(
            "execution_time_ms".to_string(),
            ResourceBudget {
                resource: "execution_time_ms".to_string(),
                soft_warning_threshold: 0.6,
                hard_limit: 600_000, // 10 minutes
                current_usage: 0,
            },
        );

        let mut tool_policies = HashMap::new();
        tool_policies.insert(
            "bash".to_string(),
            ToolPolicy {
                allowed: false,
                risk_level: ToolRiskLevel::Critical,
                requires_confirmation: true,
                dry_run: true,
                max_calls: Some(10),
                budget_key: Some("tool_calls".to_string()),
            },
        );
        tool_policies.insert(
            "write".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::High,
                requires_confirmation: true,
                dry_run: true,
                max_calls: Some(20),
                budget_key: Some("tool_calls".to_string()),
            },
        );
        tool_policies.insert(
            "read".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::Low,
                requires_confirmation: false,
                dry_run: false,
                max_calls: Some(200),
                budget_key: None,
            },
        );

        Self {
            budgets,
            execution_limits: ExecutionLimits {
                max_tool_calls: 50,
                max_execution_time_secs: 600,
                max_tokens: 20_000,
                max_retries_per_node: 1,
                max_concurrent_tools: 2,
            },
            tool_policies,
            default_tool_policy: ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::Medium,
                requires_confirmation: true,
                dry_run: true,
                max_calls: Some(10),
                budget_key: None,
            },
            preset: EnforcementPresetProfile::Maximum,
        }
    }

    // -----------------------------------------------------------------------
    // Builder Methods
    // -----------------------------------------------------------------------

    /// Create an EnforcementConfig from the given preset profile.
    pub fn from_preset(preset: &EnforcementPresetProfile) -> Self {
        match preset {
            EnforcementPresetProfile::Standard => Self::standard(),
            EnforcementPresetProfile::Strict => Self::strict(),
            EnforcementPresetProfile::Maximum => Self::maximum(),
        }
    }

    /// Add or override a resource budget.
    pub fn with_budget(mut self, budget: ResourceBudget) -> Self {
        self.budgets.insert(budget.resource.clone(), budget);
        self
    }

    /// Add or override a tool policy.
    pub fn with_tool_policy(mut self, tool: &str, policy: ToolPolicy) -> Self {
        self.tool_policies.insert(tool.to_string(), policy);
        self
    }

    /// Replace the execution limits.
    pub fn with_execution_limits(mut self, limits: ExecutionLimits) -> Self {
        self.execution_limits = limits;
        self
    }

    /// Replace the default tool policy.
    pub fn with_default_tool_policy(mut self, policy: ToolPolicy) -> Self {
        self.default_tool_policy = policy;
        self
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    /// Validate the configuration against safety caps.
    ///
    /// Checks that all values are within acceptable bounds.
    /// Returns a list of validation errors (empty if valid).
    pub fn validate(&self, safety_caps: &SafetyCaps) -> Vec<ConfigValidationError> {
        let mut errors = Vec::new();

        // Check execution limits against caps
        if self.execution_limits.max_tool_calls > safety_caps.max_tool_calls_cap {
            errors.push(ConfigValidationError {
                field: "execution_limits.max_tool_calls".to_string(),
                message: format!(
                    "max_tool_calls {} exceeds safety cap {}",
                    self.execution_limits.max_tool_calls, safety_caps.max_tool_calls_cap
                ),
                value: Some(self.execution_limits.max_tool_calls.to_string()),
            });
        }

        if self.execution_limits.max_execution_time_secs > safety_caps.max_timeout_secs_cap {
            errors.push(ConfigValidationError {
                field: "execution_limits.max_execution_time_secs".to_string(),
                message: format!(
                    "max_execution_time_secs {} exceeds safety cap {}",
                    self.execution_limits.max_execution_time_secs, safety_caps.max_timeout_secs_cap
                ),
                value: Some(self.execution_limits.max_execution_time_secs.to_string()),
            });
        }

        if (self.execution_limits.max_retries_per_node as u64) > safety_caps.max_retries_cap {
            errors.push(ConfigValidationError {
                field: "execution_limits.max_retries_per_node".to_string(),
                message: format!(
                    "max_retries_per_node {} exceeds safety cap {}",
                    self.execution_limits.max_retries_per_node, safety_caps.max_retries_cap
                ),
                value: Some(self.execution_limits.max_retries_per_node.to_string()),
            });
        }

        if (self.execution_limits.max_concurrent_tools as u64)
            > safety_caps.max_concurrent_tools_cap
        {
            errors.push(ConfigValidationError {
                field: "execution_limits.max_concurrent_tools".to_string(),
                message: format!(
                    "max_concurrent_tools {} exceeds safety cap {}",
                    self.execution_limits.max_concurrent_tools,
                    safety_caps.max_concurrent_tools_cap
                ),
                value: Some(self.execution_limits.max_concurrent_tools.to_string()),
            });
        }

        // Check budgets against caps
        for (name, budget) in &self.budgets {
            if budget.hard_limit > safety_caps.max_budget_cap {
                errors.push(ConfigValidationError {
                    field: format!("budgets.{}.hard_limit", name),
                    message: format!(
                        "hard_limit {} for budget '{}' exceeds safety cap {}",
                        budget.hard_limit, name, safety_caps.max_budget_cap
                    ),
                    value: Some(budget.hard_limit.to_string()),
                });
            }

            if budget.soft_warning_threshold < 0.0 || budget.soft_warning_threshold > 1.0 {
                errors.push(ConfigValidationError {
                    field: format!("budgets.{}.soft_warning_threshold", name),
                    message: format!(
                        "soft_warning_threshold {} for budget '{}' must be between 0.0 and 1.0",
                        budget.soft_warning_threshold, name
                    ),
                    value: Some(budget.soft_warning_threshold.to_string()),
                });
            }
        }

        errors
    }

    /// Check if the configuration is valid (no validation errors).
    pub fn is_valid(&self, safety_caps: &SafetyCaps) -> bool {
        self.validate(safety_caps).is_empty()
    }
}

/// Safety caps — hard upper bounds for configuration values.
///
/// These are the absolute maximum values that any enforcement
/// configuration is allowed to use. They prevent configuration
/// errors from creating dangerously permissive settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyCaps {
    /// Maximum allowed parallel tasks.
    pub max_parallel_tasks_cap: u64,

    /// Maximum tool calls across entire execution.
    pub max_tool_calls_cap: u64,

    /// Maximum allowed retries per node.
    pub max_retries_cap: u64,

    /// Maximum timeout in seconds.
    pub max_timeout_secs_cap: u64,

    /// Maximum LLM tokens per request.
    pub max_tokens_cap: u64,

    /// Maximum concurrent tool executions.
    pub max_concurrent_tools_cap: u64,

    /// Maximum budget hard limit for any resource.
    pub max_budget_cap: u64,
}

impl Default for SafetyCaps {
    fn default() -> Self {
        Self {
            max_parallel_tasks_cap: 10,
            max_tool_calls_cap: 1000,
            max_retries_cap: 5,
            max_timeout_secs_cap: 3600,
            max_tokens_cap: 200_000,
            max_concurrent_tools_cap: 20,
            max_budget_cap: 1_000_000,
        }
    }
}

/// A single configuration validation error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigValidationError {
    /// The field that failed validation.
    pub field: String,
    /// Human-readable error message.
    pub message: String,
    /// The invalid value, if representable.
    pub value: Option<String>,
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
