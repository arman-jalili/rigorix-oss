//! Factory interfaces for constructing Budget Tracking domain objects.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: Contract Freeze — LlmBudgetFactory trait
//! Issue: #68
//!
//! Factories encapsulate the construction of `LlmBudget` instances,
//! allowing implementations to apply presets, inject dependencies
//! (e.g., CancellationToken), and validate configuration without
//! exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured `LlmBudgetService`
//! - Presets replicate enforcement mode budgets
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::budget_tracking::domain::LlmBudgetError;

/// Factory for constructing `LlmBudgetService` instances.
///
/// Handles building budgets from preset modes, from raw configuration,
/// or from `EnforcementConfig` values. Applies default limits when
/// values are not explicitly provided.
#[async_trait]
pub trait LlmBudgetFactory: Send + Sync {
    /// Create a budget service with default mode settings.
    ///
    /// Default mode: 5 calls, 10K tokens.
    async fn create_default(&self) -> Result<Box<dyn super::service::LlmBudgetService>, LlmBudgetError>;

    /// Create a budget service with advanced mode settings.
    ///
    /// Advanced mode: 20 calls, 100K tokens.
    async fn create_advanced(&self) -> Result<Box<dyn super::service::LlmBudgetService>, LlmBudgetError>;

    /// Create a budget service with aggressive mode settings.
    ///
    /// Aggressive mode: 50 calls, 500K tokens.
    async fn create_aggressive(&self) -> Result<Box<dyn super::service::LlmBudgetService>, LlmBudgetError>;

    /// Create a budget service with custom limits.
    ///
    /// Allows arbitrary call and token caps for fine-grained control.
    async fn create_custom(
        &self,
        max_calls: u32,
        max_tokens: u32,
        label: String,
    ) -> Result<Box<dyn super::service::LlmBudgetService>, LlmBudgetError>;

    /// Create a budget service from an `EnforcementConfig` budget entry.
    ///
    /// Reads the "tokens" budget and "tool_calls" budget from the
    /// enforcement configuration to set call and token limits.
    async fn create_from_enforcement_config(
        &self,
        max_tool_calls: u64,
        max_tokens: u64,
    ) -> Result<Box<dyn super::service::LlmBudgetService>, LlmBudgetError>;
}
