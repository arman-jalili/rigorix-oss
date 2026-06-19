//! In-memory implementation of `RecoveryRecipeRepository`.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#repo
//! Implements: RecoveryRecipeRepository — in-memory recipe storage
//! Issue: #442
//!
//! Stores recipes in a `HashMap` with the default catalog as fallback.
//! Thread-safe via `std::sync::RwLock` for interior mutability.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::recovery_recipes::domain::{FailureScenario, RecoveryError, RecoveryRecipe};

use super::repository::RecoveryRecipeRepository;

/// In-memory implementation of `RecoveryRecipeRepository`.
///
/// Stores custom recipe overrides in a `HashMap` protected by `RwLock`.
/// Falls back to the default catalog when no custom recipe exists for
/// a given scenario.
pub struct InMemoryRecipeRepository {
    /// Custom recipe overrides (scenario → recipe).
    recipes: RwLock<HashMap<FailureScenario, RecoveryRecipe>>,
}

impl InMemoryRecipeRepository {
    /// Create a new empty repository (no custom overrides).
    pub fn new() -> Self {
        Self {
            recipes: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new repository pre-populated with custom recipes.
    pub fn with_recipes(recipes: Vec<RecoveryRecipe>) -> Self {
        let map: HashMap<_, _> = recipes.into_iter().map(|r| (r.scenario, r)).collect();
        Self {
            recipes: RwLock::new(map),
        }
    }
}

impl Default for InMemoryRecipeRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RecoveryRecipeRepository for InMemoryRecipeRepository {
    async fn recipe_for(
        &self,
        scenario: FailureScenario,
    ) -> Result<Option<RecoveryRecipe>, RecoveryError> {
        // Check custom overrides first
        if let Some(recipe) = self
            .recipes
            .read()
            .map_err(|e| RecoveryError::DependencyUnavailable {
                dependency: "InMemoryRecipeRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            })?
            .get(&scenario)
        {
            return Ok(Some(recipe.clone()));
        }

        // Fall back to default catalog
        Ok(RecoveryRecipe::default_catalog()
            .into_iter()
            .find(|r| r.scenario == scenario))
    }

    async fn store_recipe(
        &self,
        recipe: RecoveryRecipe,
    ) -> Result<Option<RecoveryRecipe>, RecoveryError> {
        let mut recipes = self.recipes.write().map_err(|e| {
            RecoveryError::DependencyUnavailable {
                dependency: "InMemoryRecipeRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;
        Ok(recipes.insert(recipe.scenario, recipe))
    }

    async fn all_recipes(&self) -> Result<Vec<RecoveryRecipe>, RecoveryError> {
        let custom = self.recipes.read().map_err(|e| {
            RecoveryError::DependencyUnavailable {
                dependency: "InMemoryRecipeRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;

        // Merge custom overrides with defaults
        let mut all = RecoveryRecipe::default_catalog();
        for recipe in custom.values() {
            if let Some(pos) = all.iter().position(|r| r.scenario == recipe.scenario) {
                all[pos] = recipe.clone();
            } else {
                all.push(recipe.clone());
            }
        }
        Ok(all)
    }

    async fn remove_recipe(
        &self,
        scenario: FailureScenario,
    ) -> Result<bool, RecoveryError> {
        let mut recipes = self.recipes.write().map_err(|e| {
            RecoveryError::DependencyUnavailable {
                dependency: "InMemoryRecipeRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;
        Ok(recipes.remove(&scenario).is_some())
    }

    async fn clear_recipes(&self) -> Result<(), RecoveryError> {
        let mut recipes = self.recipes.write().map_err(|e| {
            RecoveryError::DependencyUnavailable {
                dependency: "InMemoryRecipeRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;
        recipes.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery_recipes::domain::{EscalationPolicy, RecoveryStep};

    fn sample_recipe() -> RecoveryRecipe {
        RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![RecoveryStep::CleanBuild],
            1,
            EscalationPolicy::AlertHuman,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_empty_repository_falls_back_to_default() {
        let repo = InMemoryRecipeRepository::new();
        let recipe = repo.recipe_for(FailureScenario::CompileError).await.unwrap();
        assert!(recipe.is_some());
        assert_eq!(
            recipe.unwrap().scenario,
            FailureScenario::CompileError
        );
    }

    #[tokio::test]
    async fn test_store_and_retrieve_custom_recipe() {
        let repo = InMemoryRecipeRepository::new();
        let custom = sample_recipe();
        let previous = repo.store_recipe(custom.clone()).await.unwrap();
        assert!(previous.is_none());

        let retrieved = repo
            .recipe_for(FailureScenario::CompileError)
            .await
            .unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().scenario, FailureScenario::CompileError);
    }

    #[tokio::test]
    async fn test_custom_recipe_overrides_default() {
        let repo = InMemoryRecipeRepository::new();
        let custom = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![RecoveryStep::ExpandContext],
            2,
            EscalationPolicy::Abort,
        )
        .unwrap();
        repo.store_recipe(custom.clone()).await.unwrap();

        let retrieved = repo
            .recipe_for(FailureScenario::CompileError)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.max_attempts, 2);
        assert_eq!(retrieved.escalation_policy, EscalationPolicy::Abort);
        assert_eq!(retrieved.steps, vec![RecoveryStep::ExpandContext]);
    }

    #[tokio::test]
    async fn test_remove_recipe() {
        let repo = InMemoryRecipeRepository::new();
        let custom = sample_recipe();
        repo.store_recipe(custom).await.unwrap();

        let removed = repo.remove_recipe(FailureScenario::CompileError).await.unwrap();
        assert!(removed);
        assert!(!repo.remove_recipe(FailureScenario::CompileError).await.unwrap());
    }

    #[tokio::test]
    async fn test_clear_recipes() {
        let repo = InMemoryRecipeRepository::new();
        repo.store_recipe(sample_recipe()).await.unwrap();
        repo.clear_recipes().await.unwrap();

        let all = repo.all_recipes().await.unwrap();
        // Should only contain defaults
        assert_eq!(all.len(), 7);
    }

    #[tokio::test]
    async fn test_all_recipes_merges_custom_and_default() {
        let repo = InMemoryRecipeRepository::new();
        let custom = RecoveryRecipe::new(
            FailureScenario::StaleBranch,
            vec![RecoveryStep::RebaseBranch, RecoveryStep::ExpandContext],
            2,
            EscalationPolicy::Abort,
        )
        .unwrap();
        repo.store_recipe(custom).await.unwrap();

        let all = repo.all_recipes().await.unwrap();
        assert_eq!(all.len(), 7); // Same count because override replaces default

        let stale = all.iter().find(|r| r.scenario == FailureScenario::StaleBranch).unwrap();
        assert_eq!(stale.max_attempts, 2);
        assert_eq!(stale.escalation_policy, EscalationPolicy::Abort);
    }

    #[tokio::test]
    async fn test_recipe_for_unknown_scenario_returns_none_like() {
        // FailureScenario doesn't have an "unknown" variant, but we can test
        // a scenario that's not in the default catalog... actually all are.
        // This tests that the repository never panics.
        let repo = InMemoryRecipeRepository::new();
        let result = repo.recipe_for(FailureScenario::AuthorizationError).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_recipes_constructor() {
        let custom = sample_recipe();
        let repo = InMemoryRecipeRepository::with_recipes(vec![custom]);
        let retrieved = repo
            .recipe_for(FailureScenario::CompileError)
            .await
            .unwrap();
        assert!(retrieved.is_some());
    }
}
