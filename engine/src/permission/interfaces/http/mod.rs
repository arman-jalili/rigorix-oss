//! HTTP API contracts for Permission Enforcer endpoints.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
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

use crate::permission::application::dto::{
    CheckFileWriteOutput, CheckPermissionOutput, PermissionStatusOutput,
};
use crate::permission::domain::PermissionMode;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All permission endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/permission";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/permission/status
// ---------------------------------------------------------------------------

/// GET /api/v1/permission/status
///
/// Get the current permission status (active mode and config summary).
///
/// **Response:** `200 OK` with `PermissionStatusResponse`
pub const PERMISSION_STATUS_PATH: &str = "/api/v1/permission/status";
pub const PERMISSION_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/permission/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatusResponse {
    pub active_mode: PermissionMode,
    pub allow_count: u32,
    pub deny_count: u32,
    pub ask_count: u32,
    pub tool_permission_count: u32,
}

impl From<PermissionStatusOutput> for PermissionStatusResponse {
    fn from(output: PermissionStatusOutput) -> Self {
        Self {
            active_mode: output.active_mode,
            allow_count: output.config_summary.allow_count,
            deny_count: output.config_summary.deny_count,
            ask_count: output.config_summary.ask_count,
            tool_permission_count: output.config_summary.tool_permission_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/permission/evaluate
// ---------------------------------------------------------------------------

/// POST /api/v1/permission/evaluate
///
/// Evaluate whether a tool call is allowed by the current permission policy.
/// Returns the decision with reasoning and required/active mode info.
///
/// **Request:** `EvaluateToolPermissionRequest`
/// **Response:** `200 OK` with `EvaluateToolPermissionResponse`
pub const EVALUATE_PERMISSION_PATH: &str = "/api/v1/permission/evaluate";
pub const EVALUATE_PERMISSION_METHOD: &str = "POST";

/// Request body for POST /api/v1/permission/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateToolPermissionRequest {
    /// The name of the tool being evaluated.
    pub tool: String,
    /// The input/arguments to the tool.
    pub input: String,
}

/// Response body for POST /api/v1/permission/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateToolPermissionResponse {
    pub outcome: String,
    pub active_mode: String,
    pub required_mode: String,
    pub reason: Option<String>,
    pub requires_confirmation: bool,
}

impl From<CheckPermissionOutput> for EvaluateToolPermissionResponse {
    fn from(output: CheckPermissionOutput) -> Self {
        Self {
            outcome: output.outcome,
            active_mode: output.active_mode,
            required_mode: output.required_mode,
            reason: output.reason,
            requires_confirmation: output.requires_confirmation,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/permission/mode
// ---------------------------------------------------------------------------

/// POST /api/v1/permission/mode
///
/// Set the active permission mode.
///
/// **Request:** `SetPermissionModeRequest`
/// **Response:** `200 OK` with `SetPermissionModeResponse`
pub const SET_MODE_PATH: &str = "/api/v1/permission/mode";
pub const SET_MODE_METHOD: &str = "POST";

/// Request body for POST /api/v1/permission/mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPermissionModeRequest {
    /// The new permission mode.
    pub mode: PermissionMode,
    /// Optional reason for the mode change.
    pub reason: Option<String>,
}

/// Response body for POST /api/v1/permission/mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPermissionModeResponse {
    /// The previous mode.
    pub previous_mode: PermissionMode,
    /// The current (new) mode.
    pub current_mode: PermissionMode,
    /// Whether the change was successful.
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/permission/file-write
// ---------------------------------------------------------------------------

/// POST /api/v1/permission/file-write
///
/// Check whether a file write at the given path is allowed.
///
/// **Request:** `CheckFileWriteRequest`
/// **Response:** `200 OK` with `CheckFileWriteResponse`
pub const FILE_WRITE_PATH: &str = "/api/v1/permission/file-write";
pub const FILE_WRITE_METHOD: &str = "POST";

/// Request body for POST /api/v1/permission/file-write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckFileWriteRequest {
    /// The path to check.
    pub path: String,
    /// The workspace root.
    pub workspace_root: String,
}

/// Response body for POST /api/v1/permission/file-write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckFileWriteResponse {
    pub allowed: bool,
    pub active_mode: String,
    pub within_workspace: bool,
    pub reason: Option<String>,
}

impl From<CheckFileWriteOutput> for CheckFileWriteResponse {
    fn from(output: CheckFileWriteOutput) -> Self {
        Self {
            allowed: output.allowed,
            active_mode: output.active_mode,
            within_workspace: output.within_workspace,
            reason: output.reason,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Permission API endpoints.
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
    /// Detailed error context (optional).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available).
    pub request_id: Option<String>,
}

/// Standardized error codes for Permission API.
pub mod error_codes {
    /// Tool call denied by permission policy.
    pub const PERMISSION_DENIED: &str = "PERMISSION_DENIED";
    /// Invalid permission mode.
    pub const INVALID_MODE: &str = "INVALID_PERMISSION_MODE";
    /// Policy not found.
    pub const POLICY_NOT_FOUND: &str = "PERMISSION_POLICY_NOT_FOUND";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "PERMISSION_INTERNAL_ERROR";
}

/// HTTP status code mappings for Permission errors.
pub mod status_codes {
    pub const PERMISSION_DENIED: u16 = 403;
    pub const INVALID_MODE: u16 = 400;
    pub const POLICY_NOT_FOUND: u16 = 404;
    pub const INTERNAL_ERROR: u16 = 500;
}
