//! Repository interfaces for the Budget Tracking bounded context.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: Contract Freeze — LlmBudgetRepository trait
//! Issue: #68
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use in-memory, database, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::budget_tracking::domain::{LlmBudget, LlmBudgetError};

use crate::budget_tracking::application::dto::{CommitReservationInput, ReserveBudgetInput};

/// Repository for persisting and retrieving LLM budget state.
///
/// Implementations may use:
/// - In-memory store (default for single execution)
/// - Shared state for cross-execution budget tracking
/// - Persistent storage for audit/replay
///
/// # Security
/// - Implementations MUST NOT log budget token values to avoid leaking
///   prompt sizes or usage patterns
#[async_trait]
pub trait LlmBudgetRepository: Send + Sync {
    /// Save a budget snapshot for later retrieval.
    ///
    /// Persists the full budget state including usage counters.
    /// Overwrites any previous state for the same budget label.
    async fn save(&self, budget: &LlmBudget) -> Result<(), LlmBudgetError>;

    /// Load a budget by its label.
    ///
    /// Returns `None` if no budget exists for this label.
    async fn find_by_label(&self, label: &str) -> Result<Option<LlmBudget>, LlmBudgetError>;

    /// Record a reservation in persistent storage.
    ///
    /// Stores reservation metadata for audit and replay.
    async fn record_reservation(
        &self,
        execution_id: &uuid::Uuid,
        input: &ReserveBudgetInput,
    ) -> Result<(), LlmBudgetError>;

    /// Record a reservation commit in persistent storage.
    ///
    /// Updates the stored reservation with actual token consumption.
    async fn record_commit(
        &self,
        execution_id: &uuid::Uuid,
        input: &CommitReservationInput,
    ) -> Result<(), LlmBudgetError>;

    /// List all budget snapshots.
    ///
    /// Returns all persisted budget states, newest first.
    async fn list(&self) -> Result<Vec<LlmBudget>, LlmBudgetError>;

    /// Delete a budget by label.
    ///
    /// No-op if the budget doesn't exist.
    async fn delete(&self, label: &str) -> Result<(), LlmBudgetError>;
}
