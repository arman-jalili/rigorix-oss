//! Service interfaces (use cases) for the Budget Tracking bounded context.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: Contract Freeze — LlmBudgetService, LlmBudgetReservation traits
//! Issue: #68
//!
//! These traits define the application-level operations for LLM budget
//! reservation, commitment, rollback, and status queries. All methods are
//! async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::budget_tracking::domain::LlmBudgetError;

use super::dto::{
    CommitReservationInput, CommitReservationOutput, GetBudgetStatusInput,
    GetBudgetStatusOutput, ReserveBudgetInput, ReserveBudgetOutput,
};

/// Central budget service for LLM call budget tracking.
///
/// Manages the lifecycle of LLM budget reservations: reserve → use → commit
/// (or auto-rollback on Drop). Acts as the single point of contact for all
/// budget-related operations in an execution.
///
/// # RAII Reservation Lifecycle
///
/// 1. `reserve(estimated_tokens)` — Checks capacity and creates a reservation
/// 2. LLM API call executes with the reserved capacity
/// 3a. `commit(actual_tokens)` — Records actual consumption
/// 3b. Drop (panic/error) — Auto-rollback releases reserved capacity
///
/// # Cancellation Integration
///
/// When budget is exhausted, the service returns `LlmBudgetError::MaxCallsExceeded`
/// or `LlmBudgetError::MaxTokensExceeded`. The caller is responsible for coordinating
/// cancellation via the Cancellation module.
#[async_trait]
pub trait LlmBudgetService: Send + Sync {
    /// Reserve budget for an LLM call.
    ///
    /// Checks both call count and token capacity. On success, returns a
    /// `LlmBudgetReservation` guard that auto-rollbacks on Drop.
    ///
    /// # Errors
    ///
    /// Returns `MaxCallsExceeded` if all calls have been used.
    /// Returns `MaxTokensExceeded` if reserving `estimated_tokens` would
    /// exceed the token limit.
    async fn reserve(&self, input: ReserveBudgetInput) -> Result<ReserveBudgetOutput, LlmBudgetError>;

    /// Commit a reservation with actual token consumption.
    ///
    /// Updates the budget with the actual tokens used by the LLM call.
    /// The actual tokens may differ from the estimated tokens reserved.
    ///
    /// Returns the updated budget status after commit.
    async fn commit(
        &self,
        input: CommitReservationInput,
    ) -> Result<CommitReservationOutput, LlmBudgetError>;

    /// Get the current budget status (usage vs. limits).
    ///
    /// Returns a snapshot of calls used, tokens used, remaining capacity,
    /// and usage ratios. Does not modify any state.
    async fn get_status(
        &self,
        input: GetBudgetStatusInput,
    ) -> Result<GetBudgetStatusOutput, LlmBudgetError>;

    /// Check whether the budget has any remaining capacity.
    ///
    /// Returns `true` if both calls and tokens have remaining capacity.
    fn has_capacity(&self) -> bool;

    /// Get a summary of all active budget warnings.
    ///
    /// Returns warnings for any resource that has crossed its soft threshold
    /// but not yet reached the hard limit.
    fn active_warnings(&self) -> Vec<super::dto::BudgetWarningInfo>;
}

/// RAII guard that holds budget capacity for a single LLM call.
///
/// Created by `LlmBudgetService::reserve()`. When dropped without an explicit
/// `commit()`, the reserved capacity is automatically released (rollback).
///
/// # Contract (Frozen)
///
/// - `commit()` must be called exactly once if the LLM call succeeds
/// - On Drop without commit, the reservation is rolled back atomically
/// - The guard is `Send` but not `Clone`
///
/// # Example Lifecycle
///
/// ```ignore
/// let guard = budget.reserve(100).await?;
/// let response = llm_call(prompt).await;
/// match response {
///     Ok(output) => { guard.commit(output.tokens_used).await?; }
///     Err(_) => { /* guard drops, auto-rollback */ }
/// }
/// ```
#[async_trait]
pub trait LlmBudgetReservation: Send {
    /// Commit this reservation with the actual tokens consumed.
    ///
    /// This is the only way to finalize a reservation without triggering
    /// an automatic rollback on Drop.
    async fn commit(&mut self, actual_tokens: u32) -> Result<(), LlmBudgetError>;

    /// Get the call identifier for this reservation.
    fn call_id(&self) -> u32;

    /// Get the number of tokens reserved (estimated).
    fn reserved_tokens(&self) -> u32;

    /// Whether this reservation has been committed.
    fn is_committed(&self) -> bool;

    /// Whether this reservation has been rolled back.
    fn is_rolled_back(&self) -> bool;
}
