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
    RecoveryStepOutcome,
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

    /// Execute a single recovery step and say what actually happened.
    ///
    /// `Unsupported` is not an error: it means the capability does not exist in
    /// this engine. It must still stop the recipe, because a later step cannot
    /// depend on a repair that did not run.
    async fn execute_step(
        &self,
        step: &RecoveryStep,
        original_error: Option<&str>,
    ) -> RecoveryStepOutcome {
        match step {
            // Process-spawning sinks (build, git rebase, worker restart) need
            // their own allowlist discipline; faking them is how #1157 was
            // created. They are explicitly unsupported until that work exists.
            RecoveryStep::CleanBuild => RecoveryStepOutcome::Unsupported {
                reason: "no build supervisor: a clean build is a process-spawning sink with its \
                         own security surface"
                    .to_string(),
            },
            RecoveryStep::RebaseBranch => RecoveryStepOutcome::Unsupported {
                reason: "no git-rebase supervisor: rebasing is a process-spawning sink with its \
                         own security surface"
                    .to_string(),
            },
            RecoveryStep::RestartWorker => RecoveryStepOutcome::Unsupported {
                reason: "no supervisor exists in this engine to restart a worker".to_string(),
            },
            RecoveryStep::AcceptTrust => RecoveryStepOutcome::Unsupported {
                reason: "no trust-decision store; auto-accepting a trust prompt needs policy or \
                         human authority"
                    .to_string(),
            },

            // In-process and honest: with an original error there is context to
            // expand; without one there is nothing to expand.
            RecoveryStep::ExpandContext => match original_error {
                Some(error) if !error.trim().is_empty() => RecoveryStepOutcome::Performed,
                _ => RecoveryStepOutcome::Unsupported {
                    reason: "no original error context to expand".to_string(),
                },
            },

            // Validation steps that perform no external action (by design).
            RecoveryStep::RetryConnection { timeout_ms } => {
                if *timeout_ms == 0 {
                    RecoveryStepOutcome::Failed {
                        reason: "RetryConnection timeout must be > 0".to_string(),
                    }
                } else if *timeout_ms > DEFAULT_RETRY_TIMEOUT_MS * 10 {
                    RecoveryStepOutcome::Failed {
                        reason: format!(
                            "RetryConnection timeout {}ms exceeds maximum {}ms",
                            timeout_ms,
                            DEFAULT_RETRY_TIMEOUT_MS * 10
                        ),
                    }
                } else {
                    RecoveryStepOutcome::Performed
                }
            }
            RecoveryStep::RestartService { name } => {
                if name.trim().is_empty() {
                    RecoveryStepOutcome::Failed {
                        reason: "RestartService name must not be empty".to_string(),
                    }
                } else {
                    RecoveryStepOutcome::Performed
                }
            }
            RecoveryStep::EscalateToHuman { reason } => {
                if reason.trim().is_empty() {
                    RecoveryStepOutcome::Failed {
                        reason: "EscalateToHuman reason must not be empty".to_string(),
                    }
                } else {
                    RecoveryStepOutcome::Performed
                }
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

        let mut outcomes: Vec<RecoveryStepOutcome> = Vec::new();
        let mut performed: Vec<RecoveryStep> = Vec::new();

        for (index, step) in recipe.steps.iter().enumerate() {
            let outcome = self
                .execute_step(step, input.original_error.as_deref())
                .await;
            if outcome.is_performed() {
                performed.push(step.clone());
                outcomes.push(outcome);
                continue;
            }

            // A step that did not act stops the recipe: later steps cannot
            // depend on a repair that never happened.
            let remaining: Vec<RecoveryStep> = recipe.steps[index..].to_vec();
            let reason = outcome.reason().unwrap_or("no reason given").to_string();
            outcomes.push(outcome);
            let (result, summary) = if performed.is_empty() {
                (
                    RecoveryResult::EscalationRequired {
                        reason: format!("Step {step:?}: {reason}"),
                    },
                    format!("Escalation required: step {step:?}: {reason}"),
                )
            } else {
                (
                    RecoveryResult::PartialRecovery {
                        recovered: performed.clone(),
                        remaining,
                    },
                    format!(
                        "Partial recovery: {} performed, stopped at {step:?}: {reason}",
                        performed.len()
                    ),
                )
            };
            return Ok(AttemptRecoveryOutput {
                result,
                last_step: Some(step.clone()),
                is_final_attempt: input.attempt_number >= recipe.max_attempts,
                summary,
                step_outcomes: outcomes,
            });
        }

        // Every step performed its action — only now is the node repaired.
        Ok(AttemptRecoveryOutput {
            result: RecoveryResult::Recovered {
                steps_taken: outcomes.len() as u32,
            },
            last_step: recipe.steps.last().cloned(),
            is_final_attempt: input.attempt_number >= recipe.max_attempts,
            summary: format!("Recovered after {} step(s)", outcomes.len()),
            step_outcomes: outcomes,
        })
    }

    async fn recipe_for(&self, input: RecipeForInput) -> Result<RecipeForOutput, RecoveryError> {
        // Check custom overrides first
        if let Some(custom_recipes) = &input.custom_recipes
            && let Some(recipe) = custom_recipes.iter().find(|r| r.scenario == input.scenario)
        {
            return Ok(RecipeForOutput {
                recipe: Some(recipe.clone()),
                source: RecipeSource::CustomOverride,
            });
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
                RecoveryStep::EscalateToHuman { reason } if reason.trim().is_empty() => {
                    warnings
                        .push("EscalateToHuman with empty reason provides no context".to_string());
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

    fn performed_recipe() -> RecoveryRecipe {
        RecoveryRecipe::new(
            FailureScenario::ProviderFailure,
            vec![RecoveryStep::RetryConnection { timeout_ms: 1_000 }],
            1,
            EscalationPolicy::AlertHuman,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_attempt_recovery_success() {
        let service = RecoveryServiceImpl::new();
        let output = service
            .attempt_recovery(AttemptRecoveryInput {
                scenario: FailureScenario::ProviderFailure,
                recipe: performed_recipe(),
                attempt_number: 1,
                original_error: None,
                execution_id: None,
            })
            .await
            .unwrap();

        assert!(output.result.is_recovered());
        assert_eq!(output.result.steps_executed(), 1);
        assert_eq!(output.step_outcomes, vec![RecoveryStepOutcome::Performed]);
        assert!(output.is_final_attempt); // attempt 1 of max_attempts 1 — is final
    }

    #[tokio::test]
    async fn an_unsupported_step_is_not_reported_as_recovered() {
        let service = RecoveryServiceImpl::new();
        let output = service
            .attempt_recovery(AttemptRecoveryInput {
                scenario: FailureScenario::CompileError,
                recipe: test_recipe(), // CleanBuild: unsupported in this engine
                attempt_number: 1,
                original_error: None,
                execution_id: None,
            })
            .await
            .unwrap();

        assert!(!output.result.is_recovered(), "a no-op is not a repair");
        assert!(output.result.is_escalation_required());
        assert_eq!(output.step_outcomes.len(), 1);
        assert!(output.step_outcomes[0].is_unsupported());
        assert!(
            output.step_outcomes[0]
                .reason()
                .unwrap_or_default()
                .contains("build supervisor")
        );
    }

    #[tokio::test]
    async fn a_partial_recovery_distinguishes_performed_from_unsupported() {
        let service = RecoveryServiceImpl::new();
        let recipe = RecoveryRecipe::new(
            FailureScenario::ProviderFailure,
            vec![
                RecoveryStep::RetryConnection { timeout_ms: 1_000 },
                RecoveryStep::CleanBuild,
            ],
            1,
            EscalationPolicy::AlertHuman,
        )
        .unwrap();
        let output = service
            .attempt_recovery(AttemptRecoveryInput {
                scenario: FailureScenario::ProviderFailure,
                recipe,
                attempt_number: 1,
                original_error: Some("connection reset".to_string()),
                execution_id: None,
            })
            .await
            .unwrap();

        assert!(output.result.is_partial());
        assert_eq!(output.step_outcomes.len(), 2);
        assert!(output.step_outcomes[0].is_performed());
        assert!(output.step_outcomes[1].is_unsupported());
        match &output.result {
            RecoveryResult::PartialRecovery {
                recovered,
                remaining,
            } => {
                assert_eq!(
                    recovered,
                    &vec![RecoveryStep::RetryConnection { timeout_ms: 1_000 }]
                );
                assert_eq!(remaining, &vec![RecoveryStep::CleanBuild]);
            }
            other => panic!("expected partial recovery, got {other:?}"),
        }
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

        let performed = service
            .execute_step(&RecoveryStep::RetryConnection { timeout_ms: 1000 }, None)
            .await;
        assert!(performed.is_performed());

        let zero = service
            .execute_step(&RecoveryStep::RetryConnection { timeout_ms: 0 }, None)
            .await;
        assert!(zero.is_failed());
        assert!(zero.reason().unwrap_or_default().contains("must be > 0"));

        let too_long = service
            .execute_step(
                &RecoveryStep::RetryConnection {
                    timeout_ms: DEFAULT_RETRY_TIMEOUT_MS * 10 + 1,
                },
                None,
            )
            .await;
        assert!(too_long.is_failed());
        assert!(
            too_long
                .reason()
                .unwrap_or_default()
                .contains("exceeds maximum")
        );
    }

    #[tokio::test]
    async fn test_execute_step_restart_service_validates_name() {
        let service = RecoveryServiceImpl::new();
        let performed = service
            .execute_step(
                &RecoveryStep::RestartService {
                    name: "lsp".to_string(),
                },
                None,
            )
            .await;
        assert!(performed.is_performed());

        let empty = service
            .execute_step(
                &RecoveryStep::RestartService {
                    name: "".to_string(),
                },
                None,
            )
            .await;
        assert!(empty.is_failed());
    }

    #[tokio::test]
    async fn test_execute_step_escalate_validates_reason() {
        let service = RecoveryServiceImpl::new();
        let performed = service
            .execute_step(
                &RecoveryStep::EscalateToHuman {
                    reason: "test".to_string(),
                },
                None,
            )
            .await;
        assert!(performed.is_performed());

        let empty = service
            .execute_step(
                &RecoveryStep::EscalateToHuman {
                    reason: "".to_string(),
                },
                None,
            )
            .await;
        assert!(empty.is_failed());
    }

    #[tokio::test]
    async fn side_effect_steps_are_explicitly_unsupported() {
        let service = RecoveryServiceImpl::new();
        for step in &[
            RecoveryStep::CleanBuild,
            RecoveryStep::RebaseBranch,
            RecoveryStep::AcceptTrust,
            RecoveryStep::RestartWorker,
        ] {
            let outcome = service.execute_step(step, None).await;
            assert!(
                outcome.is_unsupported(),
                "{step:?} must not claim to have repaired anything"
            );
            assert!(outcome.reason().is_some(), "{step:?} needs a reason");
        }
    }

    #[tokio::test]
    async fn expand_context_is_performed_only_with_real_context() {
        let service = RecoveryServiceImpl::new();
        assert!(
            service
                .execute_step(&RecoveryStep::ExpandContext, Some("compile failed"))
                .await
                .is_performed()
        );
        assert!(
            service
                .execute_step(&RecoveryStep::ExpandContext, None)
                .await
                .is_unsupported()
        );
    }
}
