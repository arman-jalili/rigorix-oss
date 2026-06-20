//! Concrete implementation of `RecoveryService`.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#service
//! Implements: RecoveryService — recovery dispatch and step execution
//! Issue: #440, #441, #442, #443, #444
//!
//! Dispatches recovery attempts, looks up recipes, tracks attempts,
//! and executes recovery steps.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::recovery_recipes::domain::{
    FailureScenario, RecoveryError, RecoveryRecipe, RecoveryResult, RecoveryStep,
};

use super::dto::{
    AttemptRecoveryInput, AttemptRecoveryOutput, CanAttemptInput, CanAttemptOutput, RecipeForInput,
    RecipeForOutput, RecipeSource, ValidateRecipeInput, ValidateRecipeOutput,
};
use super::service::RecoveryService;

/// Default maximum retry timeout in milliseconds for `RetryConnection` steps.
const DEFAULT_RETRY_TIMEOUT_MS: u64 = 30000;

/// Concrete implementation of `RecoveryService`.
///
/// Uses the default recipe catalog as the source of truth, with optional
/// custom recipe overrides stored in a `HashMap`.
pub struct RecoveryServiceImpl {
    /// Custom recipe overrides (scenario → recipe).
    custom_recipes: HashMap<FailureScenario, RecoveryRecipe>,
}

impl RecoveryServiceImpl {
    /// Create a new `RecoveryServiceImpl` with no custom recipes.
    pub fn new() -> Self {
        Self {
            custom_recipes: HashMap::new(),
        }
    }

    /// Create a new `RecoveryServiceImpl` with the given custom recipes.
    pub fn with_custom_recipes(recipes: Vec<RecoveryRecipe>) -> Self {
        let mut map = HashMap::new();
        for recipe in recipes {
            map.insert(recipe.scenario, recipe);
        }
        Self {
            custom_recipes: map,
        }
    }

    /// Find a recipe by scenario, checking custom recipes first.
    fn find_recipe(&self, scenario: FailureScenario) -> Option<RecoveryRecipe> {
        self.custom_recipes.get(&scenario).cloned().or_else(|| {
            RecoveryRecipe::default_catalog()
                .into_iter()
                .find(|r| r.scenario == scenario)
        })
    }

    /// Execute a single recovery step.
    /// In a production implementation, this would actually execute the step
    /// (run clean build, retry connection, etc.). For now, returns success
    /// for steps that don't require external execution, and simulates basic steps.
    async fn execute_step(&self, step: &RecoveryStep) -> Result<(), String> {
        match step {
            // Safe steps — simulated as successful
            RecoveryStep::CleanBuild
            | RecoveryStep::ExpandContext
            | RecoveryStep::RebaseBranch
            | RecoveryStep::AcceptTrust
            | RecoveryStep::RestartWorker => Ok(()),

            // Connection retry — validates timeout > 0
            RecoveryStep::RetryConnection { timeout_ms } => {
                if *timeout_ms == 0 {
                    return Err("RetryConnection timeout must be > 0".to_string());
                }
                if *timeout_ms > DEFAULT_RETRY_TIMEOUT_MS * 10 {
                    return Err(format!(
                        "RetryConnection timeout {}ms exceeds maximum {}ms",
                        timeout_ms,
                        DEFAULT_RETRY_TIMEOUT_MS * 10
                    ));
                }
                Ok(())
            }

            // Restart service — validates name is non-empty
            RecoveryStep::RestartService { name } => {
                if name.trim().is_empty() {
                    return Err("RestartService name must not be empty".to_string());
                }
                Ok(())
            }

            // Escalate — requires a reason
            RecoveryStep::EscalateToHuman { reason } => {
                if reason.trim().is_empty() {
                    return Err("EscalateToHuman reason must not be empty".to_string());
                }
                Ok(())
            }
        }
    }
}

impl Default for RecoveryServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RecoveryService for RecoveryServiceImpl {
    async fn attempt_recovery(
        &self,
        input: AttemptRecoveryInput,
    ) -> Result<AttemptRecoveryOutput, RecoveryError> {
        let recipe = &input.recipe;

        if input.attempt_number > recipe.max_attempts {
            return Err(RecoveryError::MaxAttemptsReached(input.scenario));
        }

        let mut steps_taken = 0u32;
        let mut recovered_steps: Vec<RecoveryStep> = Vec::new();
        let mut remaining_steps: Vec<RecoveryStep> = recipe.steps.clone();

        for step in &recipe.steps {
            match self.execute_step(step).await {
                Ok(()) => {
                    steps_taken += 1;
                    recovered_steps.push(step.clone());
                    remaining_steps.remove(0);
                }
                Err(reason) => {
                    // Step failed — record partial recovery or escalation
                    if steps_taken > 0 {
                        return Ok(AttemptRecoveryOutput {
                            result: RecoveryResult::PartialRecovery {
                                recovered: recovered_steps,
                                remaining: remaining_steps,
                            },
                            last_step: Some(step.clone()),
                            is_final_attempt: input.attempt_number >= recipe.max_attempts,
                            summary: format!(
                                "Partial recovery: {} succeeded, failed at {:?}: {}",
                                steps_taken, step, reason
                            ),
                        });
                    }

                    return Ok(AttemptRecoveryOutput {
                        result: RecoveryResult::EscalationRequired {
                            reason: format!("Step {:?} failed: {}", step, reason),
                        },
                        last_step: Some(step.clone()),
                        is_final_attempt: input.attempt_number >= recipe.max_attempts,
                        summary: format!("Escalation required: step {:?} failed: {}", step, reason),
                    });
                }
            }
        }

        // All steps completed successfully
        Ok(AttemptRecoveryOutput {
            result: RecoveryResult::Recovered { steps_taken },
            last_step: recipe.steps.last().cloned(),
            is_final_attempt: input.attempt_number >= recipe.max_attempts,
            summary: format!("Recovered after {} step(s)", steps_taken),
        })
    }

    async fn recipe_for(&self, input: RecipeForInput) -> Result<RecipeForOutput, RecoveryError> {
        // Check custom overrides first
        if let Some(custom_recipes) = &input.custom_recipes {
            if let Some(recipe) = custom_recipes.iter().find(|r| r.scenario == input.scenario) {
                return Ok(RecipeForOutput {
                    recipe: Some(recipe.clone()),
                    source: RecipeSource::CustomOverride,
                });
            }
        }

        // Check internal custom recipes
        if let Some(recipe) = self.custom_recipes.get(&input.scenario) {
            return Ok(RecipeForOutput {
                recipe: Some(recipe.clone()),
                source: RecipeSource::CustomOverride,
            });
        }

        // Fall back to default catalog
        match self.find_recipe(input.scenario) {
            Some(recipe) => Ok(RecipeForOutput {
                recipe: Some(recipe),
                source: RecipeSource::DefaultCatalog,
            }),
            None => Ok(RecipeForOutput {
                recipe: None,
                source: RecipeSource::NotFound,
            }),
        }
    }

    async fn can_attempt(&self, input: CanAttemptInput) -> Result<CanAttemptOutput, RecoveryError> {
        let max = input.recipe.max_attempts;
        // In a real implementation, this would check against RecoveryContext.
        // For now, we always allow the first attempt.
        Ok(CanAttemptOutput {
            can_attempt: max > 0,
            reason: if max > 0 {
                format!("Recipe allows up to {} attempt(s)", max)
            } else {
                "Recipe has max_attempts = 0".to_string()
            },
            remaining_attempts: max,
        })
    }

    async fn validate_recipe(
        &self,
        input: ValidateRecipeInput,
    ) -> Result<ValidateRecipeOutput, RecoveryError> {
        let mut errors: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let recipe = &input.recipe;

        if recipe.steps.is_empty() {
            errors.push("Recipe must have at least one recovery step".to_string());
        }

        if recipe.max_attempts == 0 {
            errors.push("max_attempts must be >= 1".to_string());
        }

        if input.require_safe_steps {
            for step in &recipe.steps {
                if !step.is_safe() {
                    warnings.push(format!(
                        "Step {:?} is not marked as safe for auto-execution",
                        step
                    ));
                }
            }
        }

        // Validate step parameters
        for step in &recipe.steps {
            match step {
                RecoveryStep::RetryConnection { timeout_ms } => {
                    if *timeout_ms == 0 {
                        warnings.push("RetryConnection with timeout_ms = 0 will fail".to_string());
                    }
                    if *timeout_ms > DEFAULT_RETRY_TIMEOUT_MS * 10 {
                        warnings.push(format!(
                            "RetryConnection timeout {}ms exceeds recommended maximum",
                            timeout_ms
                        ));
                    }
                }
                RecoveryStep::RestartService { name } => {
                    if name.trim().is_empty() {
                        warnings.push("RestartService with empty name will fail".to_string());
                    }
                }
                RecoveryStep::EscalateToHuman { reason } => {
                    if reason.trim().is_empty() {
                        warnings.push(
                            "EscalateToHuman with empty reason provides no context".to_string(),
                        );
                    }
                }
                _ => {}
            }
        }

        Ok(ValidateRecipeOutput {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    fn default_catalog(&self) -> Vec<RecoveryRecipe> {
        RecoveryRecipe::default_catalog()
    }

    async fn register_recipe(
        &self,
        _recipe: RecoveryRecipe,
    ) -> Result<Option<RecoveryRecipe>, RecoveryError> {
        // Note: In a real implementation with interior mutability (Mutex/RwLock),
        // this would actually mutate self.custom_recipes. For the contract freeze
        // implementation, this is a simplified version.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery_recipes::domain::{EscalationPolicy, FailureScenario, RecoveryStep};

    fn test_recipe() -> RecoveryRecipe {
        RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![RecoveryStep::CleanBuild],
            1,
            EscalationPolicy::AlertHuman,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_attempt_recovery_success() {
        let service = RecoveryServiceImpl::new();
        let recipe = test_recipe();
        let output = service
            .attempt_recovery(AttemptRecoveryInput {
                scenario: FailureScenario::CompileError,
                recipe,
                attempt_number: 1,
                original_error: None,
                execution_id: None,
            })
            .await
            .unwrap();

        assert!(output.result.is_recovered());
        assert_eq!(output.result.steps_executed(), 1);
        assert!(output.is_final_attempt); // attempt 1 of max_attempts 1 — is final
    }

    #[tokio::test]
    async fn test_attempt_recovery_exceeds_max_attempts() {
        let service = RecoveryServiceImpl::new();
        let recipe = test_recipe();
        let result = service
            .attempt_recovery(AttemptRecoveryInput {
                scenario: FailureScenario::CompileError,
                recipe,
                attempt_number: 2,
                original_error: None,
                execution_id: None,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RecoveryError::MaxAttemptsReached(_)
        ));
    }

    #[tokio::test]
    async fn test_recipe_for_default_catalog() {
        let service = RecoveryServiceImpl::new();
        let output = service
            .recipe_for(RecipeForInput {
                scenario: FailureScenario::CompileError,
                custom_recipes: None,
            })
            .await
            .unwrap();

        assert!(output.recipe.is_some());
        assert_eq!(output.source, RecipeSource::DefaultCatalog);
        assert_eq!(
            output.recipe.unwrap().scenario,
            FailureScenario::CompileError
        );
    }

    #[tokio::test]
    async fn test_recipe_for_custom_override() {
        let custom_recipe = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![RecoveryStep::ExpandContext],
            2,
            EscalationPolicy::Abort,
        )
        .unwrap();

        let service = RecoveryServiceImpl::with_custom_recipes(vec![custom_recipe.clone()]);
        let output = service
            .recipe_for(RecipeForInput {
                scenario: FailureScenario::CompileError,
                custom_recipes: None,
            })
            .await
            .unwrap();

        assert!(output.recipe.is_some());
        assert_eq!(output.source, RecipeSource::CustomOverride);
        let recipe = output.recipe.unwrap();
        assert_eq!(recipe.max_attempts, 2);
        assert_eq!(recipe.escalation_policy, EscalationPolicy::Abort);
    }

    #[tokio::test]
    async fn test_recipe_for_not_found() {
        let service = RecoveryServiceImpl::new();
        let output = service
            .recipe_for(RecipeForInput {
                scenario: FailureScenario::CompileError,
                custom_recipes: Some(vec![]),
            })
            .await
            .unwrap();

        // Should fall through to default catalog
        assert!(output.source != RecipeSource::NotFound);
    }

    #[tokio::test]
    async fn test_can_attempt() {
        let service = RecoveryServiceImpl::new();
        let recipe = test_recipe();
        let output = service
            .can_attempt(CanAttemptInput {
                scenario: FailureScenario::CompileError,
                recipe,
            })
            .await
            .unwrap();

        assert!(output.can_attempt);
        assert_eq!(output.remaining_attempts, 1);
    }

    #[tokio::test]
    async fn test_validate_recipe_valid() {
        let service = RecoveryServiceImpl::new();
        let recipe = test_recipe();
        let output = service
            .validate_recipe(ValidateRecipeInput {
                recipe,
                require_safe_steps: false,
            })
            .await
            .unwrap();

        assert!(output.valid);
        assert!(output.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_recipe_empty_steps() {
        let service = RecoveryServiceImpl::new();
        let recipe = RecoveryRecipe {
            scenario: FailureScenario::CompileError,
            steps: vec![],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::AlertHuman,
        };
        let output = service
            .validate_recipe(ValidateRecipeInput {
                recipe,
                require_safe_steps: false,
            })
            .await
            .unwrap();

        assert!(!output.valid);
        assert!(output.errors.iter().any(|e| e.contains("at least one")));
    }

    #[tokio::test]
    async fn test_validate_recipe_zero_max_attempts() {
        let service = RecoveryServiceImpl::new();
        let recipe = RecoveryRecipe {
            scenario: FailureScenario::CompileError,
            steps: vec![RecoveryStep::CleanBuild],
            max_attempts: 0,
            escalation_policy: EscalationPolicy::AlertHuman,
        };
        let output = service
            .validate_recipe(ValidateRecipeInput {
                recipe,
                require_safe_steps: false,
            })
            .await
            .unwrap();

        assert!(!output.valid);
    }

    #[tokio::test]
    async fn test_default_catalog() {
        let service = RecoveryServiceImpl::new();
        let catalog = service.default_catalog();
        assert_eq!(catalog.len(), 7);
    }

    #[tokio::test]
    async fn test_execute_step_retry_connection_validates_timeout() {
        let service = RecoveryServiceImpl::new();

        // Valid timeout
        let result = service
            .execute_step(&RecoveryStep::RetryConnection { timeout_ms: 1000 })
            .await;
        assert!(result.is_ok());

        // Zero timeout
        let result = service
            .execute_step(&RecoveryStep::RetryConnection { timeout_ms: 0 })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be > 0"));
    }

    #[tokio::test]
    async fn test_execute_step_restart_service_validates_name() {
        let service = RecoveryServiceImpl::new();
        let result = service
            .execute_step(&RecoveryStep::RestartService {
                name: "lsp".to_string(),
            })
            .await;
        assert!(result.is_ok());

        let result = service
            .execute_step(&RecoveryStep::RestartService {
                name: "".to_string(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_step_escalate_validates_reason() {
        let service = RecoveryServiceImpl::new();
        let result = service
            .execute_step(&RecoveryStep::EscalateToHuman {
                reason: "test".to_string(),
            })
            .await;
        assert!(result.is_ok());

        let result = service
            .execute_step(&RecoveryStep::EscalateToHuman {
                reason: "".to_string(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_safe_steps_succeed() {
        let service = RecoveryServiceImpl::new();
        for step in &[
            RecoveryStep::CleanBuild,
            RecoveryStep::ExpandContext,
            RecoveryStep::RebaseBranch,
            RecoveryStep::AcceptTrust,
            RecoveryStep::RestartWorker,
        ] {
            assert!(
                service.execute_step(step).await.is_ok(),
                "Step {:?} should succeed",
                step
            );
        }
    }
}
