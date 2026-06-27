//! Data Transfer Objects for the Budget Tracking module.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: Contract Freeze — DTO schemas for reserve, commit, status operations
//! Issue: #68
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

use crate::budget_tracking::domain::LlmBudgetReservationState;

// ---------------------------------------------------------------------------
// Reserve Budget DTOs
// ---------------------------------------------------------------------------

/// Input for reserving budget for an LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveBudgetInput {
    /// The execution ID to associate this reservation with.
    pub execution_id: uuid::Uuid,

    /// Estimated number of tokens the LLM call will consume.
    ///
    /// Must be > 0. Should be a best-effort estimate based on
    /// the prompt length and expected output size.
    pub estimated_tokens: u32,

    /// Optional label for this specific call (e.g., "classify", "extract").
    pub call_label: Option<String>,
}

/// Output from a successful budget reservation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveBudgetOutput {
    /// Snapshot of the reservation state.
    pub reservation: LlmBudgetReservationState,

    /// Remaining call capacity after this reservation.
    pub remaining_calls: u32,

    /// Remaining token capacity after this reservation.
    pub remaining_tokens: u32,

    /// Number of calls used so far (including this one).
    pub calls_used: u32,

    /// Number of tokens used so far (not yet including this reservation).
    pub tokens_used_before_reservation: u32,
}

// ---------------------------------------------------------------------------
// Commit Reservation DTOs
// ---------------------------------------------------------------------------

/// Input for committing a reservation with actual token consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitReservationInput {
    /// The execution ID this reservation belongs to.
    pub execution_id: uuid::Uuid,

    /// The call identifier from the reservation.
    pub call_id: u32,

    /// Number of tokens that were reserved during `reserve()`.
    ///
    /// Used to compute the delta between estimated and actual consumption.
    /// If actual < reserved, the difference is refunded. If actual > reserved,
    /// the extra is deducted.
    pub reserved_tokens: u32,

    /// Actual number of tokens consumed by the LLM call.
    ///
    /// This is the total tokens (input + output) reported by the LLM provider.
    pub actual_tokens: u32,
}

/// Output from committing a reservation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitReservationOutput {
    /// Updated reservation state after commit.
    pub reservation: LlmBudgetReservationState,

    /// Updated remaining call capacity.
    pub remaining_calls: u32,

    /// Updated remaining token capacity.
    pub remaining_tokens: u32,

    /// Tokens used total after this commit.
    pub total_tokens_used: u32,

    /// Calls used total after this commit.
    pub total_calls_used: u32,

    /// Whether any budget thresholds were crossed by this commit.
    pub warnings_triggered: Vec<BudgetWarningInfo>,
}

// ---------------------------------------------------------------------------
// Get Budget Status DTOs
// ---------------------------------------------------------------------------

/// Input for querying budget status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBudgetStatusInput {
    /// The execution ID to query.
    pub execution_id: uuid::Uuid,
}

/// Output from querying budget status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBudgetStatusOutput {
    /// Call limit configured for this budget.
    pub max_calls: u32,

    /// Token limit configured for this budget.
    pub max_tokens: u32,

    /// Number of calls used.
    pub calls_used: u32,

    /// Number of tokens used.
    pub tokens_used: u32,

    /// Remaining call capacity.
    pub remaining_calls: u32,

    /// Remaining token capacity.
    pub remaining_tokens: u32,

    /// Call usage as a fraction of the limit (0.0–1.0).
    pub call_usage_ratio: f64,

    /// Token usage as a fraction of the limit (0.0–1.0).
    pub token_usage_ratio: f64,

    /// Any active warnings.
    pub active_warnings: Vec<BudgetWarningInfo>,

    /// Human-readable label for this budget.
    pub label: String,
}

// ---------------------------------------------------------------------------
// Shared DTOs
// ---------------------------------------------------------------------------

/// Information about a budget warning.
///
/// Emitted when a resource usage crosses its soft threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetWarningInfo {
    /// Which resource triggered the warning: "calls" or "tokens".
    pub resource: String,

    /// Current usage of the resource.
    pub used: u32,

    /// Maximum allowed.
    pub max: u32,

    /// The threshold value that was crossed.
    pub threshold: u32,

    /// Usage as a fraction of the limit (0.0–1.0).
    pub usage_ratio: f64,

    /// Whether this warning also represents a hard limit hit.
    pub is_exhausted: bool,
}
