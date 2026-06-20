//! HTTP API contracts for Diff Analyzer endpoints.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! Note: In production, diff analysis is triggered by GitHub events, not HTTP.
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

use crate::diff_analyzer::domain::{FileRisk, PolicyLimits, PrDiff};

use crate::diff_analyzer::application::dto::{
    AnalyzeDiffOutput, ClassifyRiskOutput, DetectAiSignalsOutput, EnforceLimitsOutput,
    ValidatePathsOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All diff analyzer endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/diff-analyzer";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/diff-analyzer/parse
// ---------------------------------------------------------------------------

/// POST /api/v1/diff-analyzer/parse
///
/// Parse a raw git diff into a structured `PrDiff`.
///
/// **Request:** `ParseDiffRequestBody`
/// **Response:** `200 OK` with `ParseDiffResponseBody`
pub const PARSE_DIFF_PATH: &str = "/api/v1/diff-analyzer/parse";
pub const PARSE_DIFF_METHOD: &str = "POST";

/// Request body for POST /api/v1/diff-analyzer/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDiffRequestBody {
    /// The raw git diff output (unified diff format).
    pub raw_diff: String,
    /// Optional PR number for metadata.
    pub pr_number: Option<u64>,
    /// Optional base branch name.
    pub base_branch: Option<String>,
    /// Optional head branch name.
    pub head_branch: Option<String>,
    /// Optional head commit SHA.
    pub head_sha: Option<String>,
}

/// Response body for POST /api/v1/diff-analyzer/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDiffResponseBody {
    pub success: bool,
    pub diff: PrDiff,
    pub files_parsed: usize,
    pub total_size_bytes: u64,
    pub has_binary_files: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/diff-analyzer/validate-paths
// ---------------------------------------------------------------------------

/// POST /api/v1/diff-analyzer/validate-paths
///
/// Validate file paths in a parsed diff.
///
/// **Request:** `ValidatePathsRequestBody`
/// **Response:** `200 OK` with `ValidatePathsResponseBody`
pub const VALIDATE_PATHS_PATH: &str = "/api/v1/diff-analyzer/validate-paths";
pub const VALIDATE_PATHS_METHOD: &str = "POST";

/// Request body for POST /api/v1/diff-analyzer/validate-paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePathsRequestBody {
    /// The parsed PR diff.
    pub diff: PrDiff,
    /// Whether to allow symlinks.
    pub allow_symlinks: Option<bool>,
}

/// Response body for POST /api/v1/diff-analyzer/validate-paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePathsResponseBody {
    pub success: bool,
    pub all_valid: bool,
    pub validation: ValidatePathsOutput,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/diff-analyzer/enforce-limits
// ---------------------------------------------------------------------------

/// POST /api/v1/diff-analyzer/enforce-limits
///
/// Enforce resource limits on a parsed diff.
///
/// **Request:** `EnforceLimitsRequestBody`
/// **Response:** `200 OK` with `EnforceLimitsResponseBody`
pub const ENFORCE_LIMITS_PATH: &str = "/api/v1/diff-analyzer/enforce-limits";
pub const ENFORCE_LIMITS_METHOD: &str = "POST";

/// Request body for POST /api/v1/diff-analyzer/enforce-limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforceLimitsRequestBody {
    /// The parsed PR diff.
    pub diff: PrDiff,
    /// The policy limits to enforce.
    pub limits: PolicyLimits,
    /// Whether to apply progressive degradation (default: true).
    pub progressive_degradation: Option<bool>,
}

/// Response body for POST /api/v1/diff-analyzer/enforce-limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforceLimitsResponseBody {
    pub success: bool,
    pub limits_exceeded: bool,
    pub enforcement: EnforceLimitsOutput,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/diff-analyzer/classify-risk
// ---------------------------------------------------------------------------

/// POST /api/v1/diff-analyzer/classify-risk
///
/// Classify files in a diff by risk level.
///
/// **Request:** `ClassifyRiskRequestBody`
/// **Response:** `200 OK` with `ClassifyRiskResponseBody`
pub const CLASSIFY_RISK_PATH: &str = "/api/v1/diff-analyzer/classify-risk";
pub const CLASSIFY_RISK_METHOD: &str = "POST";

/// Request body for POST /api/v1/diff-analyzer/classify-risk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRiskRequestBody {
    /// The parsed PR diff.
    pub diff: PrDiff,
    /// Custom risk patterns (maps glob pattern to risk level).
    pub custom_patterns: Option<std::collections::HashMap<String, FileRisk>>,
}

/// Response body for POST /api/v1/diff-analyzer/classify-risk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRiskResponseBody {
    pub success: bool,
    pub classification: ClassifyRiskOutput,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/diff-analyzer/detect-ai-signals
// ---------------------------------------------------------------------------

/// POST /api/v1/diff-analyzer/detect-ai-signals
///
/// Detect AI-generated code signals in a diff.
///
/// **Request:** `DetectAiSignalsRequestBody`
/// **Response:** `200 OK` with `DetectAiSignalsResponseBody`
pub const DETECT_AI_PATH: &str = "/api/v1/diff-analyzer/detect-ai-signals";
pub const DETECT_AI_METHOD: &str = "POST";

/// Request body for POST /api/v1/diff-analyzer/detect-ai-signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAiSignalsRequestBody {
    /// The parsed PR diff.
    pub diff: PrDiff,
    /// Confidence threshold for flagging (default: 0.7).
    pub threshold: Option<f64>,
    /// Whether to check indentation patterns.
    pub check_indentation: Option<bool>,
    /// Whether to check comment patterns.
    pub check_comments: Option<bool>,
}

/// Response body for POST /api/v1/diff-analyzer/detect-ai-signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAiSignalsResponseBody {
    pub success: bool,
    pub detection: DetectAiSignalsOutput,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/diff-analyzer/analyze
// ---------------------------------------------------------------------------

/// POST /api/v1/diff-analyzer/analyze
///
/// Run the full diff analysis pipeline.
///
/// **Request:** `AnalyzeDiffRequestBody`
/// **Response:** `200 OK` with `AnalyzeDiffResponseBody`
pub const ANALYZE_DIFF_PATH: &str = "/api/v1/diff-analyzer/analyze";
pub const ANALYZE_DIFF_METHOD: &str = "POST";

/// Request body for POST /api/v1/diff-analyzer/analyze.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeDiffRequestBody {
    /// The raw git diff output.
    pub raw_diff: String,
    /// Policy limits to enforce (uses defaults if not provided).
    pub limits: Option<PolicyLimits>,
    /// PR metadata.
    pub pr_number: Option<u64>,
    pub base_branch: Option<String>,
    pub head_branch: Option<String>,
    pub head_sha: Option<String>,
    /// AI detection threshold.
    pub ai_threshold: Option<f64>,
    /// Custom risk patterns.
    pub custom_risk_patterns: Option<std::collections::HashMap<String, FileRisk>>,
}

/// Response body for POST /api/v1/diff-analyzer/analyze.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeDiffResponseBody {
    pub success: bool,
    pub result: AnalyzeDiffOutput,
    pub processing_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Diff Analyzer API endpoints.
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

/// Standardized error codes for Diff Analyzer API.
pub mod error_codes {
    /// Failed to parse raw diff content.
    pub const DIFF_PARSE_ERROR: &str = "DIFF_PARSE_ERROR";
    /// Path traversal or injection detected.
    pub const PATH_VIOLATION: &str = "PATH_VIOLATION";
    /// Diff exceeds configured size limit.
    pub const DIFF_TOO_LARGE: &str = "DIFF_TOO_LARGE";
    /// Too many files in diff.
    pub const TOO_MANY_FILES: &str = "TOO_MANY_FILES";
    /// File exceeds per-file line limit.
    pub const FILE_TOO_LARGE: &str = "FILE_TOO_LARGE";
    /// Invalid policy limits configuration.
    pub const INVALID_LIMITS: &str = "INVALID_POLICY_LIMITS";
    /// AI signal detection internal error.
    pub const AI_DETECTION_ERROR: &str = "AI_DETECTION_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Diff Analyzer errors.
pub mod status_codes {
    pub const DIFF_PARSE_ERROR: u16 = 400;
    pub const PATH_VIOLATION: u16 = 422;
    pub const DIFF_TOO_LARGE: u16 = 413;
    pub const TOO_MANY_FILES: u16 = 413;
    pub const FILE_TOO_LARGE: u16 = 413;
    pub const INVALID_LIMITS: u16 = 400;
    pub const AI_DETECTION_ERROR: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
