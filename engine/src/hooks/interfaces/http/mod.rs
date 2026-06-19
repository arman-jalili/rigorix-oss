//! HTTP API contracts for Hook System endpoints.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #410
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

use crate::hooks::domain::event::HookEvent;
use crate::hooks::domain::protocol::{HookDecision, HookPermissionOverride};
use crate::hooks::domain::result::HookRunResult;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All hook system endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/hooks";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/hooks/run
// ---------------------------------------------------------------------------

/// POST /api/v1/hooks/run
///
/// Run all hooks for a given lifecycle event.
///
/// **Request:** `RunHooksRequest`
/// **Response:** `200 OK` with `RunHooksResponse`
pub const RUN_HOOKS_PATH: &str = "/api/v1/hooks/run";
pub const RUN_HOOKS_METHOD: &str = "POST";

/// Request body for POST /api/v1/hooks/run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHooksRequest {
    /// The lifecycle event to run hooks for.
    pub event: HookEvent,

    /// Name of the tool being intercepted.
    pub tool_name: String,

    /// The tool input as a JSON value.
    pub tool_input: serde_json::Value,

    /// The tool output or error output (for Post* events).
    /// Leave empty for PreToolUse.
    #[serde(default)]
    pub tool_output: String,

    /// The session/execution ID for correlation.
    pub session_id: String,

    /// The workspace root directory path.
    pub workspace_root: String,
}

/// Response body for POST /api/v1/hooks/run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHooksResponse {
    pub success: bool,

    /// The lifecycle event that was processed.
    pub event: HookEvent,

    /// Whether the tool execution is allowed to proceed.
    pub allowed: bool,

    /// Whether the execution was denied by a hook.
    pub denied: bool,

    /// Whether a hook execution failed.
    pub failed: bool,

    /// Whether the execution was cancelled.
    pub cancelled: bool,

    /// Aggregated feedback messages from hooks.
    #[serde(default)]
    pub messages: Vec<String>,

    /// Permission override from hooks (if any).
    #[serde(default)]
    pub permission_override: Option<HookPermissionOverride>,

    /// Updated tool input from hooks (if any).
    #[serde(default)]
    pub updated_input: Option<serde_json::Value>,

    /// Number of hooks that were executed.
    pub hooks_executed: usize,

    /// Number of hooks that failed.
    pub hooks_failed: usize,
}

impl From<(HookRunResult, usize, usize)> for RunHooksResponse {
    fn from((result, executed, failed): (HookRunResult, usize, usize)) -> Self {
        Self {
            success: !result.failed,
            event: result.event,
            allowed: result.is_allowed(),
            denied: result.is_denied(),
            failed: result.failed,
            cancelled: result.is_cancelled(),
            messages: result.messages,
            permission_override: result.permission_override,
            updated_input: result.updated_input,
            hooks_executed: executed,
            hooks_failed: failed,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/hooks/config
// ---------------------------------------------------------------------------

/// GET /api/v1/hooks/config
///
/// Get the current hook configuration (registered commands).
///
/// **Response:** `200 OK` with `HookConfigResponse`
pub const GET_HOOKS_CONFIG_PATH: &str = "/api/v1/hooks/config";
pub const GET_HOOKS_CONFIG_METHOD: &str = "GET";

/// Response body for GET /api/v1/hooks/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfigResponse {
    pub success: bool,
    pub pre_tool_use: Vec<String>,
    pub post_tool_use: Vec<String>,
    pub post_tool_use_failure: Vec<String>,
    pub timeout_secs: u64,
    pub total_hooks: usize,
}

// ---------------------------------------------------------------------------
// Endpoint: PUT /api/v1/hooks/config
// ---------------------------------------------------------------------------

/// PUT /api/v1/hooks/config
///
/// Update the hook configuration at runtime.
///
/// **Request:** `UpdateHookConfigRequest`
/// **Response:** `200 OK` with `HookConfigResponse`
pub const UPDATE_HOOKS_CONFIG_PATH: &str = "/api/v1/hooks/config";
pub const UPDATE_HOOKS_CONFIG_METHOD: &str = "PUT";

/// Request body for PUT /api/v1/hooks/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateHookConfigRequest {
    /// Commands to run before every tool execution.
    #[serde(default)]
    pub pre_tool_use: Vec<String>,

    /// Commands to run after every successful tool execution.
    #[serde(default)]
    pub post_tool_use: Vec<String>,

    /// Commands to run after every failed tool execution.
    #[serde(default)]
    pub post_tool_use_failure: Vec<String>,

    /// Timeout in seconds for each hook command (optional).
    pub timeout_secs: Option<u64>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/hooks/test
// ---------------------------------------------------------------------------

/// POST /api/v1/hooks/test
///
/// Test-run a single hook command without affecting the real hook pipeline.
/// Useful for debugging hook scripts.
///
/// **Request:** `TestHookRequest`
/// **Response:** `200 OK` with `TestHookResponse`
pub const TEST_HOOK_PATH: &str = "/api/v1/hooks/test";
pub const TEST_HOOK_METHOD: &str = "POST";

/// Request body for POST /api/v1/hooks/test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestHookRequest {
    /// The command to test-run.
    pub command: String,

    /// The lifecycle event to simulate.
    #[serde(default)]
    pub event: HookEvent,

    /// Name of the tool to simulate.
    pub tool_name: String,

    /// Tool input to pass to the hook.
    pub tool_input: serde_json::Value,
}

/// Response body for POST /api/v1/hooks/test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestHookResponse {
    pub success: bool,
    pub decision: HookDecision,
    pub messages: Vec<String>,
    pub duration_ms: u64,
    pub raw_output: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Hook System API endpoints.
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

/// Standardized error codes for Hook System API.
pub mod error_codes {
    /// Hook command not found.
    pub const COMMAND_NOT_FOUND: &str = "HOOK_COMMAND_NOT_FOUND";
    /// Hook execution timed out.
    pub const TIMEOUT: &str = "HOOK_TIMEOUT";
    /// Hook returned invalid JSON.
    pub const INVALID_JSON: &str = "HOOK_INVALID_JSON";
    /// Hook process exited with error.
    pub const PROCESS_ERROR: &str = "HOOK_PROCESS_ERROR";
    /// Hook execution was aborted.
    pub const ABORTED: &str = "HOOK_ABORTED";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "HOOK_INTERNAL_ERROR";
}

/// HTTP status code mappings for Hook System errors.
pub mod status_codes {
    pub const COMMAND_NOT_FOUND: u16 = 404;
    pub const TIMEOUT: u16 = 504;
    pub const INVALID_JSON: u16 = 422;
    pub const PROCESS_ERROR: u16 = 502;
    pub const ABORTED: u16 = 499;
    pub const INTERNAL_ERROR: u16 = 500;
}
