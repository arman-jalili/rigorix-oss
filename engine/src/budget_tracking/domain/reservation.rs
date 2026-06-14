//! LlmBudgetReservation domain entity.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md#reservation
//! Implements: Contract Freeze — LlmBudgetReservation RAII guard state
//! Issue: #68
//!
//! Represents the state of an RAII budget reservation. Every successful
//! `reserve()` call on an `LlmBudget` creates a reservation that, when
//! dropped without an explicit `commit()`, automatically rolls back the
//! reserved call and token counts.
//!
//! # Contract (Frozen)
//! - `LlmBudgetReservationState` is the observable state snapshot of a reservation
//! - The actual RAII guard is a service trait method return type
//! - Commit updates the actual tokens consumed (may differ from reserved)
//! - Auto-rollback on Drop ensures no budget leakage on panic

use serde::{Deserialize, Serialize};

/// Observable state of an LLM budget reservation at a point in time.
///
/// This is a snapshot value object, not the RAII guard itself.
/// The RAII guard (`LlmBudgetReservation` trait in `application/service.rs`)
/// holds a mutable reference to the budget and auto-rollbacks on Drop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBudgetReservationState {
    /// Monotonically increasing call identifier for this reservation.
    pub call_id: u32,

    /// Number of tokens reserved for this call (estimated before execution).
    pub reserved_tokens: u32,

    /// Number of tokens actually consumed once committed (None if not yet committed).
    pub actual_tokens: Option<u32>,

    /// Whether the reservation has been committed.
    pub committed: bool,

    /// Whether the reservation was rolled back on Drop.
    pub rolled_back: bool,
}

impl LlmBudgetReservationState {
    /// Create a new reservation state snapshot.
    pub fn new(call_id: u32, reserved_tokens: u32) -> Self {
        Self {
            call_id,
            reserved_tokens,
            actual_tokens: None,
            committed: false,
            rolled_back: false,
        }
    }

    /// Mark the reservation as committed with actual token consumption.
    pub fn with_commit(mut self, actual_tokens: u32) -> Self {
        self.actual_tokens = Some(actual_tokens);
        self.committed = true;
        self
    }

    /// Mark the reservation as rolled back.
    pub fn with_rollback(mut self) -> Self {
        self.rolled_back = true;
        self.committed = false;
        self
    }
}
