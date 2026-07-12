//! Domain events for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#events
//! Implements: Contract Freeze — execution-tools event payload schemas
//!
//! These events are emitted throughout plan execution, validation, and
//! enforcement checking. Consumers (observability, telemetry, audit trail)
//! subscribe to these event types.
//!
//! # Event Catalog
//!
//! | Event | Trigger | Published By |
//! |-------|---------|-------------|
//! | PlanExecutionStarted | ExecuteHandler on receiving valid plan | ExecuteHandler |
//! | PlanExecutionCompleted | ExecuteHandler after engine returns result | ExecuteHandler |
//! | PlanValidated | ValidatePlanHandler after validation | ValidatePlanHandler |
//! | EnforcementChecked | CheckEnforcementHandler after status query | CheckEnforcementHandler |
//!
//! # Contract (Frozen)
//!
//! - Every event carries an execution_id or session_id and timestamp for correlation
//! - Serialized as tagged union with `#[serde(tag = "type")]`
//! - Events are facts — immutable and append-only
//! - No behavior — pure data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::value::ExecutionStatus;

/// All domain events emitted by the Execution Tools bounded context.
///
/// Each variant represents a meaningful domain occurrence.
/// Consumers use these events for observability, logging, and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionToolsEvent {
    /// A plan execution was initiated via `rigorix_execute`.
    PlanExecutionStarted {
        /// Execution identifier.
        execution_id: Uuid,
        /// Name of the plan template (if provided).
        template_name: Option<String>,
        /// Number of steps in the plan.
        step_count: usize,
        /// Timestamp of execution start.
        started_at: DateTime<Utc>,
    },

    /// A plan execution completed (success, failure, or partial).
    PlanExecutionCompleted {
        /// Execution identifier.
        execution_id: Uuid,
        /// Final execution status.
        status: ExecutionStatus,
        /// Total duration in milliseconds.
        duration_ms: u64,
        /// Token count used (if tracked).
        token_count: Option<u64>,
        /// Timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// A plan was validated via `rigorix_validate_plan`.
    PlanValidated {
        /// Session identifier.
        session_id: Uuid,
        /// Whether the plan is valid.
        is_valid: bool,
        /// Number of warning messages.
        warning_count: usize,
        /// Number of error messages.
        error_count: usize,
        /// Optional estimated cost.
        estimated_cost: Option<u64>,
        /// Timestamp of validation.
        timestamp: DateTime<Utc>,
    },

    /// Enforcement status was queried via `rigorix_check_enforcement`.
    EnforcementChecked {
        /// Session identifier.
        session_id: Uuid,
        /// Active enforcement preset name.
        preset: String,
        /// Remaining budget.
        budget: EnforcementBudgetPayload,
        /// Number of active circuit breakers.
        circuit_breaker_count: usize,
        /// Timestamp of the check.
        timestamp: DateTime<Utc>,
    },
}

/// Budget information payload for the EnforcementChecked event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementBudgetPayload {
    /// Remaining tool calls.
    pub tool_calls_remaining: u64,
    /// Remaining tokens.
    pub tokens_remaining: u64,
}
