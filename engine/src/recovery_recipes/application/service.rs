//! Service interfaces (use cases) for the Recovery Recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#service
//! Implements: Contract Freeze — RecoveryService trait
//! Issue: #438 (recovery-recipes epic)
//!
//! These traits define the application-level operations that can be performed
//! for recovery recipe execution. All methods are async and return domain
//! error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::recovery_recipes::domain::{RecoveryError, RecoveryRecipe};

use super::dto::{
    AttemptRecoveryInput, AttemptRecoveryOutput, CanAttemptInput, CanAttemptOutput,
    RecipeForInput, RecipeForOutput, ValidateRecipeInput, ValidateRecipeOutput,
};

/// Application service for managing and executing recovery recipes.
///
/// The `RecoveryService` is the primary entry point for the recovery-recipes
/// module. It handles:
/// - Looking up recipes for failure scenarios
/// - Checking whether recovery can be attempted (attempt tracking)
/// - Executing recovery steps
/// - Validating recipe configurations
#[async_trait]
pub trait RecoveryService: Send + Sync {
    /// Attempt recovery for a given failure scenario.
    ///
    /// Executes the steps in the recipe sequentially. Returns the result
    /// of the recovery attempt. Each step is executed in order until one
    /// succeeds or all are exhausted.
    ///
    /// # Errors
    /// - `RecoveryError::NoRecipe` if no recipe is available for the scenario
    /// - `RecoveryError::MaxAttemptsReached` if attempts are exhausted
    /// - `RecoveryError::StepFailed` if a recovery step fails
    /// - `RecoveryError::Aborted` if recovery was cancelled
    async fn attempt_recovery(
        &self,
        input: AttemptRecoveryInput,
    ) -> Result<AttemptRecoveryOutput, RecoveryError>;

    /// Look up the recovery recipe for a given failure scenario.
    ///
    /// Checks custom overrides first, then falls back to the default
    /// built-in catalog. Returns `None` if no recipe is configured for
    /// the scenario.
    async fn recipe_for(
        &self,
        input: RecipeForInput,
    ) -> Result<RecipeForOutput, RecoveryError>;

    /// Check whether recovery can be attempted for a scenario.
    ///
    /// Considers the recipe's `max_attempts` and the current attempt
    /// count (tracked in `RecoveryContext`).
    async fn can_attempt(
        &self,
        input: CanAttemptInput,
    ) -> Result<CanAttemptOutput, RecoveryError>;

    /// Validate a recipe configuration.
    ///
    /// Checks:
    /// - Steps list is non-empty
    /// - `max_attempts` >= 1
    /// - Step parameters are valid (e.g., timeout > 0)
    /// - If `require_safe_steps`, all steps must be `is_safe() == true`
    async fn validate_recipe(
        &self,
        input: ValidateRecipeInput,
    ) -> Result<ValidateRecipeOutput, RecoveryError>;

    /// Get the default recipe catalog.
    ///
    /// Returns the canonical set of built-in recipes. This is a convenience
    /// method that delegates to `RecoveryRecipe::default_catalog()`.
    fn default_catalog(&self) -> Vec<RecoveryRecipe>;

    /// Register a custom recipe override.
    ///
    /// Custom recipes take precedence over the default catalog.
    /// Returns the previous recipe for the same scenario, if any.
    async fn register_recipe(
        &self,
        recipe: RecoveryRecipe,
    ) -> Result<Option<RecoveryRecipe>, RecoveryError>;
}
