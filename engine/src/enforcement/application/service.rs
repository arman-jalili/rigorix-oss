//! Service interfaces (use cases) for the Enforcement bounded context.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — ExecutionEnforcer trait
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for enforcement:
//! tool call evaluation, resource budget tracking, execution limit checking,
//! and policy management. All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::enforcement::domain::EnforcementError;

use super::dto::{
    CheckExecutionLimitsInput, CheckExecutionLimitsOutput, EvaluateToolCallInput,
    EvaluateToolCallOutput, GetBudgetStatusInput, GetBudgetStatusOutput, ReloadConfigOutput,
    TrackResourceUsageInput, TrackResourceUsageOutput,
};

/// Central enforcement service that gates tool calls and tracks resource usage.
///
/// The ExecutionEnforcer sits between the ParallelExecutor (which executes
/// DAG nodes) and the tool execution environment. Every tool call passes
/// through the enforcer, which:
///
/// 1. Checks if the tool is allowed by policy
/// 2. Checks if resource budgets have remaining capacity
/// 3. Checks if execution limits have been reached
/// 4. Tracks resource consumption after tool execution
///
/// # Cancellation Integration
///
/// The enforcer cooperates with the Cancellation module:
/// - When a hard limit is reached, the enforcer may request cancellation
///   via the orchestrator, not directly through the enforcer API
/// - Budget warnings are informational; enforcement actions (blocking calls,
///   terminating execution) are the enforcer's responsibility
#[async_trait]
pub trait ExecutionEnforcer: Send + Sync {
    /// Evaluate whether a tool call is allowed to execute.
    ///
    /// Checks the tool policy, current budget state, and execution limits.
    /// Returns an assessment with the decision and reasoning.
    ///
    /// If the tool is blocked, returns `EnforcementError::ToolBlocked` or
    /// `EnforcementError::BudgetExceeded` as appropriate.
    async fn evaluate_tool_call(
        &self,
        input: EvaluateToolCallInput,
    ) -> Result<EvaluateToolCallOutput, EnforcementError>;

    /// Track resource usage after a tool call completes.
    ///
    /// Updates the tracked budget(s) for the specified resource.
    /// If a hard limit is exceeded, returns `EnforcementError::BudgetExceeded`.
    ///
    /// Returns the updated budget state so callers can check thresholds.
    async fn track_resource_usage(
        &self,
        input: TrackResourceUsageInput,
    ) -> Result<TrackResourceUsageOutput, EnforcementError>;

    /// Get the current budget status for all tracked resources.
    ///
    /// Returns usage vs. limits for every registered resource budget.
    async fn get_budget_status(
        &self,
        input: GetBudgetStatusInput,
    ) -> Result<GetBudgetStatusOutput, EnforcementError>;

    /// Check whether any execution limits have been reached.
    ///
    /// Returns a list of any limits that have been exceeded or are at
    /// their threshold. If no limits are breached, returns an empty list.
    async fn check_execution_limits(
        &self,
        input: CheckExecutionLimitsInput,
    ) -> Result<CheckExecutionLimitsOutput, EnforcementError>;

    /// Reload enforcement configuration from the latest source.
    ///
    /// Updates budgets, limits, and tool policies in-place.
    /// Returns the new configuration summary.
    async fn reload_config(&self) -> Result<ReloadConfigOutput, EnforcementError>;

    /// Check whether the enforcer has any pending warnings.
    ///
    /// Returns true if any resource budget has crossed its soft threshold
    /// but not yet reached its hard limit.
    fn has_active_warnings(&self) -> bool;

    /// Get a summary of all active warnings.
    fn active_warnings(&self) -> Vec<super::dto::ActiveWarning>;
}
