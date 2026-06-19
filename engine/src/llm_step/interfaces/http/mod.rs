//! HTTP API contracts for LLM Step endpoints.
//!
//! @canonical .pi/architecture/modules/llm-step.md
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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::llm_step::application::dto::LlmStepSummary;
use crate::llm_step::domain::LlmGenerationOutput;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All LLM Step endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/llm-step";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/llm-step/nodes
// ---------------------------------------------------------------------------

/// POST /api/v1/llm-step/nodes
///
/// Create a new LlmGenerateNode with the given configuration.
///
/// **Request Body:** `CreateNodeRequest`
/// **Response:** `201 Created` with `NodeResponse`
pub const CREATE_NODE_PATH: &str = "/api/v1/llm-step/nodes";
pub const CREATE_NODE_METHOD: &str = "POST";

/// Request body for POST /api/v1/llm-step/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNodeRequest {
    /// Human-readable name for the generation node.
    pub name: String,

    /// The LLM model configuration.
    pub model_config: LlmModelConfigRequest,

    /// The prompt template with placeholders for context.
    ///
    /// Supported placeholders: `{source_code}`, `{error_context}`,
    /// `{execution_context}`, `{symbol_definitions}`
    pub prompt_template: String,

    /// The expected output schema.
    pub output_schema: OutputSchemaRequest,
}

/// LLM model configuration for API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelConfigRequest {
    /// The LLM provider (e.g., "anthropic", "openai").
    pub provider: String,

    /// The model identifier.
    pub model: String,

    /// Maximum tokens for the response (default: 4096).
    pub max_tokens: Option<u32>,

    /// Temperature for generation (default: 0.7).
    pub temperature: Option<f64>,

    /// Top-p sampling parameter (default: 0.9).
    pub top_p: Option<f64>,

    /// Request timeout in seconds (default: 120).
    pub timeout_secs: Option<u64>,

    /// Maximum retries for transient errors (default: 3).
    pub max_retries: Option<u8>,
}

/// Output schema configuration for API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSchemaRequest {
    /// The output format (text, json, code, markdown).
    pub format: String,

    /// A JSON Schema or description of the expected output.
    pub schema: String,

    /// Whether the schema is required (strict mode).
    pub strict: bool,
}

/// Response body for POST /api/v1/llm-step/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResponse {
    pub node_id: Uuid,
    pub name: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/llm-step/nodes/{id}
// ---------------------------------------------------------------------------

/// GET /api/v1/llm-step/nodes/{id}
///
/// Retrieve an LlmGenerateNode by its ID.
///
/// **Path Param:** `id` — Node UUID
/// **Response:** `200 OK` with `GetNodeResponse`
pub const GET_NODE_PATH: &str = "/api/v1/llm-step/nodes/{id}";
pub const GET_NODE_METHOD: &str = "GET";

/// Response body for GET /api/v1/llm-step/nodes/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeResponse {
    pub node_id: Uuid,
    pub name: String,
    pub state: String,
    pub model: String,
    pub provider: String,
    pub prompt_template: String,
    pub output: Option<LlmGenerationOutput>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/llm-step/nodes/{id}/build-context
// ---------------------------------------------------------------------------

/// POST /api/v1/llm-step/nodes/{id}/build-context
///
/// Assemble context for a generation node.
///
/// **Path Param:** `id` — Node UUID
/// **Request Body:** `BuildContextRequest`
/// **Response:** `200 OK` with `BuildContextResponse`
pub const BUILD_CONTEXT_PATH: &str = "/api/v1/llm-step/nodes/{id}/build-context";
pub const BUILD_CONTEXT_METHOD: &str = "POST";

/// Request body for POST /api/v1/llm-step/nodes/{id}/build-context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextRequest {
    /// The execution ID this node belongs to.
    pub execution_id: Uuid,

    /// The DAG ID for execution context.
    pub dag_id: Uuid,

    /// Target file path for the generated code (if known).
    pub target_file_path: Option<String>,

    /// Source file paths to include in context.
    pub source_file_paths: Vec<String>,

    /// Whether to include failure context.
    pub include_failure_context: bool,
}

/// Response body for POST /api/v1/llm-step/nodes/{id}/build-context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextResponse {
    pub node_id: Uuid,
    pub context_summary: ContextSummary,
    pub assembled_at: DateTime<Utc>,
}

/// Summary of the assembled context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub source_file_count: u32,
    pub symbol_count: u32,
    pub has_failure_context: bool,
    pub prompt_length: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/llm-step/nodes/{id}/execute
// ---------------------------------------------------------------------------

/// POST /api/v1/llm-step/nodes/{id}/execute
///
/// Execute a full LLM step: build context → generate → parse.
///
/// **Path Param:** `id` — Node UUID
/// **Request Body:** `ExecuteStepRequest`
/// **Response:** `200 OK` with `ExecuteStepResponse`
pub const EXECUTE_STEP_PATH: &str = "/api/v1/llm-step/nodes/{id}/execute";
pub const EXECUTE_STEP_METHOD: &str = "POST";

/// Request body for POST /api/v1/llm-step/nodes/{id}/execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteStepRequest {
    /// The execution ID this node belongs to.
    pub execution_id: Uuid,

    /// The DAG ID for execution context.
    pub dag_id: Uuid,

    /// Target file path for the generated code (if known).
    pub target_file_path: Option<String>,

    /// Source file paths to include in context.
    pub source_file_paths: Vec<String>,

    /// Whether to include failure context.
    pub include_failure_context: bool,

    /// The LLM API key for authentication.
    ///
    /// This is a sensitive field. It must never be logged.
    /// For server-side implementations, the API key should be
    /// injected from the server configuration instead.
    pub api_key: String,
}

/// Response body for POST /api/v1/llm-step/nodes/{id}/execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteStepResponse {
    pub node_id: Uuid,
    pub output: LlmGenerationOutput,
    pub total_duration_ms: u64,
    pub context_duration_ms: u64,
    pub generation_duration_ms: u64,
    pub total_tokens_used: u32,
    pub completed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/llm-step/nodes/{id}/retry
// ---------------------------------------------------------------------------

/// POST /api/v1/llm-step/nodes/{id}/retry
///
/// Retry a failed generation with updated failure context.
///
/// **Path Param:** `id` — Node UUID
/// **Request Body:** `RetryStepRequest`
/// **Response:** `200 OK` with `RetryStepResponse`
pub const RETRY_STEP_PATH: &str = "/api/v1/llm-step/nodes/{id}/retry";
pub const RETRY_STEP_METHOD: &str = "POST";

/// Request body for POST /api/v1/llm-step/nodes/{id}/retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryStepRequest {
    /// The retry attempt number (1-indexed).
    pub attempt: u8,

    /// The updated failure context.
    pub error_message: String,
    pub error_output: String,

    /// The LLM API key.
    pub api_key: String,
}

/// Response body for POST /api/v1/llm-step/nodes/{id}/retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryStepResponse {
    pub node_id: Uuid,
    pub attempt: u8,
    pub output: LlmGenerationOutput,
    pub duration_ms: u64,
    pub generated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/llm-step/nodes
// ---------------------------------------------------------------------------

/// GET /api/v1/llm-step/nodes
///
/// List all LLM generation nodes with summary information.
///
/// **Query Params:**
/// - `limit` (optional, default 50) — Maximum number of nodes to return
/// - `offset` (optional, default 0) — Pagination offset
/// - `execution_id` (optional) — Filter by execution ID
///
/// **Response:** `200 OK` with `ListNodesResponse`
pub const LIST_NODES_PATH: &str = "/api/v1/llm-step/nodes";
pub const LIST_NODES_METHOD: &str = "GET";

/// Response body for GET /api/v1/llm-step/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<LlmStepSummary>,
    pub total_count: u32,
    pub limit: u32,
    pub offset: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: DELETE /api/v1/llm-step/nodes/{id}
// ---------------------------------------------------------------------------

/// DELETE /api/v1/llm-step/nodes/{id}
///
/// Delete an LlmGenerateNode.
///
/// **Path Param:** `id` — Node UUID
/// **Response:** `200 OK` with `DeleteNodeResponse`
pub const DELETE_NODE_PATH: &str = "/api/v1/llm-step/nodes/{id}";
pub const DELETE_NODE_METHOD: &str = "DELETE";

/// Response body for DELETE /api/v1/llm-step/nodes/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteNodeResponse {
    pub node_id: Uuid,
    pub deleted: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/llm-step/validate
// ---------------------------------------------------------------------------

/// POST /api/v1/llm-step/validate
///
/// Validate an LlmGenerateNode configuration without creating it.
///
/// **Request Body:** `CreateNodeRequest`
/// **Response:** `200 OK` with `ValidateConfigResponse`
pub const VALIDATE_CONFIG_PATH: &str = "/api/v1/llm-step/validate";
pub const VALIDATE_CONFIG_METHOD: &str = "POST";

/// Response body for POST /api/v1/llm-step/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigResponse {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub estimated_token_cost: Option<u32>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/llm-step/health
// ---------------------------------------------------------------------------

/// GET /api/v1/llm-step/health
///
/// Health check for the LLM Step system.
///
/// **Response:** `200 OK` with `HealthResponse`
pub const HEALTH_PATH: &str = "/api/v1/llm-step/health";
pub const HEALTH_METHOD: &str = "GET";

/// Response body for GET /api/v1/llm-step/health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub node_count: u64,
    pub default_provider: String,
    pub default_model: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all LLM Step API endpoints.
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

/// Standardized error codes for LLM Step API.
pub mod error_codes {
    /// Node not found.
    pub const NODE_NOT_FOUND: &str = "LLM_STEP_NODE_NOT_FOUND";
    /// Invalid node configuration.
    pub const INVALID_CONFIGURATION: &str = "LLM_STEP_INVALID_CONFIGURATION";
    /// LLM provider error (network, auth, rate limit).
    pub const PROVIDER_ERROR: &str = "LLM_STEP_PROVIDER_ERROR";
    /// Response parse error.
    pub const PARSE_ERROR: &str = "LLM_STEP_PARSE_ERROR";
    /// Context build failure.
    pub const CONTEXT_BUILD_FAILED: &str = "LLM_STEP_CONTEXT_BUILD_FAILED";
    /// Token budget exceeded.
    pub const TOKEN_BUDGET_EXCEEDED: &str = "LLM_STEP_TOKEN_BUDGET_EXCEEDED";
    /// Generation timed out.
    pub const TIMEOUT: &str = "LLM_STEP_TIMEOUT";
    /// Unsupported model.
    pub const UNSUPPORTED_MODEL: &str = "LLM_STEP_UNSUPPORTED_MODEL";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "LLM_STEP_INTERNAL_ERROR";
}

/// HTTP status code mappings for LLM Step errors.
pub mod status_codes {
    pub const NODE_NOT_FOUND: u16 = 404;
    pub const INVALID_CONFIGURATION: u16 = 400;
    pub const PROVIDER_ERROR: u16 = 502;
    pub const PARSE_ERROR: u16 = 422;
    pub const CONTEXT_BUILD_FAILED: u16 = 500;
    pub const TOKEN_BUDGET_EXCEEDED: u16 = 429;
    pub const TIMEOUT: u16 = 504;
    pub const UNSUPPORTED_MODEL: u16 = 400;
    pub const INTERNAL_ERROR: u16 = 500;
}
