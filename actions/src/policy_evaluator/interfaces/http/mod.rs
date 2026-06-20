//! HTTP API contracts for Policy Evaluator endpoints.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! Note: In production, policy evaluation is triggered by GitHub events, not HTTP.
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

use crate::diff_analyzer::domain::PrDiff;
use crate::policy_evaluator::domain::{OrgPolicyConfig, PolicyDocument, PolicyResult};

use crate::policy_evaluator::application::dto::{
    EvaluatePolicyOutput, MergePoliciesOutput, RunPolicyEvaluationOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All policy evaluator endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/policy-evaluator";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy-evaluator/load
// ---------------------------------------------------------------------------

/// POST /api/v1/policy-evaluator/load
///
/// Load a policy document from a repository base branch.
///
/// **Request:** `LoadPolicyRequestBody`
/// **Response:** `200 OK` with `LoadPolicyResponseBody`
pub const LOAD_POLICY_PATH: &str = "/api/v1/policy-evaluator/load";
pub const LOAD_POLICY_METHOD: &str = "POST";

/// Request body for POST /api/v1/policy-evaluator/load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPolicyRequestBody {
    /// Path to the policy file in the repository.
    pub policy_path: String,
    /// The base git ref to load from (e.g., "origin/main").
    pub base_ref: String,
    /// Repository in "owner/repo" format.
    pub repo: String,
}

/// Response body for POST /api/v1/policy-evaluator/load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPolicyResponseBody {
    pub success: bool,
    pub policy: PolicyDocument,
    pub source_ref: String,
    pub from_base_branch: bool,
    pub rules_count: usize,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy-evaluator/evaluate
// ---------------------------------------------------------------------------

/// POST /api/v1/policy-evaluator/evaluate
///
/// Evaluate a PR diff against policy rules.
///
/// **Request:** `EvaluatePolicyRequestBody`
/// **Response:** `200 OK` with `EvaluatePolicyResponseBody`
pub const EVALUATE_POLICY_PATH: &str = "/api/v1/policy-evaluator/evaluate";
pub const EVALUATE_POLICY_METHOD: &str = "POST";

/// Request body for POST /api/v1/policy-evaluator/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyRequestBody {
    /// The parsed PR diff to evaluate.
    pub diff: PrDiff,
    /// The loaded policy document.
    pub policy: PolicyDocument,
    /// Whether to fail on violations.
    pub fail_on_violation: bool,
}

/// Response body for POST /api/v1/policy-evaluator/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyResponseBody {
    pub success: bool,
    pub result: PolicyResult,
    pub evaluation: EvaluatePolicyOutput,
    pub files_evaluated: usize,
    pub processing_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy-evaluator/merge
// ---------------------------------------------------------------------------

/// POST /api/v1/policy-evaluator/merge
///
/// Merge organization-level policy with repository policy.
///
/// **Request:** `MergePoliciesRequestBody`
/// **Response:** `200 OK` with `MergePoliciesResponseBody`
pub const MERGE_POLICIES_PATH: &str = "/api/v1/policy-evaluator/merge";
pub const MERGE_POLICIES_METHOD: &str = "POST";

/// Request body for POST /api/v1/policy-evaluator/merge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePoliciesRequestBody {
    /// The repository-level policy.
    pub repo_policy: PolicyDocument,
    /// The organization-level policy (optional).
    pub org_policy: Option<PolicyDocument>,
    /// Merge strategy: "restrictive", "repo_preferred", or "org_preferred".
    pub merge_strategy: Option<String>,
}

/// Response body for POST /api/v1/policy-evaluator/merge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePoliciesResponseBody {
    pub success: bool,
    pub merged: MergePoliciesOutput,
    pub org_rules_added: usize,
    pub limits_tightened: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy-evaluator/detect-tamper
// ---------------------------------------------------------------------------

/// POST /api/v1/policy-evaluator/detect-tamper
///
/// Detect if a PR modifies the policy file (tamper detection).
///
/// **Request:** `DetectTamperRequestBody`
/// **Response:** `200 OK` with `DetectTamperResponseBody`
pub const DETECT_TAMPER_PATH: &str = "/api/v1/policy-evaluator/detect-tamper";
pub const DETECT_TAMPER_METHOD: &str = "POST";

/// Request body for POST /api/v1/policy-evaluator/detect-tamper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectTamperRequestBody {
    /// The parsed PR diff.
    pub diff: PrDiff,
    /// The expected policy file path.
    pub policy_path: String,
}

/// Response body for POST /api/v1/policy-evaluator/detect-tamper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectTamperResponseBody {
    pub success: bool,
    pub tamper_detected: bool,
    pub tampered_path: Option<String>,
    pub proceed: bool,
    pub warning: Option<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy-evaluator/run
// ---------------------------------------------------------------------------

/// POST /api/v1/policy-evaluator/run
///
/// Run the full policy evaluation pipeline: load → merge → evaluate.
///
/// **Request:** `RunPolicyEvaluationRequestBody`
/// **Response:** `200 OK` with `RunPolicyEvaluationResponseBody`
pub const RUN_EVALUATION_PATH: &str = "/api/v1/policy-evaluator/run";
pub const RUN_EVALUATION_METHOD: &str = "POST";

/// Request body for POST /api/v1/policy-evaluator/run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPolicyEvaluationRequestBody {
    /// The parsed PR diff to evaluate.
    pub diff: PrDiff,
    /// Path to the policy file.
    pub policy_path: String,
    /// Base git ref to load policy from.
    pub base_ref: String,
    /// Repository in "owner/repo" format.
    pub repo: String,
    /// Organization policy configuration (optional).
    pub org_policy_config: Option<OrgPolicyConfig>,
    /// Whether to fail on violations.
    pub fail_on_violation: bool,
}

/// Response body for POST /api/v1/policy-evaluator/run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPolicyEvaluationResponseBody {
    pub success: bool,
    pub result: RunPolicyEvaluationOutput,
    pub has_blocking: bool,
    pub has_warnings: bool,
    pub violation_count: usize,
    pub processing_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Policy Evaluator API endpoints.
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

/// Standardized error codes for Policy Evaluator API.
pub mod error_codes {
    /// Policy file not found at the given path.
    pub const FILE_NOT_FOUND: &str = "POLICY_FILE_NOT_FOUND";
    /// Invalid TOML syntax in policy file.
    pub const INVALID_SYNTAX: &str = "POLICY_INVALID_SYNTAX";
    /// Unsupported policy version.
    pub const UNSUPPORTED_VERSION: &str = "POLICY_UNSUPPORTED_VERSION";
    /// Invalid glob pattern in a rule.
    pub const INVALID_GLOB_PATTERN: &str = "POLICY_INVALID_GLOB_PATTERN";
    /// Policy evaluation error.
    pub const EVALUATION_ERROR: &str = "POLICY_EVALUATION_ERROR";
    /// Policy tampering detected.
    pub const TAMPER_DETECTED: &str = "POLICY_TAMPER_DETECTED";
    /// Organization policy load error.
    pub const ORG_POLICY_ERROR: &str = "ORG_POLICY_ERROR";
    /// Policy merge error.
    pub const MERGE_ERROR: &str = "POLICY_MERGE_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Policy Evaluator errors.
pub mod status_codes {
    pub const FILE_NOT_FOUND: u16 = 404;
    pub const INVALID_SYNTAX: u16 = 400;
    pub const UNSUPPORTED_VERSION: u16 = 400;
    pub const INVALID_GLOB_PATTERN: u16 = 400;
    pub const EVALUATION_ERROR: u16 = 500;
    pub const TAMPER_DETECTED: u16 = 409;
    pub const ORG_POLICY_ERROR: u16 = 502;
    pub const MERGE_ERROR: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
