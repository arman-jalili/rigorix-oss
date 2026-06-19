//! HTTP API contracts for Quality Gates endpoints.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #449 (quality-gates epic)
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

use crate::quality_gates::application::dto::{
    ClassifyTestScopeInput, ClassifyTestScopeOutput, EvaluateGateInput, EvaluateGateOutput,
};

use crate::quality_gates::domain::{GreenContract, QualityGateOutcome, QualityLevel};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All quality gates endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/quality";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/quality/evaluate
// ---------------------------------------------------------------------------

/// POST /api/v1/quality/evaluate
///
/// Evaluate a quality gate against an observed test scope.
///
/// **Request:** `EvaluateGateRequest`
/// **Response:** `200 OK` with `EvaluateGateResponse`
pub const EVALUATE_PATH: &str = "/api/v1/quality/evaluate";
pub const EVALUATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/quality/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateGateRequest {
    /// The green contract specifying the required quality level.
    pub contract: GreenContract,
    /// The observed quality level from test execution.
    pub observed_level: Option<QualityLevel>,
    /// Optional task ID for traceability.
    pub task_id: Option<String>,
}

impl From<EvaluateGateRequest> for EvaluateGateInput {
    fn from(req: EvaluateGateRequest) -> Self {
        Self {
            contract: req.contract,
            observed_level: req.observed_level,
            task_id: req.task_id,
        }
    }
}

/// Response body for POST /api/v1/quality/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateGateResponse {
    pub success: bool,
    pub outcome: QualityGateOutcome,
    pub summary: String,
    pub task_id: Option<String>,
}

impl From<EvaluateGateOutput> for EvaluateGateResponse {
    fn from(output: EvaluateGateOutput) -> Self {
        Self {
            success: output.outcome.is_satisfied(),
            outcome: output.outcome,
            summary: output.summary,
            task_id: output.task_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/quality/classify
// ---------------------------------------------------------------------------

/// POST /api/v1/quality/classify
///
/// Classify a test scope into a quality level.
///
/// **Request:** `ClassifyScopeRequest`
/// **Response:** `200 OK` with `ClassifyScopeResponse`
pub const CLASSIFY_PATH: &str = "/api/v1/quality/classify";
pub const CLASSIFY_METHOD: &str = "POST";

/// Request body for POST /api/v1/quality/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyScopeRequest {
    /// Whether targeted tests were run.
    pub targeted_tests_run: bool,
    /// Whether package-level tests were run.
    pub package_tests_run: bool,
    /// Whether workspace-level tests were run.
    pub workspace_tests_run: bool,
    /// Whether lint (clippy) passed.
    pub lint_passed: bool,
    /// Whether format check passed.
    pub format_passed: bool,
    /// Whether security audit passed.
    pub audit_passed: bool,
}

impl From<ClassifyScopeRequest> for ClassifyTestScopeInput {
    fn from(req: ClassifyScopeRequest) -> Self {
        Self {
            targeted_tests_run: req.targeted_tests_run,
            package_tests_run: req.package_tests_run,
            workspace_tests_run: req.workspace_tests_run,
            lint_passed: req.lint_passed,
            format_passed: req.format_passed,
            audit_passed: req.audit_passed,
        }
    }
}

/// Response body for POST /api/v1/quality/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyScopeResponse {
    pub level: QualityLevel,
    pub explanation: String,
}

impl From<ClassifyTestScopeOutput> for ClassifyScopeResponse {
    fn from(output: ClassifyTestScopeOutput) -> Self {
        Self {
            level: output.level,
            explanation: output.explanation,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/quality/contract
// ---------------------------------------------------------------------------

/// GET /api/v1/quality/contract
///
/// Get the quality contract for a template or task.
///
/// **Query Params:** `?template=refactor&task=task-42`
/// **Response:** `200 OK` with `ContractResponse`
pub const CONTRACT_PATH: &str = "/api/v1/quality/contract";
pub const CONTRACT_METHOD: &str = "GET";

/// Response body for GET /api/v1/quality/contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractResponse {
    pub required_level: QualityLevel,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Quality Gates API endpoints.
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

/// Standardized error codes for Quality Gates API.
pub mod error_codes {
    /// Classification failed — unable to determine quality level.
    pub const CLASSIFICATION_FAILED: &str = "CLASSIFICATION_FAILED";
    /// No contract defined for the given task or template.
    pub const MISSING_CONTRACT: &str = "MISSING_CONTRACT";
    /// Invalid quality level value.
    pub const INVALID_QUALITY_LEVEL: &str = "INVALID_QUALITY_LEVEL";
    /// Invalid input provided.
    pub const INVALID_INPUT: &str = "INVALID_INPUT";
    /// Configuration error.
    pub const CONFIGURATION_ERROR: &str = "CONFIGURATION_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Quality Gates errors.
pub mod status_codes {
    pub const CLASSIFICATION_FAILED: u16 = 422;
    pub const MISSING_CONTRACT: u16 = 404;
    pub const INVALID_QUALITY_LEVEL: u16 = 422;
    pub const INVALID_INPUT: u16 = 400;
    pub const CONFIGURATION_ERROR: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
