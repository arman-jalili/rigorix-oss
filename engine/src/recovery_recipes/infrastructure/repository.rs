//! Repository interfaces for the Recovery Recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#repo
//! Implements: Contract Freeze — RecoveryRecipeRepository trait
//! Issue: #438 (recovery-recipes epic)
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use filesystem, environment, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::recovery_recipes::domain::{FailureScenario, RecoveryError, RecoveryRecipe};

/// Repository for storing and retrieving recovery recipes.
///
/// Implementations can store recipes in memory, on disk, or in a database.
/// The repository is the single source of truth for recipe configuration,
/// including custom overrides that take precedence over built-in defaults.
///
/// # Security
/// - Recipe steps are predefined — no arbitrary command injection possible
/// - `max_attempts` is validated to be >= 1
/// - Steps must be non-empty
#[async_trait]
pub trait RecoveryRecipeRepository: Send + Sync {
    /// Retrieve the recipe for a given failure scenario.
    ///
    /// Checks custom recipes first, then falls back to the default catalog.
    /// Returns `None` if no recipe is configured for the scenario.
    async fn recipe_for(
        &self,
        scenario: FailureScenario,
    ) -> Result<Option<RecoveryRecipe>, RecoveryError>;

    /// Store a custom recipe override.
    ///
    /// Custom recipes take precedence over the default catalog.
    /// Returns the previous recipe for the same scenario, if any.
    async fn store_recipe(
        &self,
        recipe: RecoveryRecipe,
    ) -> Result<Option<RecoveryRecipe>, RecoveryError>;

    /// Retrieve all stored custom recipes.
    ///
    /// Returns a map of scenario → recipe for registered overrides only
    /// (does not include built-in defaults).
    async fn all_recipes(&self) -> Result<Vec<RecoveryRecipe>, RecoveryError>;

    /// Remove a custom recipe override for the given scenario.
    ///
    /// Returns `true` if a recipe existed and was removed, `false` otherwise.
    async fn remove_recipe(
        &self,
        scenario: FailureScenario,
    ) -> Result<bool, RecoveryError>;

    /// Clear all custom recipe overrides, resetting to the default catalog.
    async fn clear_recipes(&self) -> Result<(), RecoveryError>;
}
