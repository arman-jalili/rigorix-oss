//! Event payload schemas for the Enforcement bounded context.
//!
//! @canonical .pi/architecture/decisions/ADR-005-event-bus-persistence.md
//! Implements: Contract Freeze — EnforcementEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the `EventBus` whenever enforcement actions
//! are taken — tool calls evaluated, budgets updated, limits reached.
//! Consumers (orchestrator, audit, TUI) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Events emitted by the Enforcement module (ExecutionEnforcer).
///
/// Wrapped in `ExecutionEvent::enforcement(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnforcementEvent {
    /// A tool call was evaluated by the enforcer.
    ///
    /// Emitted for every tool call attempt, whether allowed or blocked.
    ToolEvaluated {
        /// The execution ID being enforced.
        execution_id: String,
        /// Identifier of the DAG node requesting the tool.
        node_id: String,
        /// The name of the tool being evaluated.
        tool: String,
        /// The risk level assigned to this tool.
        risk_level: String,
        /// Whether the tool was allowed to execute.
        allowed: bool,
        /// If blocked, the reason.
        reason: Option<String>,
    },

    /// A resource budget was updated (usage changed).
    ///
    /// Emitted whenever the enforcer tracks resource consumption.
    BudgetUpdated {
        /// The execution ID being enforced.
        execution_id: String,
        /// The resource whose budget was updated (e.g., "tokens", "tool_calls").
        resource: String,
        /// Previous usage value.
        previous_usage: u64,
        /// New usage value after the update.
        current_usage: u64,
        /// The hard limit for this resource.
        limit: u64,
        /// Whether the soft warning threshold was crossed.
        warning_threshold_crossed: bool,
    },

    /// A soft warning threshold was crossed for a resource budget.
    ///
    /// Execution continues — this is informational.
    BudgetWarning {
        /// The execution ID being enforced.
        execution_id: String,
        /// The resource nearing its limit (e.g., "tokens", "tool_calls", "execution_time_ms").
        resource: String,
        /// Current usage of the resource.
        used: u64,
        /// The hard limit for this resource.
        limit: u64,
        /// The soft threshold that was crossed (fraction of limit).
        threshold: f64,
    },

    /// An execution hard limit was reached.
    ///
    /// Execution will be terminated or the action will be blocked.
    HardLimitReached {
        /// The execution ID being enforced.
        execution_id: String,
        /// Type of limit reached (e.g., "max_tool_calls", "max_tokens", "max_execution_time").
        limit_type: String,
        /// Current value when the limit was reached.
        current: u64,
        /// The maximum allowed value.
        max: u64,
        /// Whether execution will be terminated as a result.
        terminates_execution: bool,
    },

    /// The enforcement configuration was loaded or reloaded.
    ConfigLoaded {
        /// The execution ID.
        execution_id: String,
        /// The enforcement preset used.
        preset: String,
        /// Number of resource budgets defined.
        budget_count: u32,
        /// Number of tool policies defined.
        policy_count: u32,
        /// Execution limits applied.
        limits: HashMap<String, u64>,
    },
}
