//! Data Transfer Objects for the Recovery Recipes module.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md
//! Implements: Contract Freeze — DTO schemas for recovery recipes
//! Issue: #438 (recovery-recipes epic)
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::recovery_recipes::domain::{
    FailureScenario, RecoveryRecipe, RecoveryResult, RecoveryStep,
};

// ---------------------------------------------------------------------------
// Attempt Recovery DTOs
// ---------------------------------------------------------------------------

/// Input for attempting recovery on a failure scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptRecoveryInput {
    /// The failure scenario to attempt recovery for.
    pub scenario: FailureScenario,

    /// The recipe to use for recovery steps.
    pub recipe: RecoveryRecipe,

    /// Current attempt number (1-based). Used for attempt tracking.
    /// Must be >= 1 and <= recipe.max_attempts.
    pub attempt_number: u32,

    /// Optional context about the original failure (error message, etc.)
    /// This is passed through to recovery steps that need it (e.g., ExpandContext).
    pub original_error: Option<String>,

    /// Optional execution context for tool use (only if the recovery step
    /// requires executing external commands).
    pub execution_id: Option<String>,
}

/// Output from attempting recovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptRecoveryOutput {
    /// The result of the recovery attempt.
    pub result: RecoveryResult,

    /// The step that was being executed when recovery completed/failed.
    pub last_step: Option<RecoveryStep>,

    /// Whether this was the final attempt before escalation.
    pub is_final_attempt: bool,

    /// Human-readable summary of the recovery outcome.
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Recipe Lookup DTOs
// ---------------------------------------------------------------------------

/// Input for looking up a recipe for a failure scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeForInput {
    /// The failure scenario to find a recipe for.
    pub scenario: FailureScenario,

    /// Optional custom recipe overrides. If provided, these are checked
    /// before the default catalog.
    pub custom_recipes: Option<Vec<RecoveryRecipe>>,
}

/// Output from looking up a recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeForOutput {
    /// The recipe for the scenario, if found.
    pub recipe: Option<RecoveryRecipe>,

    /// Whether the recipe came from the default catalog or a custom override.
    pub source: RecipeSource,
}

/// Source of a recipe lookup result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecipeSource {
    /// Recipe found in the default built-in catalog.
    DefaultCatalog,
    /// Recipe found in a custom override configuration.
    CustomOverride,
    /// No recipe found for the scenario.
    NotFound,
}

// ---------------------------------------------------------------------------
// Validate Recipe DTOs
// ---------------------------------------------------------------------------

/// Input for validating a recipe configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateRecipeInput {
    /// The recipe to validate.
    pub recipe: RecoveryRecipe,

    /// Whether to require that steps have safe=true for auto-execution.
    pub require_safe_steps: bool,
}

/// Output from validating a recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateRecipeOutput {
    /// Whether the recipe configuration is valid.
    pub valid: bool,
    /// List of validation errors (empty if valid).
    pub errors: Vec<String>,
    /// List of warnings (non-blocking issues).
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Attempt Check DTOs
// ---------------------------------------------------------------------------

/// Input for checking whether recovery can be attempted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanAttemptInput {
    /// The failure scenario to check.
    pub scenario: FailureScenario,
    /// The recipe that would be used.
    pub recipe: RecoveryRecipe,
}

/// Output from checking whether recovery can be attempted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanAttemptOutput {
    /// Whether recovery can be attempted.
    pub can_attempt: bool,
    /// Why recovery can or cannot be attempted.
    pub reason: String,
    /// Remaining attempts.
    pub remaining_attempts: u32,
}
