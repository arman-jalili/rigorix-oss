//! HTTP API contracts for Tool System endpoints.
//!
//! @canonical .pi/architecture/modules/tool-system.md#api
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #124
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
use std::collections::HashMap;

use crate::tools::application::dto::{
    ExecuteToolOutput, ListToolsOutput, RegisterToolOutput, ToolInfo,
};
use crate::tools::domain::error::ToolError;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All tool system endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/tools";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/tools
// ---------------------------------------------------------------------------

/// GET /api/v1/tools
///
/// List all registered tools with metadata.
///
/// **Response:** `200 OK` with `ListToolsResponse`
pub const LIST_TOOLS_PATH: &str = "/api/v1/tools";
pub const LIST_TOOLS_METHOD: &str = "GET";

/// Response for GET /api/v1/tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResponse {
    /// Metadata for each registered tool.
    pub tools: Vec<ToolInfo>,
    /// Total number of registered tools.
    pub total: usize,
}

impl From<ListToolsOutput> for ListToolsResponse {
    fn from(output: ListToolsOutput) -> Self {
        Self {
            tools: output.tools,
            total: output.total,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/tools/{name}
// ---------------------------------------------------------------------------

/// GET /api/v1/tools/{name}
///
/// Get metadata for a specific registered tool.
///
/// **Response:** `200 OK` with `GetToolResponse`
/// **Error:** `404 Not Found` with `ApiErrorResponse`
pub const GET_TOOL_PATH: &str = "/api/v1/tools/{name}";
pub const GET_TOOL_METHOD: &str = "GET";

/// Response for GET /api/v1/tools/{name}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetToolResponse {
    /// Tool metadata.
    pub tool: ToolInfo,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/tools/execute
// ---------------------------------------------------------------------------

/// POST /api/v1/tools/execute
///
/// Execute a registered tool with the given parameters.
/// Risk gating is applied before execution.
///
/// **Request:** `ExecuteToolRequest`
/// **Response:** `200 OK` with `ExecuteToolResponse`
/// **Error:** `404 Not Found` if tool is not registered
/// **Error:** `403 Forbidden` if tool requires confirmation or path is denied
/// **Error:** `400 Bad Request` if input parameters are invalid
pub const EXECUTE_TOOL_PATH: &str = "/api/v1/tools/execute";
pub const EXECUTE_TOOL_METHOD: &str = "POST";

/// Request body for POST /api/v1/tools/execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolRequest {
    /// Name of the tool to execute.
    pub tool_name: String,

    /// Tool-specific parameters.
    pub params: HashMap<String, serde_json::Value>,

    /// Execution ID for tracing. If not provided, one is generated.
    pub execution_id: Option<uuid::Uuid>,

    /// Whether to execute in dry-run mode (preview, no side effects).
    #[serde(default)]
    pub dry_run: bool,
}

/// Response for POST /api/v1/tools/execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolResponse {
    /// Whether execution was successful.
    pub success: bool,

    /// The tool name that was executed.
    pub tool_name: String,

    /// Execution result.
    pub result: ToolExecutionResult,

    /// Risk level that was applied for gating.
    pub risk_level: String,

    /// Whether this was a dry-run execution.
    pub dry_run: bool,
}

impl From<ExecuteToolOutput> for ExecuteToolResponse {
    fn from(output: ExecuteToolOutput) -> Self {
        let result = &output.result;
        Self {
            success: result.is_success(),
            tool_name: String::new(), // Filled in by handler from request
            result: ToolExecutionResult {
                output: result.output.clone(),
                exit_code: result.exit_code,
                side_effects: result
                    .side_effects
                    .iter()
                    .map(|se| SideEffectInfo {
                        path: se.path.clone(),
                        effect_type: se.effect_type.clone(),
                        description: se.description.clone(),
                    })
                    .collect(),
                duration_ms: result.duration_ms,
            },
            risk_level: format!("{:?}", output.risk_level).to_lowercase(),
            dry_run: output.dry_run,
        }
    }
}

/// Tool execution result in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// Text output from the tool.
    pub output: String,

    /// Exit code.
    pub exit_code: i32,

    /// Side effects produced.
    pub side_effects: Vec<SideEffectInfo>,

    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Side effect info for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffectInfo {
    /// Affected path.
    pub path: String,
    /// Type of side effect.
    pub effect_type: String,
    /// Description of the side effect.
    pub description: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/tools/register
// ---------------------------------------------------------------------------

/// POST /api/v1/tools/register
///
/// Register a new tool in the ToolRegistry.
/// The tool implementation must be available in the runtime.
///
/// **Request:** `RegisterToolRequest`
/// **Response:** `201 Created` with `RegisterToolResponse`
/// **Error:** `409 Conflict` if tool with the same name already exists
pub const REGISTER_TOOL_PATH: &str = "/api/v1/tools/register";
pub const REGISTER_TOOL_METHOD: &str = "POST";

/// Request body for POST /api/v1/tools/register.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterToolRequest {
    /// Unique tool name (kebab-case).
    pub name: String,

    /// Optional display name.
    pub display_name: Option<String>,

    /// Optional description.
    pub description: Option<String>,

    /// Whether to replace an existing tool with the same name.
    #[serde(default)]
    pub replace: bool,
}

/// Response for POST /api/v1/tools/register.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterToolResponse {
    pub success: bool,
    pub name: String,
    pub replaced: bool,
    pub total_tools: usize,
}

impl From<RegisterToolOutput> for RegisterToolResponse {
    fn from(output: RegisterToolOutput) -> Self {
        Self {
            success: true,
            name: output.name,
            replaced: output.replaced,
            total_tools: output.total_tools,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/tools/{name}/history
// ---------------------------------------------------------------------------

/// GET /api/v1/tools/{name}/history
///
/// Get execution history for a specific tool.
///
/// **Query Parameters:**
/// - `limit` (optional, default: 10): Maximum number of records to return.
///
/// **Response:** `200 OK` with `ToolHistoryResponse`
/// **Error:** `404 Not Found` if tool is not registered
pub const TOOL_HISTORY_PATH: &str = "/api/v1/tools/{name}/history";
pub const TOOL_HISTORY_METHOD: &str = "GET";

/// Response for GET /api/v1/tools/{name}/history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHistoryResponse {
    /// Tool name.
    pub tool_name: String,
    /// Recent execution records.
    pub history: Vec<ToolExecutionResult>,
    /// Total records available.
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Tool System API endpoints.
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

impl From<ToolError> for ApiErrorResponse {
    fn from(err: ToolError) -> Self {
        let details = match &err {
            ToolError::InvalidInput(msg) => Some(serde_json::json!({ "detail": msg })),
            ToolError::ExecutionFailed(msg) => Some(serde_json::json!({ "detail": msg })),
            ToolError::NotFound(msg) => Some(serde_json::json!({ "detail": msg })),
            ToolError::PathDenied(msg) => Some(serde_json::json!({ "detail": msg })),
            ToolError::RequiresConfirmation => None,
        };

        Self {
            status: err.http_status(),
            code: err.error_code().to_string(),
            message: err.to_string(),
            details,
            request_id: None,
        }
    }
}

/// Standardized error codes for Tool System API.
pub mod error_codes {
    /// Tool not found.
    pub const NOT_FOUND: &str = "TOOL_NOT_FOUND";
    /// Invalid tool input parameters.
    pub const INVALID_INPUT: &str = "TOOL_INVALID_INPUT";
    /// Tool execution failed at runtime.
    pub const EXECUTION_FAILED: &str = "TOOL_EXECUTION_FAILED";
    /// Tool path was denied by security policy.
    pub const PATH_DENIED: &str = "TOOL_PATH_DENIED";
    /// Tool requires user confirmation.
    pub const REQUIRES_CONFIRMATION: &str = "TOOL_REQUIRES_CONFIRMATION";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Tool System errors.
pub mod status_codes {
    pub const NOT_FOUND: u16 = 404;
    pub const INVALID_INPUT: u16 = 400;
    pub const EXECUTION_FAILED: u16 = 500;
    pub const PATH_DENIED: u16 = 403;
    pub const REQUIRES_CONFIRMATION: u16 = 403;
    pub const INTERNAL_ERROR: u16 = 500;
}
