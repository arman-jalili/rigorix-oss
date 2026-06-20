//! HTTP API contracts for Action Output endpoints.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! Note: In production, the action runs as a GitHub Action, not an HTTP server.
//! These contracts exist for:
//! - Local development & debugging endpoints
//! - Runtime introspection (health checks, status)
//! - Testing via HTTP mocks
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::action_output::domain::{FormattedOutput, OutputLevel, StepSummary, WorkflowAnnotation};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All action output endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/action-output";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-output/format
// ---------------------------------------------------------------------------

/// POST /api/v1/action-output/format
///
/// Format an execution context into a formatted output bundle.
///
/// **Request:** `FormatOutputRequest`
/// **Response:** `200 OK` with `FormatOutputResponse`
pub const FORMAT_OUTPUT_PATH: &str = "/api/v1/action-output/format";
pub const FORMAT_OUTPUT_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-output/format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatOutputRequest {
    /// The execution context serialized as JSON.
    pub execution_context: serde_json::Value,
    /// Whether to include detailed template content.
    pub include_details: Option<bool>,
    /// Whether to generate a PR comment body.
    pub post_pr_comment: Option<bool>,
}

/// Response body for POST /api/v1/action-output/format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatOutputResponse {
    pub success: bool,
    pub output: FormattedOutput,
    pub summary_length: u64,
    pub annotation_count: u32,
    pub variable_count: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-output/annotate
// ---------------------------------------------------------------------------

/// POST /api/v1/action-output/annotate
///
/// Format a failure or issue into a workflow annotation.
///
/// **Request:** `AnnotateRequest`
/// **Response:** `200 OK` with `AnnotateResponse`
pub const ANNOTATE_PATH: &str = "/api/v1/action-output/annotate";
pub const ANNOTATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-output/annotate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotateRequest {
    pub level: OutputLevel,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub title: Option<String>,
    pub message: String,
}

/// Response body for POST /api/v1/action-output/annotate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotateResponse {
    pub success: bool,
    pub annotation: WorkflowAnnotation,
    pub workflow_command: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-output/summary
// ---------------------------------------------------------------------------

/// POST /api/v1/action-output/summary
///
/// Render and write a step summary.
///
/// **Request:** `WriteSummaryRequest`
/// **Response:** `200 OK` with `WriteSummaryResponse`
pub const WRITE_SUMMARY_PATH: &str = "/api/v1/action-output/summary";
pub const WRITE_SUMMARY_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-output/summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteSummaryRequest {
    pub summary: StepSummary,
    pub append: Option<bool>,
}

/// Response body for POST /api/v1/action-output/summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteSummaryResponse {
    pub success: bool,
    pub bytes_written: u64,
    pub section_count: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/action-output/variables
// ---------------------------------------------------------------------------

/// GET /api/v1/action-output/variables
///
/// Get the reference table of standard output variable names and descriptions.
///
/// **Response:** `200 OK` with `VariablesReferenceResponse`
pub const VARIABLES_REFERENCE_PATH: &str = "/api/v1/action-output/variables";
pub const VARIABLES_REFERENCE_METHOD: &str = "GET";

/// Response for GET /api/v1/action-output/variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariablesReferenceResponse {
    pub variables: Vec<VariableReference>,
}

/// Reference entry for a standard output variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableReference {
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub example: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Action Output API endpoints.
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

/// Standardized error codes for Action Output API.
pub mod error_codes {
    /// Missing required environment variable.
    pub const MISSING_ENV: &str = "MISSING_ENVIRONMENT_VARIABLE";
    /// Failed to write output content.
    pub const WRITE_ERROR: &str = "WRITE_ERROR";
    /// Failed to format output content.
    pub const FORMAT_ERROR: &str = "FORMAT_ERROR";
    /// GitHub API call failed.
    pub const GITHUB_API_ERROR: &str = "GITHUB_API_ERROR";
    /// GitHub token not available.
    pub const MISSING_TOKEN: &str = "MISSING_GITHUB_TOKEN";
    /// Not running in a PR context.
    pub const MISSING_PR_CONTEXT: &str = "MISSING_PR_CONTEXT";
    /// Output variable value too long.
    pub const VARIABLE_TOO_LONG: &str = "VARIABLE_TOO_LONG";
    /// Invalid variable name.
    pub const INVALID_VARIABLE_NAME: &str = "INVALID_VARIABLE_NAME";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Action Output errors.
pub mod status_codes {
    pub const MISSING_ENV: u16 = 400;
    pub const WRITE_ERROR: u16 = 500;
    pub const FORMAT_ERROR: u16 = 400;
    pub const GITHUB_API_ERROR: u16 = 502;
    pub const MISSING_TOKEN: u16 = 401;
    pub const MISSING_PR_CONTEXT: u16 = 400;
    pub const VARIABLE_TOO_LONG: u16 = 400;
    pub const INVALID_VARIABLE_NAME: u16 = 400;
    pub const INTERNAL_ERROR: u16 = 500;
}
