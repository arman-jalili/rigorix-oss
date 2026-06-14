//! Event payload schemas for the Budget Tracking bounded context.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md#events
//! Implements: Contract Freeze — BudgetEvent payload schemas
//! Issue: #68
//!
//! Budget events describe state changes in LLM budget tracking: reservations,
//! commits, rollbacks, and exhaustion warnings. These can be correlated with
//! the `ExecutionEvent::BudgetWarning` at the orchestration layer.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use serde::{Deserialize, Serialize};

/// Events emitted by the Budget Tracking module.
///
/// Wrapped in `ExecutionEvent::BudgetWarning(...)` at the orchestration layer
/// when budget thresholds are crossed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BudgetEvent {
    /// A budget reservation was successfully created.
    ReservationCreated {
        /// The execution ID this budget belongs to.
        execution_id: String,
        /// The call identifier for this reservation.
        call_id: u32,
        /// Number of tokens reserved.
        reserved_tokens: u32,
        /// Remaining call capacity after this reservation.
        remaining_calls: u32,
        /// Remaining token capacity after this reservation.
        remaining_tokens: u32,
    },

    /// A budget reservation was committed with actual token usage.
    ReservationCommitted {
        /// The execution ID this budget belongs to.
        execution_id: String,
        /// The call identifier for this reservation.
        call_id: u32,
        /// Number of tokens reserved (estimate).
        reserved_tokens: u32,
        /// Actual tokens consumed by the LLM call.
        actual_tokens: u32,
        /// Token difference (actual - reserved). Positive = overage, negative = underage.
        token_delta: i64,
    },

    /// A budget reservation was rolled back (e.g. on Drop without commit).
    ReservationRolledBack {
        /// The execution ID this budget belongs to.
        execution_id: String,
        /// The call identifier for this reservation.
        call_id: u32,
        /// Number of tokens that were reserved and now released.
        released_tokens: u32,
        /// Reason for rollback (e.g. "LLM call panicked", "call cancelled").
        reason: String,
    },

    /// A budget reservation was rejected because a limit was reached.
    ReservationRejected {
        /// The execution ID this budget belongs to.
        execution_id: String,
        /// Which limit was hit: "max_calls" or "max_tokens".
        limit_type: String,
        /// Current usage when rejected.
        used: u32,
        /// Maximum allowed.
        max: u32,
        /// Number of tokens that were requested.
        requested: u32,
    },

    /// The soft warning threshold for a resource was crossed.
    BudgetWarning {
        /// The execution ID this budget belongs to.
        execution_id: String,
        /// Which budget resource triggered the warning: "calls" or "tokens".
        resource: String,
        /// Current usage of the resource.
        used: u32,
        /// Soft warning threshold value.
        threshold: u32,
        /// Maximum allowed.
        max: u32,
        /// Usage as a fraction of the limit (0.0–1.0).
        usage_ratio: f64,
    },

    /// The budget has been fully exhausted (hard limit reached).
    BudgetExhausted {
        /// The execution ID this budget belongs to.
        execution_id: String,
        /// Which budget resource was exhausted: "calls" or "tokens".
        resource: String,
        /// Final usage when exhausted.
        used: u32,
        /// Maximum allowed.
        max: u32,
    },
}
