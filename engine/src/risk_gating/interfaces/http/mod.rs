//! HTTP API contracts for Risk Gating endpoints.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
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

use crate::risk_gating::application::dto::{ClassifyToolOutput, EvaluateGateOutput, OverrideToolOutput};
use crate::risk_gating::domain::risk_level::RiskLevel;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All risk gating endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/risk-gating";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/risk-gating/evaluate
// ---------------------------------------------------------------------------

/// POST /api/v1/risk-gating/evaluate
///
/// Evaluate the risk gate for a tool call. Classifies the tool and
/// returns the gating decision (auto-execute, confirm, or dry-run).
///
/// **Request:** `EvaluateGateRequest`
/// **Response:** `200 OK` with `EvaluateGateResponse`
pub const EVALUATE_GATE_PATH: &str = "/api/v1/risk-gating/evaluate";
pub const EVALUATE_GATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/risk-gating/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateGateRequest {
    /// The execution ID.
    pub execution_id: String,
    /// Identifier of the DAG node making the request.
    pub node_id: String,
    /// The name of the tool being called.
    pub tool: String,
    /// Optional tool parameters for context-aware classification.
    pub parameters: Option<serde_json::Value>,
    /// Whether this is a retry of a previously gated call.
    #[serde(default)]
    pub is_retry: bool,
}

/// Response body for POST /api/v1/risk-gating/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateGateResponse {
    pub risk_level: RiskLevel,
    pub gating_action: String,
    pub allowed: bool,
    pub reason: String,
    pub from_override: bool,
    pub gate_id: String,
    pub warnings: Vec<String>,
}

impl From<EvaluateGateOutput> for EvaluateGateResponse {
    fn from(output: EvaluateGateOutput) -> Self {
        Self {
            risk_level: output.risk_level,
            gating_action: format!("{:?}", output.gating_action),
            allowed: output.allowed,
            reason: output.reason,
            from_override: output.from_override,
            gate_id: output.gate_id,
            warnings: output.warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/risk-gating/classify
// ---------------------------------------------------------------------------

/// POST /api/v1/risk-gating/classify
///
/// Classify a tool into a risk level without evaluating the gate.
/// Useful for informational queries.
///
/// **Request:** `ClassifyToolRequest`
/// **Response:** `200 OK` with `ClassifyToolResponse`
pub const CLASSIFY_TOOL_PATH: &str = "/api/v1/risk-gating/classify";
pub const CLASSIFY_TOOL_METHOD: &str = "POST";

/// Request body for POST /api/v1/risk-gating/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyToolRequest {
    /// The name of the tool to classify.
    pub tool: String,
    /// Optional tool parameters.
    pub parameters: Option<serde_json::Value>,
}

/// Response body for POST /api/v1/risk-gating/classify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyToolResponse {
    pub risk_level: RiskLevel,
    pub reason: String,
    pub from_override: bool,
}

impl From<ClassifyToolOutput> for ClassifyToolResponse {
    fn from(output: ClassifyToolOutput) -> Self {
        Self {
            risk_level: output.risk_level,
            reason: output.reason,
            from_override: output.from_override,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/risk-gating/resolve
// ---------------------------------------------------------------------------

/// POST /api/v1/risk-gating/resolve
///
/// Resolve a pending gate (approve or reject a confirmation request).
///
/// **Request:** `ResolveGateRequest`
/// **Response:** `200 OK` with `ResolveGateResponse`
pub const RESOLVE_GATE_PATH: &str = "/api/v1/risk-gating/resolve";
pub const RESOLVE_GATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/risk-gating/resolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveGateRequest {
    /// The execution ID.
    pub execution_id: String,
    /// The gate ID returned from evaluate.
    pub gate_id: String,
    /// Whether to approve (true) or reject (false).
    pub approved: bool,
    /// Optional reason for the decision.
    pub reason: Option<String>,
}

/// Response body for POST /api/v1/risk-gating/resolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveGateResponse {
    pub gate_id: String,
    pub approved: bool,
    pub can_proceed: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/risk-gating/{id}/status
// ---------------------------------------------------------------------------

/// GET /api/v1/risk-gating/{id}/status
///
/// Get the current gate status for an execution.
///
/// **Path Param:** `id` — Execution UUID
/// **Response:** `200 OK` with `GateStatusResponse`
pub const GATE_STATUS_PATH: &str = "/api/v1/risk-gating/{id}/status";
pub const GATE_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/risk-gating/{id}/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStatusResponse {
    pub has_pending_gates: bool,
    pub pending_gates: Vec<PendingGateResponse>,
    pub total_resolved: u32,
    pub total_approved: u32,
    pub total_rejected: u32,
}

/// DTO for a pending gate in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingGateResponse {
    pub gate_id: String,
    pub node_id: String,
    pub tool: String,
    pub risk_level: RiskLevel,
    pub action: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/risk-gating/override
// ---------------------------------------------------------------------------

/// POST /api/v1/risk-gating/override
///
/// Override the risk level for a specific tool at runtime.
///
/// **Request:** `OverrideToolRequest`
/// **Response:** `200 OK` with `OverrideToolResponse`
pub const OVERRIDE_TOOL_PATH: &str = "/api/v1/risk-gating/override";
pub const OVERRIDE_TOOL_METHOD: &str = "POST";

/// Request body for POST /api/v1/risk-gating/override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideToolRequest {
    /// The execution ID.
    pub execution_id: String,
    /// The name of the tool to override.
    pub tool: String,
    /// The new risk level.
    pub new_level: RiskLevel,
    /// Optional reason for the override.
    pub reason: Option<String>,
}

/// Response body for POST /api/v1/risk-gating/override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideToolResponse {
    pub tool: String,
    pub new_level: RiskLevel,
    pub previous_level: Option<RiskLevel>,
    pub applied: bool,
}

impl From<OverrideToolOutput> for OverrideToolResponse {
    fn from(output: OverrideToolOutput) -> Self {
        Self {
            tool: output.tool,
            new_level: output.new_level,
            previous_level: output.previous_level,
            applied: output.applied,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/risk-gating/config/reload
// ---------------------------------------------------------------------------

/// POST /api/v1/risk-gating/config/reload
///
/// Reload risk configuration from the source.
///
/// **Response:** `200 OK` with `ConfigReloadResponse`
pub const RELOAD_CONFIG_PATH: &str = "/api/v1/risk-gating/config/reload";
pub const RELOAD_CONFIG_METHOD: &str = "POST";

/// Response body for POST /api/v1/risk-gating/config/reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReloadResponse {
    pub success: bool,
    pub override_count: u32,
    pub auto_confirm_low: bool,
    pub require_review_medium: bool,
    pub dry_run_high: bool,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Risk Gating API endpoints.
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

/// Standardized error codes for Risk Gating API.
pub mod error_codes {
    /// Tool not recognized by classifier.
    pub const UNKNOWN_TOOL: &str = "UNKNOWN_TOOL";
    /// Gate not found for the given gate ID.
    pub const GATE_NOT_FOUND: &str = "GATE_NOT_FOUND";
    /// Gate has already been resolved.
    pub const GATE_ALREADY_RESOLVED: &str = "GATE_ALREADY_RESOLVED";
    /// Invalid risk level override value.
    pub const INVALID_OVERRIDE: &str = "INVALID_OVERRIDE";
    /// Invalid gate state.
    pub const INVALID_GATE_STATE: &str = "INVALID_GATE_STATE";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "RISK_GATE_INTERNAL_ERROR";
}

/// HTTP status code mappings for Risk Gating errors.
pub mod status_codes {
    pub const UNKNOWN_TOOL: u16 = 404;
    pub const GATE_NOT_FOUND: u16 = 404;
    pub const GATE_ALREADY_RESOLVED: u16 = 409;
    pub const INVALID_OVERRIDE: u16 = 400;
    pub const INVALID_GATE_STATE: u16 = 400;
    pub const INTERNAL_ERROR: u16 = 500;
}
