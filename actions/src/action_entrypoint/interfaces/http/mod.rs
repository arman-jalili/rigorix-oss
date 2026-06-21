//! HTTP API contracts for Action Entrypoint endpoints.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
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

use crate::action_entrypoint::domain::{ActionContext, ActionMode, ActionOutput};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All action entrypoint endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/action-entrypoint";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-entrypoint/dispatch
// ---------------------------------------------------------------------------

/// POST /api/v1/action-entrypoint/dispatch
///
/// Dispatch an action execution. This is the primary API endpoint —
/// it takes a built context and routes to the appropriate engine call.
///
/// **Request:** `DispatchRequest`
/// **Response:** `200 OK` with `DispatchResponse`
pub const DISPATCH_PATH: &str = "/api/v1/action-entrypoint/dispatch";
pub const DISPATCH_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-entrypoint/dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRequest {
    /// The action execution context.
    pub context: ActionContext,
    /// Optional timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// Whether to force dispatch even for non-routable events.
    pub force: Option<bool>,
}

/// Response body for POST /api/v1/action-entrypoint/dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResponse {
    pub success: bool,
    pub output: ActionOutput,
    pub mode: ActionMode,
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-entrypoint/resolve-mode
// ---------------------------------------------------------------------------

/// POST /api/v1/action-entrypoint/resolve-mode
///
/// Resolve the execution mode from inputs and event context.
/// Useful for testing mode resolution logic without dispatching.
///
/// **Request:** `ResolveModeRequest`
/// **Response:** `200 OK` with `ResolveModeResponse`
pub const RESOLVE_MODE_PATH: &str = "/api/v1/action-entrypoint/resolve-mode";
pub const RESOLVE_MODE_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-entrypoint/resolve-mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveModeRequest {
    /// Raw INPUT_MODE string (optional).
    pub input_mode: Option<String>,
    /// GitHub event name.
    pub event_name: String,
    /// Event payload JSON (optional, used for slash command detection).
    pub event_payload: Option<serde_json::Value>,
    /// Raw INPUT_INTENT string (optional).
    pub input_intent: Option<String>,
}

/// Response body for POST /api/v1/action-entrypoint/resolve-mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveModeResponse {
    pub mode: ActionMode,
    pub source: String,
    pub unambiguous: bool,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-entrypoint/context
// ---------------------------------------------------------------------------

/// POST /api/v1/action-entrypoint/context
///
/// Build an ActionContext from environment overrides or explicit values.
/// Useful for testing context construction logic.
///
/// **Request:** `BuildContextRequest`
/// **Response:** `200 OK` with `BuildContextResponse`
pub const BUILD_CONTEXT_PATH: &str = "/api/v1/action-entrypoint/context";
pub const BUILD_CONTEXT_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-entrypoint/context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextRequest {
    /// Environment variable overrides (name → value).
    pub env_overrides: Option<std::collections::HashMap<String, String>>,
    /// Override workspace root.
    pub workspace_override: Option<String>,
    /// Override event name.
    pub event_name_override: Option<String>,
    /// Override event payload path.
    pub event_path_override: Option<String>,
    /// Override event payload JSON content.
    pub event_payload_override: Option<String>,
}

/// Response body for POST /api/v1/action-entrypoint/context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextResponse {
    pub context: ActionContext,
    pub event_name: String,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/action-entrypoint/status
// ---------------------------------------------------------------------------

/// GET /api/v1/action-entrypoint/status
///
/// Health check and status endpoint for the entrypoint module.
///
/// **Response:** `200 OK` with `StatusResponse`
pub const STATUS_PATH: &str = "/api/v1/action-entrypoint/status";
pub const STATUS_METHOD: &str = "GET";

/// Response for GET /api/v1/action-entrypoint/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub healthy: bool,
    pub supported_modes: Vec<String>,
    pub supported_events: Vec<String>,
    pub available: bool,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Action Entrypoint API endpoints.
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

/// Standardized error codes for Action Entrypoint API.
pub mod error_codes {
    /// Mode could not be resolved.
    pub const MODE_RESOLUTION_ERROR: &str = "MODE_RESOLUTION_ERROR";
    /// Unsupported event type.
    pub const UNSUPPORTED_EVENT: &str = "UNSUPPORTED_EVENT";
    /// Missing required context.
    pub const MISSING_CONTEXT: &str = "MISSING_CONTEXT";
    /// Engine orchestrator error.
    pub const ENGINE_ERROR: &str = "ENGINE_ERROR";
    /// Validation loop error.
    pub const VALIDATION_LOOP_ERROR: &str = "VALIDATION_LOOP_ERROR";
    /// Invalid workspace root.
    pub const INVALID_WORKSPACE: &str = "INVALID_WORKSPACE";
    /// Context repository error.
    pub const CONTEXT_REPOSITORY_ERROR: &str = "CONTEXT_REPOSITORY_ERROR";
    /// Output formatting error.
    pub const OUTPUT_ERROR: &str = "OUTPUT_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Action Entrypoint errors.
pub mod status_codes {
    pub const MODE_RESOLUTION_ERROR: u16 = 400;
    pub const UNSUPPORTED_EVENT: u16 = 400;
    pub const MISSING_CONTEXT: u16 = 400;
    pub const ENGINE_ERROR: u16 = 502;
    pub const VALIDATION_LOOP_ERROR: u16 = 502;
    pub const INVALID_WORKSPACE: u16 = 400;
    pub const CONTEXT_REPOSITORY_ERROR: u16 = 500;
    pub const OUTPUT_ERROR: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
