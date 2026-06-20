//! HTTP API contracts for Action Input endpoints.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
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

use crate::action_input::domain::{
    ActionConfig, ActionInputs, CiEnvironment, CommentCommand, GitHubEvent,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All action input endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/action-input";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/action-input/inputs
// ---------------------------------------------------------------------------

/// GET /api/v1/action-input/inputs
///
/// Retrieve the current parsed action inputs.
///
/// **Response:** `200 OK` with `GetInputsResponse`
pub const GET_INPUTS_PATH: &str = "/api/v1/action-input/inputs";
pub const GET_INPUTS_METHOD: &str = "GET";

/// Response for GET /api/v1/action-input/inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInputsResponse {
    /// The parsed action inputs.
    pub inputs: ActionInputs,
    /// Number of populated input fields.
    pub populated_count: u32,
    /// Environment variables that were checked.
    pub env_vars_checked: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-input/parse
// ---------------------------------------------------------------------------

/// POST /api/v1/action-input/parse
///
/// Parse inputs from a provided environment map (for testing/debugging).
///
/// **Request:** `ParseInputsRequest`
/// **Response:** `200 OK` with `ParseInputsResponse`
pub const PARSE_INPUTS_PATH: &str = "/api/v1/action-input/parse";
pub const PARSE_INPUTS_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-input/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseInputsRequest {
    /// Environment variable map (key = var name, value = var value).
    pub env: std::collections::HashMap<String, String>,
    /// Optional env prefix (default: `INPUT_`).
    pub env_prefix: Option<String>,
}

/// Response body for POST /api/v1/action-input/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseInputsResponse {
    pub success: bool,
    pub inputs: ActionInputs,
    pub warnings: Vec<String>,
    pub missing_required: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-input/event
// ---------------------------------------------------------------------------

/// POST /api/v1/action-input/event
///
/// Parse a GitHub event payload JSON (for testing/debugging).
///
/// **Request:** `ParseEventRequest`
/// **Response:** `200 OK` with `ParseEventResponse`
pub const PARSE_EVENT_PATH: &str = "/api/v1/action-input/event";
pub const PARSE_EVENT_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-input/event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseEventRequest {
    /// The raw event payload JSON.
    pub payload: serde_json::Value,
    /// The event name (e.g. "pull_request", "issue_comment").
    pub event_name: String,
}

/// Response body for POST /api/v1/action-input/event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseEventResponse {
    pub success: bool,
    pub event: GitHubEvent,
    pub event_type: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-input/comment
// ---------------------------------------------------------------------------

/// POST /api/v1/action-input/comment
///
/// Parse a comment for `/rigorix` commands (for testing/debugging).
///
/// **Request:** `ParseCommentRequest`
/// **Response:** `200 OK` with `ParseCommentResponse`
pub const PARSE_COMMENT_PATH: &str = "/api/v1/action-input/comment";
pub const PARSE_COMMENT_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-input/comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseCommentRequest {
    /// The raw comment body text.
    pub body: String,
    /// The issue/PR number.
    pub issue_number: u64,
    /// The commenter's username.
    pub commenter: String,
}

/// Response body for POST /api/v1/action-input/comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseCommentResponse {
    pub found: bool,
    pub command: Option<CommentCommand>,
    pub command_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/action-input/ci
// ---------------------------------------------------------------------------

/// GET /api/v1/action-input/ci
///
/// Detect the CI environment.
///
/// **Response:** `200 OK` with `CiDetectionResponse`
pub const CI_DETECTION_PATH: &str = "/api/v1/action-input/ci";
pub const CI_DETECTION_METHOD: &str = "GET";

/// Response for GET /api/v1/action-input/ci.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiDetectionResponse {
    pub is_ci: bool,
    pub environment: CiEnvironment,
    pub permission_mode: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/action-input/config
// ---------------------------------------------------------------------------

/// POST /api/v1/action-input/config
///
/// Load and merge action configuration.
///
/// **Request:** `LoadConfigRequest`
/// **Response:** `200 OK` with `LoadConfigResponse`
pub const LOAD_CONFIG_PATH: &str = "/api/v1/action-input/config";
pub const LOAD_CONFIG_METHOD: &str = "POST";

/// Request body for POST /api/v1/action-input/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfigRequest {
    /// Override action.yml content (YAML string).
    pub action_yml_override: Option<String>,
    /// Override environment variables.
    pub env_override: Option<std::collections::HashMap<String, String>>,
    /// Allow empty/missing action.yml.
    pub allow_empty: Option<bool>,
}

/// Response body for POST /api/v1/action-input/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfigResponse {
    pub success: bool,
    pub config: ActionConfig,
    pub sources: Vec<String>,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Action Input API endpoints.
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

/// Standardized error codes for Action Input API.
pub mod error_codes {
    /// Missing required input.
    pub const MISSING_INPUT: &str = "MISSING_REQUIRED_INPUT";
    /// Input value parse error (e.g., non-numeric string for u32).
    pub const INVALID_INPUT_VALUE: &str = "INVALID_INPUT_VALUE";
    /// Event payload file not found.
    pub const EVENT_NOT_FOUND: &str = "EVENT_PAYLOAD_NOT_FOUND";
    /// Event payload JSON parse error.
    pub const EVENT_PARSE_ERROR: &str = "EVENT_PARSE_ERROR";
    /// Unsupported event type.
    pub const UNSUPPORTED_EVENT: &str = "UNSUPPORTED_EVENT_TYPE";
    /// Action YAML file not found.
    pub const ACTION_YML_NOT_FOUND: &str = "ACTION_YML_NOT_FOUND";
    /// Action YAML parse error.
    pub const ACTION_YML_PARSE_ERROR: &str = "ACTION_YML_PARSE_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Action Input errors.
pub mod status_codes {
    pub const MISSING_INPUT: u16 = 400;
    pub const INVALID_INPUT_VALUE: u16 = 400;
    pub const EVENT_NOT_FOUND: u16 = 404;
    pub const EVENT_PARSE_ERROR: u16 = 400;
    pub const UNSUPPORTED_EVENT: u16 = 400;
    pub const ACTION_YML_NOT_FOUND: u16 = 404;
    pub const ACTION_YML_PARSE_ERROR: u16 = 400;
    pub const INTERNAL_ERROR: u16 = 500;
}
