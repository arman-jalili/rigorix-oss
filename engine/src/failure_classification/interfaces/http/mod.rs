//! HTTP API contracts for Failure Classification endpoints.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #33
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

use crate::failure_classification::application::dto::{
    CheckRetryEligibilityInput, CheckRetryEligibilityOutput, ClassifyFailureInput,
    ClassifyFailureOutput, GetRetryStrategyInput, GetRetryStrategyOutput, ValidateConfigInput,
    ValidateConfigOutput,
};

use crate::failure_classification::domain::{FailureType, RetryStrategy};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All failure classification endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/failure";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure/classify
// ---------------------------------------------------------------------------

/// POST /api/v1/failure/classify
///
/// Classify an error message into a `FailureType` and get the recommended
/// `RetryStrategy`.
///
/// **Request:** `ClassifyRequest`
/// **Response:** `200 OK` with `ClassifyResponse`
pub const CLASSIFY_PATH: &str = "/api/v1/failure/classify";
pub const CLASSIFY_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRequest {
    /// The error message to classify.
    pub error_message: String,
    /// Optional context about the operation that failed.
    pub operation_context: Option<String>,
    /// Optional source identifier.
    pub source: Option<String>,
}

impl From<ClassifyRequest> for ClassifyFailureInput {
    fn from(req: ClassifyRequest) -> Self {
        Self {
            error_message: req.error_message,
            operation_context: req.operation_context,
            source: req.source,
        }
    }
}

/// Response body for POST /api/v1/failure/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyResponse {
    pub success: bool,
    pub failure_type: FailureType,
    pub recommended_strategy: RetryStrategy,
    pub is_retryable: bool,
    pub confidence: Option<f64>,
    pub explanation: Option<String>,
}

impl From<ClassifyFailureOutput> for ClassifyResponse {
    fn from(output: ClassifyFailureOutput) -> Self {
        Self {
            success: true,
            failure_type: output.failure_type,
            recommended_strategy: output.recommended_strategy,
            is_retryable: output.is_retryable,
            confidence: output.confidence,
            explanation: output.explanation,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure/strategy
// ---------------------------------------------------------------------------

/// POST /api/v1/failure/strategy
///
/// Get the recommended `RetryStrategy` for a `FailureType`.
///
/// **Request:** `StrategyRequest`
/// **Response:** `200 OK` with `StrategyResponse`
pub const STRATEGY_PATH: &str = "/api/v1/failure/strategy";
pub const STRATEGY_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure/strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRequest {
    /// The failure type to get a strategy for.
    pub failure_type: FailureType,
    /// Optional override strategy.
    pub override_strategy: Option<RetryStrategy>,
}

impl From<StrategyRequest> for GetRetryStrategyInput {
    fn from(req: StrategyRequest) -> Self {
        Self {
            failure_type: req.failure_type,
            override_strategy: req.override_strategy,
        }
    }
}

/// Response body for POST /api/v1/failure/strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyResponse {
    pub success: bool,
    pub strategy: RetryStrategy,
    pub source: String,
    pub description: String,
}

impl From<GetRetryStrategyOutput> for StrategyResponse {
    fn from(output: GetRetryStrategyOutput) -> Self {
        Self {
            success: true,
            strategy: output.strategy,
            source: format!("{:?}", output.source),
            description: output.description,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure/check-eligibility
// ---------------------------------------------------------------------------

/// POST /api/v1/failure/check-eligibility
///
/// Check whether a `FailureType` is eligible for retry.
///
/// **Request:** `CheckEligibilityRequest`
/// **Response:** `200 OK` with `CheckEligibilityResponse`
pub const CHECK_ELIGIBILITY_PATH: &str = "/api/v1/failure/check-eligibility";
pub const CHECK_ELIGIBILITY_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure/check-eligibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckEligibilityRequest {
    /// The failure type to check.
    pub failure_type: FailureType,
    /// Current retry count (for retry limit checks).
    pub current_retry_count: Option<u32>,
    /// Max retry limit.
    pub max_retries: Option<u32>,
}

impl From<CheckEligibilityRequest> for CheckRetryEligibilityInput {
    fn from(req: CheckEligibilityRequest) -> Self {
        Self {
            failure_type: req.failure_type,
            current_retry_count: req.current_retry_count,
            max_retries: req.max_retries,
        }
    }
}

/// Response body for POST /api/v1/failure/check-eligibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckEligibilityResponse {
    pub eligible: bool,
    pub reason: String,
    pub remaining_attempts: Option<u32>,
}

impl From<CheckRetryEligibilityOutput> for CheckEligibilityResponse {
    fn from(output: CheckRetryEligibilityOutput) -> Self {
        Self {
            eligible: output.eligible,
            reason: output.reason,
            remaining_attempts: output.remaining_attempts,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure/validate-config
// ---------------------------------------------------------------------------

/// POST /api/v1/failure/validate-config
///
/// Validate failure classification configuration (custom patterns, mappings).
///
/// **Request:** `ValidateConfigRequest`
/// **Response:** `200 OK` with `ValidateConfigResponse`
pub const VALIDATE_CONFIG_PATH: &str = "/api/v1/failure/validate-config";
pub const VALIDATE_CONFIG_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure/validate-config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigRequest {
    /// Custom pattern-to-FailureType mappings to validate.
    pub custom_patterns: Option<std::collections::HashMap<String, FailureType>>,
    /// Custom FailureType-to-RetryStrategy mappings to validate.
    pub custom_strategy_mappings: Option<std::collections::HashMap<FailureType, RetryStrategy>>,
}

impl From<ValidateConfigRequest> for ValidateConfigInput {
    fn from(req: ValidateConfigRequest) -> Self {
        Self {
            custom_patterns: req.custom_patterns,
            custom_strategy_mappings: req.custom_strategy_mappings,
        }
    }
}

/// Response body for POST /api/v1/failure/validate-config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigResponse {
    pub valid: bool,
    pub errors: Vec<super::super::application::dto::ValidationError>,
    pub warnings: Vec<String>,
}

impl From<ValidateConfigOutput> for ValidateConfigResponse {
    fn from(output: ValidateConfigOutput) -> Self {
        Self {
            valid: output.valid,
            errors: output.errors,
            warnings: output.warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Failure Classification API endpoints.
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

/// Standardized error codes for Failure Classification API.
pub mod error_codes {
    /// Classification failed — no pattern matched.
    pub const CLASSIFICATION_FAILED: &str = "CLASSIFICATION_FAILED";
    /// No strategy defined for the given failure type.
    pub const MISSING_STRATEGY: &str = "MISSING_STRATEGY";
    /// Invalid expansion level for ExpandContext strategy.
    pub const INVALID_EXPANSION_LEVEL: &str = "INVALID_EXPANSION_LEVEL";
    /// Invalid input provided (empty message, etc.).
    pub const INVALID_INPUT: &str = "INVALID_INPUT";
    /// Pattern repository error.
    pub const PATTERN_REPOSITORY_ERROR: &str = "PATTERN_REPOSITORY_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Failure Classification errors.
pub mod status_codes {
    pub const CLASSIFICATION_FAILED: u16 = 422;
    pub const MISSING_STRATEGY: u16 = 500;
    pub const INVALID_EXPANSION_LEVEL: u16 = 422;
    pub const INVALID_INPUT: u16 = 400;
    pub const PATTERN_REPOSITORY_ERROR: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
