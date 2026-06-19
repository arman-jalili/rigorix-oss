//! RecoveryRecipe — binds a scenario to its recovery steps and escalation policy.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#recoveryrecipe
//! Implements: Contract Freeze — RecoveryRecipe struct
//! Issue: #438 (recovery-recipes epic)
//!
//! # Contract (Frozen)
//! - A recipe is an immutable value object once constructed
//! - `scenario` identifies which failure scenario this recipe handles
//! - `steps` is an ordered sequence of recovery actions to attempt
//! - `max_attempts` limits how many automatic recovery cycles to attempt
//! - `escalation_policy` defines behavior when attempts are exhausted
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq` for testability
//! - Serialization support for configuration files and API responses

use serde::{Deserialize, Serialize};

use super::escalation::EscalationPolicy;
use super::scenario::FailureScenario;
use super::step::RecoveryStep;

/// Binds a `FailureScenario` to its ordered recovery steps and escalation policy.
///
/// A `RecoveryRecipe` defines what to do when a known failure scenario occurs:
/// which recovery steps to execute, in what order, how many times to retry,
/// and what to do when automatic recovery is exhausted.
///
/// # Built-in Recipe Catalog
///
/// | Scenario | Steps | Max Attempts | Escalation |
/// |----------|-------|-------------|------------|
/// | CompileError | CleanBuild → ExpandContext | 1 | AlertHuman |
/// | TestFailure | ExpandContext | 1 | AlertHuman |
/// | ToolConnectionError | RetryConnection(30s) → RestartService | 2 | AlertHuman |
/// | ProviderFailure | RetryConnection(10s) → RetryConnection(60s) | 2 | AlertHuman |
/// | PartialInitialization | RestartWorker | 1 | AlertHuman |
/// | AuthorizationError | AcceptTrust | 1 | AlertHuman |
/// | StaleBranch | RebaseBranch | 1 | LogAndContinue |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecipe {
    /// The failure scenario this recipe handles.
    pub scenario: FailureScenario,

    /// Ordered sequence of recovery steps to attempt.
    /// Steps are executed in order until one succeeds or all are exhausted.
    pub steps: Vec<RecoveryStep>,

    /// Maximum automatic recovery attempts for this scenario.
    /// Must be >= 1. Enforced by the recovery engine: `max_attempts = 1`
    /// means "try once, then escalate".
    pub max_attempts: u32,

    /// What to do when automatic recovery attempts are exhausted.
    pub escalation_policy: EscalationPolicy,
}

impl RecoveryRecipe {
    /// Create a new `RecoveryRecipe` with validation.
    ///
    /// Returns `Err(RecoveryError::InvalidConfiguration)` if:
    /// - `steps` is empty
    /// - `max_attempts` is 0
    pub fn new(
        scenario: FailureScenario,
        steps: Vec<RecoveryStep>,
        max_attempts: u32,
        escalation_policy: EscalationPolicy,
    ) -> Result<Self, super::error::RecoveryError> {
        if steps.is_empty() {
            return Err(super::error::RecoveryError::InvalidConfiguration {
                detail: format!(
                    "Recipe for {:?} must have at least one recovery step",
                    scenario
                ),
            });
        }
        if max_attempts == 0 {
            return Err(super::error::RecoveryError::InvalidConfiguration {
                detail: format!(
                    "Recipe for {:?} must have max_attempts >= 1",
                    scenario
                ),
            });
        }
        Ok(Self {
            scenario,
            steps,
            max_attempts,
            escalation_policy,
        })
    }

    /// Returns `true` if this recipe has remaining steps after `taken` steps.
    pub fn has_remaining_steps(&self, taken: usize) -> bool {
        taken < self.steps.len()
    }

    /// Returns the number of steps in this recipe.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Build the default recipe catalog.
    ///
    /// Returns the canonical set of built-in recipes as defined in the
    /// architecture module. Implementations may override individual recipes
    /// via the `RecoveryRecipeRepository`.
    pub fn default_catalog() -> Vec<Self> {
        use super::escalation::EscalationPolicy::*;
        use super::scenario::FailureScenario::*;
        use super::step::RecoveryStep::*;

        vec![
            // CompileError: try clean build, then expand context
            Self {
                scenario: CompileError,
                steps: vec![CleanBuild, ExpandContext],
                max_attempts: 1,
                escalation_policy: AlertHuman,
            },
            // TestFailure: expand context and retry
            Self {
                scenario: TestFailure,
                steps: vec![ExpandContext],
                max_attempts: 1,
                escalation_policy: AlertHuman,
            },
            // ToolConnectionError: retry with timeout, then restart service
            Self {
                scenario: ToolConnectionError,
                steps: vec![
                    RetryConnection { timeout_ms: 30000 },
                    RestartService {
                        name: "external".to_string(),
                    },
                ],
                max_attempts: 2,
                escalation_policy: AlertHuman,
            },
            // ProviderFailure: retry with escalating timeout
            Self {
                scenario: ProviderFailure,
                steps: vec![
                    RetryConnection { timeout_ms: 10000 },
                    RetryConnection { timeout_ms: 60000 },
                ],
                max_attempts: 2,
                escalation_policy: AlertHuman,
            },
            // PartialInitialization: restart the worker
            Self {
                scenario: PartialInitialization,
                steps: vec![RestartWorker],
                max_attempts: 1,
                escalation_policy: AlertHuman,
            },
            // AuthorizationError: auto-accept trust
            Self {
                scenario: AuthorizationError,
                steps: vec![AcceptTrust],
                max_attempts: 1,
                escalation_policy: AlertHuman,
            },
            // StaleBranch: rebase onto main
            Self {
                scenario: StaleBranch,
                steps: vec![RebaseBranch],
                max_attempts: 1,
                escalation_policy: LogAndContinue,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_recipe() {
        let recipe = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![RecoveryStep::CleanBuild],
            1,
            EscalationPolicy::AlertHuman,
        );
        assert!(recipe.is_ok());
        let recipe = recipe.unwrap();
        assert_eq!(recipe.scenario, FailureScenario::CompileError);
        assert_eq!(recipe.steps.len(), 1);
        assert_eq!(recipe.max_attempts, 1);
        assert_eq!(recipe.escalation_policy, EscalationPolicy::AlertHuman);
    }

    #[test]
    fn test_empty_steps_rejected() {
        let recipe = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![],
            1,
            EscalationPolicy::AlertHuman,
        );
        assert!(recipe.is_err());
    }

    #[test]
    fn test_zero_max_attempts_rejected() {
        let recipe = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![RecoveryStep::CleanBuild],
            0,
            EscalationPolicy::AlertHuman,
        );
        assert!(recipe.is_err());
    }

    #[test]
    fn test_has_remaining_steps() {
        let recipe = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![RecoveryStep::CleanBuild, RecoveryStep::ExpandContext],
            1,
            EscalationPolicy::AlertHuman,
        )
        .unwrap();
        assert!(recipe.has_remaining_steps(0));
        assert!(recipe.has_remaining_steps(1));
        assert!(!recipe.has_remaining_steps(2));
    }

    #[test]
    fn test_default_catalog_contains_all_scenarios() {
        let catalog = RecoveryRecipe::default_catalog();
        let scenarios: Vec<FailureScenario> = catalog.iter().map(|r| r.scenario).collect();

        assert!(scenarios.contains(&FailureScenario::CompileError));
        assert!(scenarios.contains(&FailureScenario::TestFailure));
        assert!(scenarios.contains(&FailureScenario::ToolConnectionError));
        assert!(scenarios.contains(&FailureScenario::ProviderFailure));
        assert!(scenarios.contains(&FailureScenario::PartialInitialization));
        assert!(scenarios.contains(&FailureScenario::AuthorizationError));
        assert!(scenarios.contains(&FailureScenario::StaleBranch));
        assert_eq!(catalog.len(), 7);
    }

    #[test]
    fn test_default_catalog_steps_not_empty() {
        for recipe in RecoveryRecipe::default_catalog() {
            assert!(
                !recipe.steps.is_empty(),
                "Recipe for {:?} has no steps",
                recipe.scenario
            );
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let recipe = RecoveryRecipe {
            scenario: FailureScenario::CompileError,
            steps: vec![RecoveryStep::CleanBuild, RecoveryStep::ExpandContext],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::AlertHuman,
        };
        let json = serde_json::to_string(&recipe).unwrap();
        let deserialized: RecoveryRecipe = serde_json::from_str(&json).unwrap();
        assert_eq!(recipe, deserialized);
    }
}
