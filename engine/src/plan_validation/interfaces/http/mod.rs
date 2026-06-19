//! HTTP API contracts for Plan Validation endpoints.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All plan validation endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/plan-validation";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/plan-validation/validate
// ---------------------------------------------------------------------------

/// POST /api/v1/plan-validation/validate
///
/// Execute the full validation loop for a user intent. Plans, executes,
/// and verifies the template, retrying with augmented context on failure
/// up to the configured max_iterations.
///
/// **Request:** `ValidateRequest`
/// **Response:** `200 OK` with `ValidateResponse` (Validated)
/// **Response:** `200 OK` with `ValidateResponse` (Failed — still 200, outcome in body)
/// **Response:** `200 OK` with `ValidateResponse` (BudgetExhausted)
pub const VALIDATE_PATH: &str = "/api/v1/plan-validation/validate";
pub const VALIDATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/plan-validation/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateRequest {
    /// The user's raw intent text.
    pub intent: String,

    /// Optional execution ID for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<Uuid>,

    /// Maximum validation iterations (default: 3).
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Required quality level (default: "package").
    /// Allowed values: "targeted_tests", "package", "workspace", "merge_ready".
    #[serde(default = "default_quality")]
    pub required_quality: String,

    /// Maximum cumulative LLM tokens (default: 50000).
    #[serde(default = "default_max_tokens")]
    pub max_cumulative_tokens: u64,

    /// Whether to cache validated templates (default: true).
    #[serde(default = "default_true")]
    pub cache_successful_templates: bool,
}

fn default_max_iterations() -> u32 {
    3
}
fn default_quality() -> String {
    "package".to_string()
}
fn default_max_tokens() -> u64 {
    50_000
}
fn default_true() -> bool {
    true
}

/// Response body for POST /api/v1/plan-validation/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateResponse {
    /// The execution ID for correlation.
    pub execution_id: Uuid,

    /// The final outcome: "validated", "failed", or "budget_exhausted".
    pub outcome: String,

    /// The validated template ID (present only if outcome is "validated").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,

    /// Number of iterations executed.
    pub iterations: u32,

    /// Maximum iterations configured.
    pub max_iterations: u32,

    /// Cumulative LLM tokens consumed.
    pub cumulative_tokens: u64,

    /// Total duration in milliseconds.
    pub total_duration_ms: u64,

    /// Per-iteration failure summaries (empty if validated on first attempt).
    #[serde(default)]
    pub iteration_summaries: Vec<IterationSummary>,

    /// ISO 8601 timestamp of completion.
    pub completed_at: DateTime<Utc>,
}

/// Summary of a single validation iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationSummary {
    /// The iteration number (1-indexed).
    pub iteration: u32,

    /// Whether this iteration passed.
    pub passed: bool,

    /// Number of failures in this iteration.
    pub failure_count: u32,

    /// Summary of failure messages.
    #[serde(default)]
    pub failure_messages: Vec<String>,

    /// LLM tokens used in this iteration.
    pub llm_tokens_used: u64,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/plan-validation/reports/{execution_id}
// ---------------------------------------------------------------------------

/// GET /api/v1/plan-validation/reports/{execution_id}
///
/// Retrieve a validation report by execution ID.
///
/// **Response:** `200 OK` with `ReportResponse`
/// **Response:** `404 Not Found` with `ValidationApiError`
pub const REPORT_PATH: &str = "/api/v1/plan-validation/reports";
pub const REPORT_METHOD: &str = "GET";

/// Response body for GET /api/v1/plan-validation/reports/{execution_id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportResponse {
    /// The execution ID.
    pub execution_id: Uuid,

    /// The final outcome.
    pub outcome: String,

    /// Number of iterations.
    pub iterations: u32,

    /// Total duration in milliseconds.
    pub total_duration_ms: u64,

    /// Cumulative LLM tokens consumed.
    pub cumulative_tokens: u64,

    /// Whether the validated template is cached.
    pub template_cached: bool,

    /// ISO 8601 timestamp of the report.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/plan-validation/reports
// ---------------------------------------------------------------------------

/// GET /api/v1/plan-validation/reports?limit=10
///
/// List recent validation reports.
///
/// **Query Parameter:** `limit` (default: 10, max: 100)
/// **Response:** `200 OK` with `ReportsListResponse`
pub const REPORTS_LIST_PATH: &str = "/api/v1/plan-validation/reports";
pub const REPORTS_LIST_METHOD: &str = "GET";

/// Response body for GET /api/v1/plan-validation/reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportsListResponse {
    /// List of recent validation reports (summary only).
    pub reports: Vec<ReportSummary>,

    /// Total number of reports available.
    pub total_count: u64,
}

/// Summary of a validation report for list endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub execution_id: Uuid,
    pub outcome: String,
    pub iterations: u32,
    pub total_duration_ms: u64,
    pub cumulative_tokens: u64,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/plan-validation/retry
// ---------------------------------------------------------------------------

/// POST /api/v1/plan-validation/retry
///
/// Retry only the generative nodes of a failed template with augmented
/// context. Used for manual retry when the caller has additional context
/// or wants to retry a specific template without re-running the full
/// validation loop.
///
/// **Request:** `RetryRequest`
/// **Response:** `200 OK` with `RetryResponse`
pub const RETRY_PATH: &str = "/api/v1/plan-validation/retry";
pub const RETRY_METHOD: &str = "POST";

/// Request body for POST /api/v1/plan-validation/retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryRequest {
    /// The execution ID for correlation.
    pub execution_id: Uuid,

    /// The TOML template content to retry.
    pub template_toml: String,

    /// Failure descriptions to augment the context with.
    #[serde(default)]
    pub failure_descriptions: Vec<String>,

    /// Optional additional instructions for the LLM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_instructions: Option<String>,
}

/// Response body for POST /api/v1/plan-validation/retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryResponse {
    pub success: bool,
    pub execution_id: Uuid,
    pub updated_template_toml: String,
    pub nodes_retried: u32,
}

// ---------------------------------------------------------------------------
// Error Response Format
// ---------------------------------------------------------------------------

/// Unified error response format for all validation endpoints.
///
/// All error responses use HTTP 4xx/5xx with this body shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationApiError {
    /// Machine-readable error code.
    pub code: String,

    /// Human-readable error message.
    pub message: String,

    /// Optional details for debugging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    /// The validation iteration where the error occurred (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,

    /// Request ID for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ValidationApiError {
    /// Create a new API error response.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            iteration: None,
            request_id: None,
        }
    }

    /// Attach optional details.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Attach the iteration where the error occurred.
    pub fn with_iteration(mut self, iteration: u32) -> Self {
        self.iteration = Some(iteration);
        self
    }

    /// Attach a request ID for correlation.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

/// Standard error codes for the validation API.
pub mod error_codes {
    pub const VALIDATION_FAILED: &str = "VALIDATION_FAILED";
    pub const BUDGET_EXHAUSTED: &str = "BUDGET_EXHAUSTED";
    pub const PLANNING_ERROR: &str = "PLANNING_ERROR";
    pub const EXECUTION_ERROR: &str = "EXECUTION_ERROR";
    pub const INVALID_REQUEST: &str = "INVALID_REQUEST";
    pub const REPORT_NOT_FOUND: &str = "REPORT_NOT_FOUND";
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
    pub const CANCELLED: &str = "CANCELLED";
}

/// HTTP status code mapping for ValidationLoopError variants (documentation only).
///
/// | Error Code | HTTP Status | Description |
/// |------------|-------------|-------------|
/// | VALIDATION_FAILED | 422 Unprocessable Entity | All retries exhausted |
/// | BUDGET_EXHAUSTED | 429 Too Many Requests | Cumulative token budget exhausted |
/// | PLANNING_ERROR | 500 Internal Server Error | Planning pipeline failure |
/// | EXECUTION_ERROR | 500 Internal Server Error | Execution engine failure |
/// | INVALID_REQUEST | 400 Bad Request | Malformed request body |
/// | REPORT_NOT_FOUND | 404 Not Found | Report not found |
/// | INTERNAL_ERROR | 500 Internal Server Error | Unexpected error |
/// | CANCELLED | 499 Client Closed Request | User cancelled |
pub const ERROR_STATUS_CODES: &[(u16, &str)] = &[
    (422, "VALIDATION_FAILED"),
    (429, "BUDGET_EXHAUSTED"),
    (500, "PLANNING_ERROR"),
    (500, "EXECUTION_ERROR"),
    (400, "INVALID_REQUEST"),
    (404, "REPORT_NOT_FOUND"),
    (500, "INTERNAL_ERROR"),
    (499, "CANCELLED"),
];
