//! HTTP API contracts for Recovery Recipes endpoints.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #438 (recovery-recipes epic)
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::recovery_recipes::application::dto::{
    AttemptRecoveryInput, AttemptRecoveryOutput, CanAttemptInput, CanAttemptOutput, RecipeForInput,
    RecipeForOutput,
};

use crate::recovery_recipes::domain::{
    FailureScenario, RecoveryRecipe, RecoveryResult, RecoveryStep,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All recovery recipes endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/recovery";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/recovery/attempt
// ---------------------------------------------------------------------------

/// POST /api/v1/recovery/attempt
///
/// Attempt recovery for a given failure scenario using its configured recipe.
///
/// **Request:** `AttemptRecoveryRequest`
/// **Response:** `200 OK` with `AttemptRecoveryResponse`
pub const ATTEMPT_PATH: &str = "/api/v1/recovery/attempt";
pub const ATTEMPT_METHOD: &str = "POST";

/// Request body for POST /api/v1/recovery/attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptRecoveryRequest {
    /// The failure scenario to recover from.
    pub scenario: FailureScenario,
    /// The recipe to use for recovery steps.
    pub recipe: RecoveryRecipe,
    /// Current attempt number (1-based).
    pub attempt_number: u32,
    /// Optional original error message.
    pub original_error: Option<String>,
    /// Optional execution ID for audit logging.
    pub execution_id: Option<String>,
}

impl From<AttemptRecoveryRequest> for AttemptRecoveryInput {
    fn from(req: AttemptRecoveryRequest) -> Self {
        Self {
            scenario: req.scenario,
            recipe: req.recipe,
            attempt_number: req.attempt_number,
            original_error: req.original_error,
            execution_id: req.execution_id,
        }
    }
}

/// Response body for POST /api/v1/recovery/attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptRecoveryResponse {
    pub success: bool,
    pub result: RecoveryResult,
    pub last_step: Option<RecoveryStep>,
    pub is_final_attempt: bool,
    pub summary: String,
}

impl From<AttemptRecoveryOutput> for AttemptRecoveryResponse {
    fn from(output: AttemptRecoveryOutput) -> Self {
        Self {
            success: output.result.is_recovered() || output.result.is_partial(),
            result: output.result,
            last_step: output.last_step,
            is_final_attempt: output.is_final_attempt,
            summary: output.summary,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/recovery/recipe
// ---------------------------------------------------------------------------

/// POST /api/v1/recovery/recipe
///
/// Look up the recovery recipe for a given failure scenario.
///
/// **Request:** `RecipeForRequest`
/// **Response:** `200 OK` with `RecipeForResponse`
pub const RECIPE_PATH: &str = "/api/v1/recovery/recipe";
pub const RECIPE_METHOD: &str = "POST";

/// Request body for POST /api/v1/recovery/recipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeForRequest {
    /// The failure scenario to find a recipe for.
    pub scenario: FailureScenario,
}

impl From<RecipeForRequest> for RecipeForInput {
    fn from(req: RecipeForRequest) -> Self {
        Self {
            scenario: req.scenario,
            custom_recipes: None,
        }
    }
}

/// Response body for POST /api/v1/recovery/recipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeForResponse {
    pub success: bool,
    pub recipe: Option<RecoveryRecipe>,
    pub source: String,
}

impl From<RecipeForOutput> for RecipeForResponse {
    fn from(output: RecipeForOutput) -> Self {
        Self {
            success: output.recipe.is_some(),
            recipe: output.recipe,
            source: format!("{:?}", output.source),
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/recovery/can-attempt
// ---------------------------------------------------------------------------

/// POST /api/v1/recovery/can-attempt
///
/// Check whether recovery can be attempted for a failure scenario.
///
/// **Request:** `CanAttemptRequest`
/// **Response:** `200 OK` with `CanAttemptResponse`
pub const CAN_ATTEMPT_PATH: &str = "/api/v1/recovery/can-attempt";
pub const CAN_ATTEMPT_METHOD: &str = "POST";

/// Request body for POST /api/v1/recovery/can-attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanAttemptRequest {
    /// The failure scenario to check.
    pub scenario: FailureScenario,
    /// The recipe that would be used.
    pub recipe: RecoveryRecipe,
}

impl From<CanAttemptRequest> for CanAttemptInput {
    fn from(req: CanAttemptRequest) -> Self {
        Self {
            scenario: req.scenario,
            recipe: req.recipe,
        }
    }
}

/// Response body for POST /api/v1/recovery/can-attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanAttemptResponse {
    pub can_attempt: bool,
    pub reason: String,
    pub remaining_attempts: u32,
}

impl From<CanAttemptOutput> for CanAttemptResponse {
    fn from(output: CanAttemptOutput) -> Self {
        Self {
            can_attempt: output.can_attempt,
            reason: output.reason,
            remaining_attempts: output.remaining_attempts,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/recovery/recipes
// ---------------------------------------------------------------------------

/// GET /api/v1/recovery/recipes
///
/// Retrieve the default recovery recipe catalog.
///
/// **Request:** None
/// **Response:** `200 OK` with `CatalogResponse`
pub const CATALOG_PATH: &str = "/api/v1/recovery/recipes";
pub const CATALOG_METHOD: &str = "GET";

/// Response body for GET /api/v1/recovery/recipes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogResponse {
    pub recipes: Vec<RecoveryRecipe>,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Recovery Recipes API endpoints.
///
/// All 4xx/5xx responses use this format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// HTTP status code.
    pub status: u16,
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Detailed error context (optional, may include field-level errors).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available).
    pub request_id: Option<String>,
}

/// Standardized error codes for Recovery Recipes API.
pub mod error_codes {
    /// No recipe configured for the given scenario.
    pub const NO_RECIPE: &str = "NO_RECIPE";
    /// Maximum recovery attempts reached.
    pub const MAX_ATTEMPTS_REACHED: &str = "MAX_ATTEMPTS_REACHED";
    /// A recovery step failed during execution.
    pub const STEP_FAILED: &str = "STEP_FAILED";
    /// Recovery was aborted by a cancellation signal.
    pub const ABORTED: &str = "ABORTED";
    /// Invalid recipe configuration.
    pub const INVALID_CONFIGURATION: &str = "INVALID_CONFIGURATION";
    /// A required dependency is unavailable.
    pub const DEPENDENCY_UNAVAILABLE: &str = "DEPENDENCY_UNAVAILABLE";
    /// Invalid input provided (empty steps, zero max_attempts, etc.).
    pub const INVALID_INPUT: &str = "INVALID_INPUT";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Recovery Recipes errors.
pub mod status_codes {
    pub const NO_RECIPE: u16 = 404;
    pub const MAX_ATTEMPTS_REACHED: u16 = 429;
    pub const STEP_FAILED: u16 = 500;
    pub const ABORTED: u16 = 499;
    pub const INVALID_CONFIGURATION: u16 = 422;
    pub const DEPENDENCY_UNAVAILABLE: u16 = 503;
    pub const INVALID_INPUT: u16 = 400;
    pub const INTERNAL_ERROR: u16 = 500;
}
