//! HTTP API contracts for Enforcement endpoints.
//!
//! @canonical .pi/architecture/modules/enforcement.md
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

use serde::{Deserialize, Serialize};

use crate::enforcement::application::dto::{GetBudgetStatusOutput, ResourceBudgetStatus};

use crate::enforcement::domain::ToolRiskLevel;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All enforcement endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/enforcement";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/enforcement/{id}/budgets
// ---------------------------------------------------------------------------

/// GET /api/v1/enforcement/{id}/budgets
///
/// Get the current budget status for all tracked resources in an execution.
///
/// **Path Param:** `id` — Execution UUID
/// **Response:** `200 OK` with `BudgetStatusResponse`
pub const BUDGET_STATUS_PATH: &str = "/api/v1/enforcement/{id}/budgets";
pub const BUDGET_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/enforcement/{id}/budgets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatusResponse {
    pub execution_id: String,
    pub budgets: Vec<ResourceBudgetStatus>,
    pub has_warnings: bool,
    pub has_exceeded_limits: bool,
}

impl From<GetBudgetStatusOutput> for BudgetStatusResponse {
    fn from(output: GetBudgetStatusOutput) -> Self {
        Self {
            execution_id: output.execution_id,
            budgets: output.budgets,
            has_warnings: output.has_warnings,
            has_exceeded_limits: output.has_exceeded_limits,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/enforcement/{id}/limits
// ---------------------------------------------------------------------------

/// GET /api/v1/enforcement/{id}/limits
///
/// Check execution limits and get any that have been reached.
///
/// **Path Param:** `id` — Execution UUID
/// **Response:** `200 OK` with `ExecutionLimitsResponse`
pub const EXECUTION_LIMITS_PATH: &str = "/api/v1/enforcement/{id}/limits";
pub const EXECUTION_LIMITS_METHOD: &str = "GET";

/// Response body for GET /api/v1/enforcement/{id}/limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLimitsResponse {
    pub execution_id: String,
    pub limits_reached: Vec<LimitReached>,
    pub has_reached_limit: bool,
    pub should_terminate: bool,
}

/// A limit that has been reached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitReached {
    pub limit_type: String,
    pub current: u64,
    pub max: u64,
    pub is_hard_limit: bool,
    pub is_soft_limit: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/enforcement/{id}/evaluate
// ---------------------------------------------------------------------------

/// POST /api/v1/enforcement/{id}/evaluate
///
/// Evaluate whether a tool call is allowed for this execution.
/// Returns the decision with reasoning and any active warnings.
///
/// **Path Param:** `id` — Execution UUID
/// **Request:** `EvaluateToolRequest`
/// **Response:** `200 OK` with `EvaluateToolResponse`
pub const EVALUATE_TOOL_PATH: &str = "/api/v1/enforcement/{id}/evaluate";
pub const EVALUATE_TOOL_METHOD: &str = "POST";

/// Request body for POST /api/v1/enforcement/{id}/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateToolRequest {
    /// Identifier of the DAG node making the request.
    pub node_id: String,
    /// The name of the tool being called.
    pub tool: String,
    /// Whether this is a retry of a previously failed call.
    pub is_retry: Option<bool>,
    /// The current attempt number.
    pub attempt: Option<u32>,
}

/// Response body for POST /api/v1/enforcement/{id}/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateToolResponse {
    pub allowed: bool,
    pub reason: Option<String>,
    pub risk_level: ToolRiskLevel,
    pub requires_confirmation: bool,
    pub dry_run: bool,
    pub active_warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/enforcement/policies
// ---------------------------------------------------------------------------

/// GET /api/v1/enforcement/policies
///
/// Get the current tool policies in effect.
///
/// **Response:** `200 OK` with `ToolPoliciesResponse`
pub const TOOL_POLICIES_PATH: &str = "/api/v1/enforcement/policies";
pub const TOOL_POLICIES_METHOD: &str = "GET";

/// Response body for GET /api/v1/enforcement/policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPoliciesResponse {
    pub default_policy: ToolPolicyDto,
    pub tool_overrides: Vec<ToolPolicyDto>,
}

/// DTO for a tool policy in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicyDto {
    pub tool: Option<String>,
    pub allowed: bool,
    pub risk_level: ToolRiskLevel,
    pub requires_confirmation: bool,
    pub dry_run: bool,
    pub max_calls: Option<u64>,
    pub budget_key: Option<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/enforcement/config/reload
// ---------------------------------------------------------------------------

/// POST /api/v1/enforcement/config/reload
///
/// Reload enforcement configuration from the source.
///
/// **Response:** `200 OK` with `ConfigReloadResponse`
pub const RELOAD_CONFIG_PATH: &str = "/api/v1/enforcement/config/reload";
pub const RELOAD_CONFIG_METHOD: &str = "POST";

/// Response body for POST /api/v1/enforcement/config/reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReloadResponse {
    pub success: bool,
    pub preset: String,
    pub budget_count: u32,
    pub policy_count: u32,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Enforcement API endpoints.
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

/// Standardized error codes for Enforcement API.
pub mod error_codes {
    /// Tool call blocked by policy.
    pub const TOOL_BLOCKED: &str = "TOOL_BLOCKED";
    /// Resource budget exceeded hard limit.
    pub const BUDGET_EXCEEDED: &str = "BUDGET_EXCEEDED";
    /// Execution limit reached.
    pub const EXECUTION_LIMIT_REACHED: &str = "EXECUTION_LIMIT_REACHED";
    /// Policy not found for the requested tool.
    pub const POLICY_NOT_FOUND: &str = "POLICY_NOT_FOUND";
    /// Budget not found for the requested resource.
    pub const BUDGET_NOT_FOUND: &str = "BUDGET_NOT_FOUND";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "ENFORCEMENT_INTERNAL_ERROR";
}

/// HTTP status code mappings for Enforcement errors.
pub mod status_codes {
    pub const TOOL_BLOCKED: u16 = 403;
    pub const BUDGET_EXCEEDED: u16 = 429;
    pub const EXECUTION_LIMIT_REACHED: u16 = 429;
    pub const POLICY_NOT_FOUND: u16 = 404;
    pub const BUDGET_NOT_FOUND: u16 = 404;
    pub const INTERNAL_ERROR: u16 = 500;
}
