//! Implementation of the ExecutionEnforcer service.
//!
//! @canonical .pi/architecture/modules/enforcement.md#application
//! Implements: ISSUE-ENFORCEMENT-2 — ExecutionEnforcer runtime enforcement
//! Issue: #59
//!
//! Provides the concrete `ExecutionEnforcerImpl` that gates tool calls,
//! tracks resource budgets, checks execution limits, and manages
//! enforcement warnings at runtime.
//!
//! # Thread Safety
//! - Budget state is protected by `RwLock` for concurrent read/write
//! - All async methods are safe to call from multiple tasks
//! - `has_active_warnings()` and `active_warnings()` are sync and
//!   return snapshot data

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use chrono::Utc;

use crate::enforcement::application::dto::{
    ActiveWarning, BudgetSnapshot, CheckExecutionLimitsInput, CheckExecutionLimitsOutput,
    ConfigSummary, EvaluateToolCallInput, EvaluateToolCallOutput, GetBudgetStatusInput,
    GetBudgetStatusOutput, LimitStatus, ReloadConfigOutput, ResourceBudgetStatus,
    TrackResourceUsageInput, TrackResourceUsageOutput, ExecutionLimitsSummary,
};
use crate::enforcement::application::service::ExecutionEnforcer;
use crate::enforcement::domain::{
    EnforcementConfig, EnforcementError, ResourceBudget, ToolPolicy,
};

/// Internal mutable state for the execution enforcer.
struct EnforcerState {
    /// The enforcement configuration (budgets, limits, policies).
    config: EnforcementConfig,

    /// Active warnings keyed by resource name.
    warnings: HashMap<String, ActiveWarning>,

    /// The number of tool calls that have been made so far.
    tool_call_count: u64,

    /// Total execution time tracked in milliseconds.
    total_execution_time_ms: u64,

    /// Total tokens consumed (input + output).
    total_tokens: u64,
}

/// Concrete implementation of the `ExecutionEnforcer` trait.
///
/// Uses interior mutability (`RwLock`) to allow concurrent access
/// from multiple async tasks. The enforcer is typically shared via
/// `Arc<dyn ExecutionEnforcer>`.
pub struct ExecutionEnforcerImpl {
    /// The execution ID this enforcer is managing.
    execution_id: String,

    /// Internal state protected by a read-write lock.
    state: RwLock<EnforcerState>,

    /// Whether there are any active warnings (fast flag for `has_active_warnings()`).
    has_warnings: AtomicBool,
}

impl ExecutionEnforcerImpl {
    /// Create a new `ExecutionEnforcerImpl` from an `EnforcementConfig`.
    pub fn new(execution_id: &str, config: EnforcementConfig) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            state: RwLock::new(EnforcerState {
                config,
                warnings: HashMap::new(),
                tool_call_count: 0,
                total_execution_time_ms: 0,
                total_tokens: 0,
            }),
            has_warnings: AtomicBool::new(false),
        }
    }

    /// Get the tool policy for a given tool name.
    ///
    /// Checks tool-specific policies first, then falls back to the
    /// default tool policy.
    fn get_tool_policy(&self, state: &EnforcerState, tool: &str) -> ToolPolicy {
        state
            .config
            .tool_policies
            .get(tool)
            .cloned()
            .unwrap_or_else(|| state.config.default_tool_policy.clone())
    }

    /// Calculate the usage ratio for a budget, capped at 1.0.
    fn usage_ratio(used: u64, limit: u64) -> f64 {
        if limit == 0 {
            return 0.0;
        }
        (used as f64 / limit as f64).min(1.0)
    }

    /// Create a budget snapshot from a resource budget.
    fn build_budget_snapshot(budget: &ResourceBudget) -> BudgetSnapshot {
        let usage_ratio = Self::usage_ratio(budget.current_usage, budget.hard_limit);
        let warning_active = budget.current_usage as f64 / budget.hard_limit as f64
            >= budget.soft_warning_threshold;
        BudgetSnapshot {
            resource: budget.resource.clone(),
            used: budget.current_usage,
            limit: budget.hard_limit,
            usage_ratio,
            warning_active,
        }
    }

    /// Build a ResourceBudgetStatus from a budget entry.
    fn build_resource_budget_status(
        name: &str,
        budget: &ResourceBudget,
    ) -> ResourceBudgetStatus {
        let usage_ratio = Self::usage_ratio(budget.current_usage, budget.hard_limit);
        let warning_active = usage_ratio >= budget.soft_warning_threshold;
        let limit_reached = budget.current_usage >= budget.hard_limit;
        ResourceBudgetStatus {
            resource: name.to_string(),
            used: budget.current_usage,
            limit: budget.hard_limit,
            usage_ratio,
            warning_threshold: budget.soft_warning_threshold,
            warning_active,
            limit_reached,
        }
    }
}

#[async_trait]
impl ExecutionEnforcer for ExecutionEnforcerImpl {
    async fn evaluate_tool_call(
        &self,
        input: EvaluateToolCallInput,
    ) -> Result<EvaluateToolCallOutput, EnforcementError> {
        let state = self.state.read().map_err(|e| EnforcementError::InvalidState {
            detail: format!("Failed to read enforcer state: {}", e),
        })?;

        // 1. Get the tool policy
        let policy = self.get_tool_policy(&state, &input.tool);

        // 2. Check if the tool is allowed by policy
        if !policy.allowed {
            return Ok(EvaluateToolCallOutput {
                allowed: false,
                reason: Some(format!("Tool '{}' is not allowed by enforcement policy", input.tool)),
                risk_level: policy.risk_level,
                requires_confirmation: policy.requires_confirmation,
                dry_run: policy.dry_run,
                budget_status: None,
                active_warnings: state.warnings.keys().cloned().collect(),
            });
        }

        // 3. Check if the tool has exceeded its max calls (future: per-tool tracking)
        if let Some(_max_calls) = policy.max_calls {
            // Per-tool call count tracking is reserved for a future implementation.
        }

        // 4. Check budget for the tool's associated resource
        let budget_status = if let Some(budget_key) = &policy.budget_key {
            state.config.budgets.get(budget_key).map(Self::build_budget_snapshot)
        } else {
            None
        };

        // 5. Check if budget is exceeded
        if let Some(ref snapshot) = budget_status {
            if snapshot.used >= snapshot.limit {
                return Ok(EvaluateToolCallOutput {
                    allowed: false,
                    reason: Some(format!(
                        "Resource budget '{}' exhausted (used {}, limit {})",
                        snapshot.resource, snapshot.used, snapshot.limit
                    )),
                    risk_level: policy.risk_level,
                    requires_confirmation: policy.requires_confirmation,
                    dry_run: policy.dry_run,
                    budget_status: None,
                    active_warnings: state.warnings.keys().cloned().collect(),
                });
            }
        }

        // 6. Check execution limits
        if state.tool_call_count >= state.config.execution_limits.max_tool_calls {
            return Ok(EvaluateToolCallOutput {
                allowed: false,
                reason: Some(format!(
                    "Execution limit reached: max_tool_calls ({})",
                    state.config.execution_limits.max_tool_calls
                )),
                risk_level: policy.risk_level,
                requires_confirmation: policy.requires_confirmation,
                dry_run: policy.dry_run,
                budget_status: None,
                active_warnings: state.warnings.keys().cloned().collect(),
            });
        }

        // Tool call is allowed
        Ok(EvaluateToolCallOutput {
            allowed: true,
            reason: None,
            risk_level: policy.risk_level,
            requires_confirmation: policy.requires_confirmation,
            dry_run: policy.dry_run,
            budget_status,
            active_warnings: state.warnings.keys().cloned().collect(),
        })
    }

    async fn track_resource_usage(
        &self,
        input: TrackResourceUsageInput,
    ) -> Result<TrackResourceUsageOutput, EnforcementError> {
        let mut state = self.state.write().map_err(|e| EnforcementError::InvalidState {
            detail: format!("Failed to write enforcer state: {}", e),
        })?;

        // Extract budget data before mutating to avoid borrow conflicts
        let budget_data = {
            let budget = state.config.budgets.get_mut(&input.resource).ok_or_else(|| {
                EnforcementError::BudgetNotFound {
                    resource: input.resource.clone(),
                }
            })?;

            let previous_usage = budget.current_usage;
            budget.current_usage = budget.current_usage.saturating_add(input.amount);
            let limit = budget.hard_limit;
            let threshold = budget.soft_warning_threshold;
            let current_usage = budget.current_usage;

            let warning_threshold_crossed =
                previous_usage < (limit as f64 * threshold) as u64
                    && current_usage >= (limit as f64 * threshold) as u64;

            let limit_exceeded = current_usage >= limit;

            (previous_usage, current_usage, limit, threshold, warning_threshold_crossed, limit_exceeded)
        };

        let (previous_usage, current_usage, limit, threshold, warning_threshold_crossed, limit_exceeded) = budget_data;

        // Track warning if threshold was crossed
        if warning_threshold_crossed {
            let warning = ActiveWarning {
                resource: input.resource.clone(),
                used: current_usage,
                limit,
                threshold,
                triggered_at: Utc::now().to_rfc3339(),
            };
            state.warnings.insert(input.resource.clone(), warning);
            self.has_warnings.store(true, Ordering::SeqCst);
        }

        // Update aggregate counters based on resource type
        match input.resource.as_str() {
            "tool_calls" => state.tool_call_count = state.tool_call_count.saturating_add(input.amount),
            "execution_time_ms" => {
                state.total_execution_time_ms = state.total_execution_time_ms.saturating_add(input.amount);
            }
            "tokens" => state.total_tokens = state.total_tokens.saturating_add(input.amount),
            _ => {}
        }

        Ok(TrackResourceUsageOutput {
            previous_usage,
            current_usage,
            limit,
            warning_threshold_crossed,
            limit_exceeded,
        })
    }

    async fn get_budget_status(
        &self,
        input: GetBudgetStatusInput,
    ) -> Result<GetBudgetStatusOutput, EnforcementError> {
        let state = self.state.read().map_err(|e| EnforcementError::InvalidState {
            detail: format!("Failed to read enforcer state: {}", e),
        })?;

        let budgets: Vec<ResourceBudgetStatus> = state
            .config
            .budgets
            .iter()
            .filter(|(name, _)| {
                input
                    .resources
                    .as_ref()
                    .map_or(true, |resources| resources.contains(name))
            })
            .map(|(name, budget)| Self::build_resource_budget_status(name, budget))
            .collect();

        let has_exceeded_limits = budgets.iter().any(|b| b.limit_reached);

        Ok(GetBudgetStatusOutput {
            execution_id: self.execution_id.clone(),
            budgets,
            has_warnings: self.has_warnings.load(Ordering::SeqCst),
            has_exceeded_limits,
        })
    }

    async fn check_execution_limits(
        &self,
        input: CheckExecutionLimitsInput,
    ) -> Result<CheckExecutionLimitsOutput, EnforcementError> {
        let state = self.state.read().map_err(|e| EnforcementError::InvalidState {
            detail: format!("Failed to read enforcer state: {}", e),
        })?;

        let mut limits_reached = Vec::new();

        // Check tool call limit
        if state.tool_call_count >= state.config.execution_limits.max_tool_calls {
            limits_reached.push(LimitStatus {
                limit_type: "max_tool_calls".to_string(),
                current: state.tool_call_count,
                max: state.config.execution_limits.max_tool_calls,
                is_hard_limit: true,
                is_soft_limit: false,
            });
        }

        // Check execution time limit
        let exec_time_secs = state.total_execution_time_ms / 1000;
        if exec_time_secs >= state.config.execution_limits.max_execution_time_secs {
            limits_reached.push(LimitStatus {
                limit_type: "max_execution_time".to_string(),
                current: exec_time_secs,
                max: state.config.execution_limits.max_execution_time_secs,
                is_hard_limit: true,
                is_soft_limit: false,
            });
        }

        // Check token limit
        if state.total_tokens >= state.config.execution_limits.max_tokens {
            limits_reached.push(LimitStatus {
                limit_type: "max_tokens".to_string(),
                current: state.total_tokens,
                max: state.config.execution_limits.max_tokens,
                is_hard_limit: true,
                is_soft_limit: false,
            });
        }

        let should_terminate = !limits_reached.is_empty();

        Ok(CheckExecutionLimitsOutput {
            execution_id: input.execution_id,
            limits_reached,
            has_reached_limit: should_terminate,
            should_terminate,
        })
    }

    async fn reload_config(&self) -> Result<ReloadConfigOutput, EnforcementError> {
        let mut state = self.state.write().map_err(|e| EnforcementError::InvalidState {
            detail: format!("Failed to write enforcer state: {}", e),
        })?;

        // In a full implementation, this would reload from the repository.
        // For now, we rebuild from the existing preset.
        let preset = state.config.preset.clone();
        let new_config = EnforcementConfig::from_preset(&preset);

        let budget_count = new_config.budgets.len() as u32;
        let policy_count = new_config.tool_policies.len() as u32;
        let limits = new_config.execution_limits.clone();

        state.config = new_config;
        // Reset warnings since config changed
        state.warnings.clear();
        self.has_warnings.store(false, Ordering::SeqCst);

        Ok(ReloadConfigOutput {
            success: true,
            config_summary: ConfigSummary {
                preset: format!("{:?}", preset),
                budget_count,
                policy_count,
                limits: ExecutionLimitsSummary {
                    max_tool_calls: limits.max_tool_calls,
                    max_execution_time_secs: limits.max_execution_time_secs,
                    max_tokens: limits.max_tokens,
                    max_retries_per_node: limits.max_retries_per_node,
                    max_concurrent_tools: limits.max_concurrent_tools,
                },
            },
        })
    }

    fn has_active_warnings(&self) -> bool {
        self.has_warnings.load(Ordering::SeqCst)
    }

    fn active_warnings(&self) -> Vec<ActiveWarning> {
        self.state
            .read()
            .map(|state| state.warnings.values().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enforcement::application::dto::{
        CheckExecutionLimitsInput, EvaluateToolCallInput, GetBudgetStatusInput,
        TrackResourceUsageInput,
    };
    use crate::enforcement::domain::{EnforcementPresetProfile, ResourceBudget, ToolRiskLevel};

    fn create_test_enforcer() -> ExecutionEnforcerImpl {
        let config = EnforcementConfig::standard();
        ExecutionEnforcerImpl::new("test-exec-1", config)
    }

    fn create_strict_enforcer() -> ExecutionEnforcerImpl {
        let config = EnforcementConfig::strict();
        ExecutionEnforcerImpl::new("test-exec-2", config)
    }

    // -----------------------------------------------------------------------
    // evaluate_tool_call tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_evaluate_allowed_tool() {
        let enforcer = create_test_enforcer();
        let input = EvaluateToolCallInput {
            execution_id: "test-exec-1".to_string(),
            node_id: "node-1".to_string(),
            tool: "read".to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        };
        let output = enforcer.evaluate_tool_call(input).await.unwrap();
        assert!(output.allowed);
        assert_eq!(output.risk_level, ToolRiskLevel::Low);
        assert!(!output.requires_confirmation);
    }

    #[tokio::test]
    async fn test_evaluate_tool_not_allowed_by_policy() {
        // Use maximum preset which blocks bash by default
        let config = EnforcementConfig::maximum();
        let enforcer = ExecutionEnforcerImpl::new("test-exec", config);

        let input = EvaluateToolCallInput {
            execution_id: "test-exec".to_string(),
            node_id: "node-1".to_string(),
            tool: "bash".to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        };
        let output = enforcer.evaluate_tool_call(input).await.unwrap();
        assert!(!output.allowed);
        assert!(output.reason.is_some());
        assert!(output.reason.unwrap().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_evaluate_tool_requires_confirmation() {
        let enforcer = create_test_enforcer();
        let input = EvaluateToolCallInput {
            execution_id: "test-exec-1".to_string(),
            node_id: "node-1".to_string(),
            tool: "bash".to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        };
        let output = enforcer.evaluate_tool_call(input).await.unwrap();
        assert!(output.allowed);
        assert!(output.requires_confirmation);
        assert_eq!(output.risk_level, ToolRiskLevel::High);
    }

    #[tokio::test]
    async fn test_evaluate_tool_blocked_when_budget_exhausted() {
        let enforcer = create_strict_enforcer();

        // Exhaust the tool_calls budget
        let track_input = TrackResourceUsageInput {
            execution_id: "test-exec-2".to_string(),
            resource: "tool_calls".to_string(),
            amount: 200, // exact limit
            context: None,
        };
        enforcer.track_resource_usage(track_input).await.unwrap();

        // Now try to call a tool
        let input = EvaluateToolCallInput {
            execution_id: "test-exec-2".to_string(),
            node_id: "node-1".to_string(),
            tool: "read".to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        };
        let output = enforcer.evaluate_tool_call(input).await.unwrap();
        assert!(!output.allowed);
    }

    #[tokio::test]
    async fn test_evaluate_tool_blocked_when_execution_limit_reached() {
        let enforcer = create_strict_enforcer();

        // Set tool call count to the max
        for i in 0..200 {
            let track_input = TrackResourceUsageInput {
                execution_id: "test-exec-2".to_string(),
                resource: "tool_calls".to_string(),
                amount: 1,
                context: None,
            };
            enforcer.track_resource_usage(track_input).await.unwrap();
        }

        let input = EvaluateToolCallInput {
            execution_id: "test-exec-2".to_string(),
            node_id: "node-1".to_string(),
            tool: "read".to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        };
        let output = enforcer.evaluate_tool_call(input).await.unwrap();
        assert!(!output.allowed);
    }

    #[tokio::test]
    async fn test_evaluate_tool_allows_unknown_tools_with_default_policy() {
        let enforcer = create_test_enforcer();
        let input = EvaluateToolCallInput {
            execution_id: "test-exec-1".to_string(),
            node_id: "node-1".to_string(),
            tool: "unknown_tool".to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        };
        let output = enforcer.evaluate_tool_call(input).await.unwrap();
        // Unknown tools use the default policy which allows all
        assert!(output.allowed);
    }

    #[tokio::test]
    async fn test_evaluate_tool_with_active_warnings() {
        let enforcer = create_test_enforcer();

        // Exceed warning threshold for tokens budget
        let track_input = TrackResourceUsageInput {
            execution_id: "test-exec-1".to_string(),
            resource: "tokens".to_string(),
            amount: 85_000, // 85% of 100_000, exceeds 80% warning threshold
            context: None,
        };
        enforcer.track_resource_usage(track_input).await.unwrap();

        let input = EvaluateToolCallInput {
            execution_id: "test-exec-1".to_string(),
            node_id: "node-1".to_string(),
            tool: "read".to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        };
        let output = enforcer.evaluate_tool_call(input).await.unwrap();
        assert!(output.allowed);
        // Active warnings should be listed
        assert!(!output.active_warnings.is_empty());
    }

    // -----------------------------------------------------------------------
    // track_resource_usage tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_track_resource_usage_basic() {
        let enforcer = create_test_enforcer();
        let input = TrackResourceUsageInput {
            execution_id: "test-exec-1".to_string(),
            resource: "tokens".to_string(),
            amount: 500,
            context: None,
        };
        let output = enforcer.track_resource_usage(input).await.unwrap();
        assert_eq!(output.previous_usage, 0);
        assert_eq!(output.current_usage, 500);
        assert_eq!(output.limit, 100_000);
        assert!(!output.warning_threshold_crossed);
        assert!(!output.limit_exceeded);
    }

    #[tokio::test]
    async fn test_track_resource_usage_exceeds_limit() {
        let enforcer = create_strict_enforcer();
        let input = TrackResourceUsageInput {
            execution_id: "test-exec-2".to_string(),
            resource: "tokens".to_string(),
            amount: 60_000, // exceeds the strict limit of 50_000
            context: None,
        };
        let output = enforcer.track_resource_usage(input).await.unwrap();
        assert!(output.limit_exceeded);
    }

    #[tokio::test]
    async fn test_track_resource_usage_triggers_warning() {
        let enforcer = create_test_enforcer();
        // Standard tokens budget: soft_warning_threshold=0.8, hard_limit=100_000
        // 80_000 is exactly at the threshold — crossing it should trigger warning
        let input = TrackResourceUsageInput {
            execution_id: "test-exec-1".to_string(),
            resource: "tokens".to_string(),
            amount: 80_000,
            context: None,
        };
        let output = enforcer.track_resource_usage(input).await.unwrap();
        assert!(output.warning_threshold_crossed);
        assert!(enforcer.has_active_warnings());
        assert_eq!(enforcer.active_warnings().len(), 1);
    }

    #[tokio::test]
    async fn test_track_resource_usage_unknown_resource() {
        let enforcer = create_test_enforcer();
        let input = TrackResourceUsageInput {
            execution_id: "test-exec-1".to_string(),
            resource: "nonexistent".to_string(),
            amount: 100,
            context: None,
        };
        let result = enforcer.track_resource_usage(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EnforcementError::BudgetNotFound { resource } => {
                assert_eq!(resource, "nonexistent");
            }
            e => panic!("Expected BudgetNotFound, got: {}", e),
        }
    }

    #[tokio::test]
    async fn test_track_multiple_resources() {
        let enforcer = create_test_enforcer();

        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tool_calls".to_string(),
                amount: 10,
                context: None,
            })
            .await
            .unwrap();

        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tokens".to_string(),
                amount: 5000,
                context: None,
            })
            .await
            .unwrap();

        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec-1".to_string(),
                resources: None,
            })
            .await
            .unwrap();

        assert_eq!(status.budgets.len(), 3);
        let tool_calls = status.budgets.iter().find(|b| b.resource == "tool_calls").unwrap();
        assert_eq!(tool_calls.used, 10);

        let tokens = status.budgets.iter().find(|b| b.resource == "tokens").unwrap();
        assert_eq!(tokens.used, 5000);
    }

    // -----------------------------------------------------------------------
    // get_budget_status tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_budget_status_all() {
        let enforcer = create_test_enforcer();
        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec-1".to_string(),
                resources: None,
            })
            .await
            .unwrap();

        assert_eq!(status.budgets.len(), 3);
        assert!(!status.has_warnings);
        assert!(!status.has_exceeded_limits);
    }

    #[tokio::test]
    async fn test_get_budget_status_filtered() {
        let enforcer = create_test_enforcer();
        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec-1".to_string(),
                resources: Some(vec!["tokens".to_string()]),
            })
            .await
            .unwrap();

        assert_eq!(status.budgets.len(), 1);
        assert_eq!(status.budgets[0].resource, "tokens");
    }

    #[tokio::test]
    async fn test_get_budget_status_shows_limit_reached() {
        let enforcer = create_strict_enforcer();

        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-2".to_string(),
                resource: "tokens".to_string(),
                amount: 50_000,
                context: None,
            })
            .await
            .unwrap();

        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec-2".to_string(),
                resources: Some(vec!["tokens".to_string()]),
            })
            .await
            .unwrap();

        assert!(status.has_exceeded_limits);
        assert!(status.budgets[0].limit_reached);
        assert_eq!(status.budgets[0].usage_ratio, 1.0);
    }

    // -----------------------------------------------------------------------
    // check_execution_limits tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_check_execution_limits_no_limits_reached() {
        let enforcer = create_test_enforcer();
        let result = enforcer
            .check_execution_limits(CheckExecutionLimitsInput {
                execution_id: "test-exec-1".to_string(),
            })
            .await
            .unwrap();

        assert!(!result.has_reached_limit);
        assert!(!result.should_terminate);
        assert!(result.limits_reached.is_empty());
    }

    #[tokio::test]
    async fn test_check_execution_limits_tool_call_limit() {
        let enforcer = create_strict_enforcer();

        // Exceed the strict tool call limit of 200
        for _ in 0..200 {
            enforcer
                .track_resource_usage(TrackResourceUsageInput {
                    execution_id: "test-exec-2".to_string(),
                    resource: "tool_calls".to_string(),
                    amount: 1,
                    context: None,
                })
                .await
                .unwrap();
        }

        // One more to exceed
        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-2".to_string(),
                resource: "tool_calls".to_string(),
                amount: 1,
                context: None,
            })
            .await
            .unwrap();

        let result = enforcer
            .check_execution_limits(CheckExecutionLimitsInput {
                execution_id: "test-exec-2".to_string(),
            })
            .await
            .unwrap();

        assert!(result.has_reached_limit);
        assert!(result.should_terminate);
        assert!(result.limits_reached.iter().any(|l| l.limit_type == "max_tool_calls"));
    }

    #[tokio::test]
    async fn test_check_execution_limits_token_limit() {
        let enforcer = create_strict_enforcer();

        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-2".to_string(),
                resource: "tokens".to_string(),
                amount: 50_000,
                context: None,
            })
            .await
            .unwrap();

        let result = enforcer
            .check_execution_limits(CheckExecutionLimitsInput {
                execution_id: "test-exec-2".to_string(),
            })
            .await
            .unwrap();

        assert!(result.has_reached_limit);
        assert!(result.limits_reached.iter().any(|l| l.limit_type == "max_tokens"));
    }

    // -----------------------------------------------------------------------
    // reload_config tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_reload_config_standard() {
        let enforcer = create_test_enforcer();

        // First track some usage
        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tokens".to_string(),
                amount: 50_000,
                context: None,
            })
            .await
            .unwrap();

        // Reload should reset to preset defaults
        let output = enforcer.reload_config().await.unwrap();
        assert!(output.success);
        assert_eq!(output.config_summary.preset, "Standard");

        // Budgets should be reset
        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec-1".to_string(),
                resources: Some(vec!["tokens".to_string()]),
            })
            .await
            .unwrap();

        assert_eq!(status.budgets[0].used, 0);
    }

    // -----------------------------------------------------------------------
    // has_active_warnings / active_warnings tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_no_warnings_initially() {
        let enforcer = create_test_enforcer();
        assert!(!enforcer.has_active_warnings());
        assert!(enforcer.active_warnings().is_empty());
    }

    #[tokio::test]
    async fn test_warning_after_threshold_crossed() {
        let enforcer = create_test_enforcer();

        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tokens".to_string(),
                amount: 80_000,
                context: None,
            })
            .await
            .unwrap();

        assert!(enforcer.has_active_warnings());
        let warnings = enforcer.active_warnings();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].resource, "tokens");
        assert_eq!(warnings[0].used, 80_000);
    }

    #[tokio::test]
    async fn test_multiple_warnings() {
        let enforcer = create_test_enforcer();

        // Cross threshold for tokens
        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tokens".to_string(),
                amount: 80_000,
                context: None,
            })
            .await
            .unwrap();

        // Cross threshold for tool_calls (80% of 500 = 400)
        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tool_calls".to_string(),
                amount: 400,
                context: None,
            })
            .await
            .unwrap();

        assert!(enforcer.has_active_warnings());
        assert_eq!(enforcer.active_warnings().len(), 2);
    }

    // -----------------------------------------------------------------------
    // Standard/Strict/Maximum preset integration tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_standard_preset_tool_policies() {
        let enforcer = create_test_enforcer();

        // Bash: allowed but requires confirmation
        let bash = enforcer
            .evaluate_tool_call(EvaluateToolCallInput {
                execution_id: "test-exec-1".to_string(),
                node_id: "n1".to_string(),
                tool: "bash".to_string(),
                arguments: None,
                is_retry: false,
                attempt: 1,
            })
            .await
            .unwrap();
        assert!(bash.allowed);
        assert!(bash.requires_confirmation);
        assert_eq!(bash.risk_level, ToolRiskLevel::High);

        // Read: allowed without confirmation
        let read = enforcer
            .evaluate_tool_call(EvaluateToolCallInput {
                execution_id: "test-exec-1".to_string(),
                node_id: "n1".to_string(),
                tool: "read".to_string(),
                arguments: None,
                is_retry: false,
                attempt: 1,
            })
            .await
            .unwrap();
        assert!(read.allowed);
        assert!(!read.requires_confirmation);
        assert_eq!(read.risk_level, ToolRiskLevel::Low);
    }

    #[tokio::test]
    async fn test_maximum_preset_blocks_bash() {
        let config = EnforcementConfig::maximum();
        let enforcer = ExecutionEnforcerImpl::new("test-exec", config);

        let bash = enforcer
            .evaluate_tool_call(EvaluateToolCallInput {
                execution_id: "test-exec".to_string(),
                node_id: "n1".to_string(),
                tool: "bash".to_string(),
                arguments: None,
                is_retry: false,
                attempt: 1,
            })
            .await
            .unwrap();
        assert!(!bash.allowed);
    }

    // -----------------------------------------------------------------------
    // Factory tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_factory_create_default() {
        use crate::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
        use crate::enforcement::application::factory::ExecutionEnforcerFactory;

        let factory = ExecutionEnforcerFactoryImpl;
        let enforcer = factory.create_default("test-exec").await.unwrap();

        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec".to_string(),
                resources: None,
            })
            .await
            .unwrap();
        assert_eq!(status.budgets.len(), 3);
    }

    #[tokio::test]
    async fn test_factory_create_from_config() {
        use crate::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
        use crate::enforcement::application::factory::ExecutionEnforcerFactory;

        let factory = ExecutionEnforcerFactoryImpl;
        let config = EnforcementConfig::strict();
        let enforcer = factory.create_from_config("test-exec", config).await.unwrap();

        let bash = enforcer
            .evaluate_tool_call(EvaluateToolCallInput {
                execution_id: "test-exec".to_string(),
                node_id: "n1".to_string(),
                tool: "bash".to_string(),
                arguments: None,
                is_retry: false,
                attempt: 1,
            })
            .await
            .unwrap();
        assert!(bash.requires_confirmation);
        assert_eq!(bash.risk_level, ToolRiskLevel::Critical);
    }

    #[tokio::test]
    async fn test_factory_create_with_custom_budgets() {
        use crate::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
        use crate::enforcement::application::factory::ExecutionEnforcerFactory;

        let factory = ExecutionEnforcerFactoryImpl;
        let mut budgets = std::collections::HashMap::new();
        budgets.insert(
            "custom_budget".to_string(),
            ResourceBudget {
                resource: "custom_budget".to_string(),
                soft_warning_threshold: 0.9,
                hard_limit: 50,
                current_usage: 0,
            },
        );

        let enforcer = factory
            .create_with_custom_budgets("test-exec", EnforcementConfig::standard(), budgets)
            .await
            .unwrap();

        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec".to_string(),
                resources: None,
            })
            .await
            .unwrap();

        assert!(status.budgets.iter().any(|b| b.resource == "custom_budget"));
        assert_eq!(status.budgets.len(), 4); // 3 standard + 1 custom
    }

    #[tokio::test]
    async fn test_factory_create_with_tool_overrides() {
        use crate::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
        use crate::enforcement::application::factory::ExecutionEnforcerFactory;

        let factory = ExecutionEnforcerFactoryImpl;
        let mut overrides = std::collections::HashMap::new();
        overrides.insert(
            "read".to_string(),
            crate::enforcement::domain::ToolPolicy {
                allowed: false,
                ..crate::enforcement::domain::ToolPolicy::default()
            },
        );

        let enforcer = factory
            .create_with_tool_overrides("test-exec", EnforcementConfig::standard(), overrides)
            .await
            .unwrap();

        let read = enforcer
            .evaluate_tool_call(EvaluateToolCallInput {
                execution_id: "test-exec".to_string(),
                node_id: "n1".to_string(),
                tool: "read".to_string(),
                arguments: None,
                is_retry: false,
                attempt: 1,
            })
            .await
            .unwrap();
        assert!(!read.allowed);
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_concurrent_evaluate_and_track() {
        let enforcer = std::sync::Arc::new(create_test_enforcer());
        let mut handles = Vec::new();

        for i in 0..10 {
            let enforcer = enforcer.clone();
            handles.push(tokio::spawn(async move {
                // Evaluate and track concurrently
                let eval = enforcer
                    .evaluate_tool_call(EvaluateToolCallInput {
                        execution_id: "test-exec-1".to_string(),
                        node_id: format!("node-{}", i),
                        tool: "read".to_string(),
                        arguments: None,
                        is_retry: false,
                        attempt: 1,
                    })
                    .await
                    .unwrap();
                assert!(eval.allowed);

                enforcer
                    .track_resource_usage(TrackResourceUsageInput {
                        execution_id: "test-exec-1".to_string(),
                        resource: "tool_calls".to_string(),
                        amount: 1,
                        context: None,
                    })
                    .await
                    .unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let status = enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: "test-exec-1".to_string(),
                resources: Some(vec!["tool_calls".to_string()]),
            })
            .await
            .unwrap();

        assert_eq!(status.budgets[0].used, 10);
    }

    #[tokio::test]
    async fn test_track_zero_usage_no_change() {
        let enforcer = create_test_enforcer();
        let output = enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tokens".to_string(),
                amount: 0,
                context: None,
            })
            .await
            .unwrap();

        assert_eq!(output.previous_usage, 0);
        assert_eq!(output.current_usage, 0);
        assert!(!output.warning_threshold_crossed);
    }

    #[tokio::test]
    async fn test_reload_config_clears_warnings() {
        let enforcer = create_test_enforcer();

        // Cross warning threshold
        enforcer
            .track_resource_usage(TrackResourceUsageInput {
                execution_id: "test-exec-1".to_string(),
                resource: "tokens".to_string(),
                amount: 80_000,
                context: None,
            })
            .await
            .unwrap();

        assert!(enforcer.has_active_warnings());

        // Reload should clear warnings
        enforcer.reload_config().await.unwrap();
        assert!(!enforcer.has_active_warnings());
    }
}
