//! HTTP API contracts for Planning Pipeline endpoints.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
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
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All planning pipeline endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/planning";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/planning/plan
// ---------------------------------------------------------------------------

/// POST /api/v1/planning/plan
///
/// Execute the full 6-phase planning flow and return the PlanningResult
/// (without the TaskGraph).
///
/// **Request:** `PlanRequest`
/// **Response:** `200 OK` with `PlanResponse`
pub const PLAN_PATH: &str = "/api/v1/planning/plan";
pub const PLAN_METHOD: &str = "POST";

/// Request body for POST /api/v1/planning/plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRequest {
    /// The user's raw intent text.
    pub intent: String,

    /// Optional execution ID for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<Uuid>,

    /// Whether to enable template generator fallback.
    #[serde(default = "default_true")]
    pub enable_generator_fallback: bool,

    /// Whether to skip plan validation.
    #[serde(default)]
    pub skip_validation: bool,
}

fn default_true() -> bool {
    true
}

/// Response body for POST /api/v1/planning/plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResponse {
    pub execution_id: Uuid,
    pub template_id: String,
    pub confidence: f64,
    pub parameters: HashMap<String, String>,
    pub planning_hash: String,
    pub required_clarification: bool,
    pub from_generator: bool,
    pub clarification_used: bool,
    pub total_llm_calls: u32,
    pub total_llm_tokens: u32,
    pub planned_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/planning/plan-with-graph
// ---------------------------------------------------------------------------

/// POST /api/v1/planning/plan-with-graph
///
/// Execute the full planning flow and return both the PlanningResult
/// and the generated TaskGraph (as JSON).
///
/// **Request:** `PlanWithGraphRequest`
/// **Response:** `200 OK` with `PlanWithGraphResponse`
pub const PLAN_WITH_GRAPH_PATH: &str = "/api/v1/planning/plan-with-graph";
pub const PLAN_WITH_GRAPH_METHOD: &str = "POST";

/// Request body for POST /api/v1/planning/plan-with-graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanWithGraphRequest {
    /// The user's raw intent text.
    pub intent: String,

    /// Optional execution ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<Uuid>,

    /// Whether to enable generator fallback.
    #[serde(default = "default_true")]
    pub enable_generator_fallback: bool,

    /// Whether to skip validation.
    #[serde(default)]
    pub skip_validation: bool,
}

/// Response body for POST /api/v1/planning/plan-with-graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanWithGraphResponse {
    pub execution_id: Uuid,
    pub template_id: String,
    pub confidence: f64,
    pub parameters: HashMap<String, String>,
    pub planning_hash: String,
    pub graph: serde_json::Value,
    pub node_count: u32,
    pub validation_passed: bool,
    pub validation_warnings: Vec<String>,
    pub from_generator: bool,
    pub clarification_used: bool,
    pub total_llm_calls: u32,
    pub total_llm_tokens: u32,
    pub completed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/planning/classify
// ---------------------------------------------------------------------------

/// POST /api/v1/planning/classify
///
/// Classify user intent against available templates (phase 2 only).
///
/// **Request:** `ClassifyRequest`
/// **Response:** `200 OK` with `ClassifyResponse`
pub const CLASSIFY_PATH: &str = "/api/v1/planning/classify";
pub const CLASSIFY_METHOD: &str = "POST";

/// Request body for POST /api/v1/planning/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRequest {
    /// The user's intent text.
    pub intent: String,
}

/// One classified alternative in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedAlternative {
    pub template_id: String,
    pub confidence: f64,
    pub reasoning: String,
}

/// Response body for POST /api/v1/planning/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyResponse {
    pub alternatives: Vec<ClassifiedAlternative>,
    pub requires_clarification: bool,
    pub needs_generator: bool,
    pub reasoning: String,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/planning/templates
// ---------------------------------------------------------------------------

/// GET /api/v1/planning/templates
///
/// List all available templates with summary metadata.
///
/// **Response:** `200 OK` with `TemplatesResponse`
pub const TEMPLATES_PATH: &str = "/api/v1/planning/templates";
pub const TEMPLATES_METHOD: &str = "GET";

/// Response body for GET /api/v1/planning/templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatesResponse {
    pub templates: Vec<TemplateInfo>,
    pub total_count: u32,
}

/// Template metadata for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parameter_count: u32,
    pub node_count: u32,
    pub category: Option<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/planning/clarify
// ---------------------------------------------------------------------------

/// POST /api/v1/planning/clarify
///
/// Submit a clarification response and re-run classification.
///
/// **Request:** `ClarifyRequest`
/// **Response:** `200 OK` with `ClarifyResponse`
pub const CLARIFY_PATH: &str = "/api/v1/planning/clarify";
pub const CLARIFY_METHOD: &str = "POST";

/// Request body for POST /api/v1/planning/clarify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarifyRequest {
    /// The execution ID.
    pub execution_id: Uuid,

    /// The clarification question that was asked.
    pub question: String,

    /// The user's response.
    pub answer: String,
}

/// Response body for POST /api/v1/planning/clarify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarifyResponse {
    pub execution_id: Uuid,
    pub classification: ClassifyResponse,
    pub requires_further_clarification: bool,
}

// ---------------------------------------------------------------------------
// Error Response Format
// ---------------------------------------------------------------------------

/// Unified error response format for all planning pipeline endpoints.
///
/// All error responses use HTTP 4xx/5xx with this body shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningApiError {
    /// Machine-readable error code.
    pub code: String,

    /// Human-readable error message.
    pub message: String,

    /// Optional details for debugging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    /// The pipeline phase where the error occurred (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,

    /// Request ID for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl PlanningApiError {
    /// Create a new API error response.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            phase: None,
            request_id: None,
        }
    }

    /// Attach optional details.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Attach the pipeline phase where the error occurred.
    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    /// Attach a request ID for correlation.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

/// Standard error codes for the planning pipeline API.
pub mod error_codes {
    pub const BUDGET_EXHAUSTED: &str = "BUDGET_EXHAUSTED";
    pub const NO_MATCHING_TEMPLATE: &str = "NO_MATCHING_TEMPLATE";
    pub const MISSING_PARAMETER: &str = "MISSING_PARAMETER";
    pub const VALIDATION_FAILED: &str = "VALIDATION_FAILED";
    pub const CLASSIFICATION_ERROR: &str = "CLASSIFICATION_ERROR";
    pub const EXTRACTION_ERROR: &str = "EXTRACTION_ERROR";
    pub const GENERATION_ERROR: &str = "GENERATION_ERROR";
    pub const INVALID_REQUEST: &str = "INVALID_REQUEST";
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
    pub const CANCELLED: &str = "CANCELLED";
}

/// HTTP status code mapping for PlanningError variants (documentation only).
///
/// | Error Code | HTTP Status | Description |
/// |------------|-------------|-------------|
/// | BUDGET_EXHAUSTED | 429 Too Many Requests | LLM budget exhausted |
/// | NO_MATCHING_TEMPLATE | 422 Unprocessable Entity | No template matched |
/// | MISSING_PARAMETER | 422 Unprocessable Entity | Required param missing |
/// | VALIDATION_FAILED | 422 Unprocessable Entity | Plan failed validation |
/// | CLASSIFICATION_ERROR | 500 Internal Server Error | LLM call failed |
/// | EXTRACTION_ERROR | 500 Internal Server Error | Extraction failed |
/// | GENERATION_ERROR | 500 Internal Server Error | Graph generation failed |
/// | INVALID_REQUEST | 400 Bad Request | Malformed request |
/// | INTERNAL_ERROR | 500 Internal Server Error | Unexpected error |
/// | CANCELLED | 499 Client Closed Request | User cancelled |
pub const ERROR_STATUS_CODES: &[(u16, &str)] = &[
    (429, "BUDGET_EXHAUSTED"),
    (422, "NO_MATCHING_TEMPLATE"),
    (422, "MISSING_PARAMETER"),
    (422, "VALIDATION_FAILED"),
    (500, "CLASSIFICATION_ERROR"),
    (500, "EXTRACTION_ERROR"),
    (500, "GENERATION_ERROR"),
    (400, "INVALID_REQUEST"),
    (500, "INTERNAL_ERROR"),
    (499, "CANCELLED"),
];

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/planning/generate-template
// ---------------------------------------------------------------------------

/// POST /api/v1/planning/generate-template
///
/// Generate a new template from user intent using the LLM fallback.
/// Builds RepoContext, calls LLM with retry, validates against schema
/// and symbol graph.
///
/// **Request:** `GenerateTemplateRequest`
/// **Response:** `200 OK` with `GenerateTemplateResponse`
/// **Error:** `422 Unprocessable Entity` if generation fails
/// **Error:** `429 Too Many Requests` if budget exhausted
pub const GENERATE_TEMPLATE_PATH: &str = "/api/v1/planning/generate-template";
pub const GENERATE_TEMPLATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/planning/generate-template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateTemplateRequest {
    /// The user's intent text.
    pub intent: String,

    /// Optional execution ID for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<Uuid>,

    /// Root directory to build RepoContext from.
    pub root_dir: String,

    /// Whether to auto-register the generated template.
    #[serde(default)]
    pub auto_register: bool,

    /// Whether to run Phase 3 symbol validation.
    #[serde(default = "default_true")]
    pub validate_symbols: bool,

    /// Maximum LLM retry attempts.
    #[serde(default = "default_max_retries_api")]
    pub max_retries: u8,
}

fn default_max_retries_api() -> u8 {
    3
}

/// Response body for POST /api/v1/planning/generate-template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateTemplateResponse {
    pub success: bool,
    pub execution_id: Uuid,
    pub template_id: String,
    pub template_name: String,
    pub llm_calls_used: u32,
    pub llm_tokens_used: u32,
    pub attempts: u8,
    pub validated: bool,
    pub symbol_validation_passed: bool,
    pub errors: Vec<String>,
    pub toml_content: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/planning/validate-symbols
// ---------------------------------------------------------------------------

/// POST /api/v1/planning/validate-symbols
///
/// Run Phase 3 symbol validation against a provided TOML template.
/// Checks for hallucinated type references and field accesses.
///
/// **Request:** `ValidateSymbolsRequest`
/// **Response:** `200 OK` with `ValidateSymbolsResponse`
pub const VALIDATE_SYMBOLS_PATH: &str = "/api/v1/planning/validate-symbols";
pub const VALIDATE_SYMBOLS_METHOD: &str = "POST";

/// Request body for POST /api/v1/planning/validate-symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSymbolsRequest {
    /// The TOML template content to validate.
    pub toml_content: String,

    /// Whether to flag `any` type usage.
    #[serde(default = "default_true")]
    pub flag_any_type: bool,
}

/// Response body for POST /api/v1/planning/validate-symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSymbolsResponse {
    pub valid: bool,
    pub references_checked: u32,
    pub invalid_references: Vec<InvalidSymbolRefResponse>,
    pub any_type_detected: bool,
}

/// An invalid symbol reference in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidSymbolRefResponse {
    pub symbol: String,
    pub usage: String,
    pub reason: String,
    pub is_any_type: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/planning/build-context
// ---------------------------------------------------------------------------

/// POST /api/v1/planning/build-context
///
/// Build a RepoContext from a working directory (for preview/debugging).
///
/// **Request:** `BuildContextRequest`
/// **Response:** `200 OK` with `BuildContextResponse`
pub const BUILD_CONTEXT_PATH: &str = "/api/v1/planning/build-context";
pub const BUILD_CONTEXT_METHOD: &str = "POST";

/// Request body for POST /api/v1/planning/build-context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextRequest {
    /// Root directory to scan.
    pub root_dir: String,
    /// Whether to include the symbol graph snapshot.
    #[serde(default)]
    pub include_symbol_graph: bool,
}

/// Response body for POST /api/v1/planning/build-context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextResponse {
    pub success: bool,
    pub project_type: String,
    pub files_scanned: u32,
    pub api_entries_found: u32,
    pub dependencies: Vec<String>,
    pub directory_tree: Vec<String>,
}
