//! HTTP API contracts for Execution Engine endpoints.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
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

use crate::execution_engine::domain::NodeStatus;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All Execution Engine endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/execution";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/execution/graphs/{id}/execute
// ---------------------------------------------------------------------------

/// POST /api/v1/execution/graphs/{id}/execute
///
/// Execute a sealed TaskGraph. Returns immediately with the execution ID;
/// callers poll GET /execution/graphs/{dag_id}/state to track progress.
///
/// **Path Param:** `id` — Graph UUID
/// **Request Body:** `ExecuteRequest`
/// **Response:** `202 Accepted` with `ExecuteResponse`
pub const EXECUTE_GRAPH_PATH: &str = "/api/v1/execution/graphs/{id}/execute";
pub const EXECUTE_GRAPH_METHOD: &str = "POST";

/// Request body for POST /api/v1/execution/graphs/{id}/execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// Optional concurrency limit override (default from config).
    pub max_concurrent: Option<u32>,
    /// Optional maximum failures before abort (default from config).
    pub max_failures: Option<u32>,
    /// Whether to block until execution completes (default: false).
    pub wait_for_completion: Option<bool>,
}

/// Response body for POST /api/v1/execution/graphs/{id}/execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub dag_id: Uuid,
    pub execution_id: Uuid,
    pub status: String,
    pub started_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/execution/graphs/{dag_id}/state
// ---------------------------------------------------------------------------

/// GET /api/v1/execution/graphs/{dag_id}/state
///
/// Get the current execution state of a DAG execution.
///
/// **Path Param:** `dag_id` — DAG execution UUID
/// **Response:** `200 OK` with `ExecutionStateResponse`
pub const EXECUTION_STATE_PATH: &str = "/api/v1/execution/graphs/{dag_id}/state";
pub const EXECUTION_STATE_METHOD: &str = "GET";

/// Response body for GET /api/v1/execution/graphs/{dag_id}/state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStateResponse {
    pub dag_id: Uuid,
    pub status: ExecutionStatusResponse,
    pub completed_count: u32,
    pub failed_count: u32,
    pub skipped_count: u32,
    pub total_nodes: u32,
    pub total_retries: u32,
    pub total_duration_ms: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub paused: bool,
}

/// Execution status for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatusResponse {
    /// Execution is in progress.
    Running,
    /// Execution has completed successfully.
    Completed,
    /// Execution completed with failures.
    CompletedWithFailures,
    /// Execution was cancelled.
    Cancelled,
    /// Execution was aborted.
    Aborted,
    /// Execution is paused.
    Paused,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/execution/graphs/{dag_id}/nodes
// ---------------------------------------------------------------------------

/// GET /api/v1/execution/graphs/{dag_id}/nodes
///
/// Get execution state for all nodes in a DAG execution.
///
/// **Query Params:**
/// - `status` (optional) — Filter by node status
///
/// **Path Param:** `dag_id` — DAG execution UUID
/// **Response:** `200 OK` with `NodeStatesResponse`
pub const NODE_STATES_PATH: &str = "/api/v1/execution/graphs/{dag_id}/nodes";
pub const NODE_STATES_METHOD: &str = "GET";

/// Response body for GET /api/v1/execution/graphs/{dag_id}/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatesResponse {
    pub dag_id: Uuid,
    pub nodes: Vec<NodeStateResponse>,
}

/// Individual node state for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateResponse {
    pub node_id: Uuid,
    pub node_name: String,
    pub status: NodeStatus,
    pub retry_attempts: u8,
    pub last_duration_ms: Option<u64>,
    pub last_error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/execution/graphs/{dag_id}/pause
// ---------------------------------------------------------------------------

/// POST /api/v1/execution/graphs/{dag_id}/pause
///
/// Pause an in-flight execution.
///
/// **Path Param:** `dag_id` — DAG execution UUID
/// **Response:** `200 OK` with `PauseResponse`
pub const PAUSE_EXECUTION_PATH: &str = "/api/v1/execution/graphs/{dag_id}/pause";
pub const PAUSE_EXECUTION_METHOD: &str = "POST";

/// Response body for POST /api/v1/execution/graphs/{dag_id}/pause.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseResponse {
    pub dag_id: Uuid,
    pub paused: bool,
    pub paused_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/execution/graphs/{dag_id}/resume
// ---------------------------------------------------------------------------

/// POST /api/v1/execution/graphs/{dag_id}/resume
///
/// Resume a paused execution.
///
/// **Path Param:** `dag_id` — DAG execution UUID
/// **Response:** `200 OK` with `ResumeResponse`
pub const RESUME_EXECUTION_PATH: &str = "/api/v1/execution/graphs/{dag_id}/resume";
pub const RESUME_EXECUTION_METHOD: &str = "POST";

/// Response body for POST /api/v1/execution/graphs/{dag_id}/resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResponse {
    pub dag_id: Uuid,
    pub resumed: bool,
    pub resumed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/execution/graphs/{dag_id}/abort
// ---------------------------------------------------------------------------

/// POST /api/v1/execution/graphs/{dag_id}/abort
///
/// Abort an in-flight execution.
///
/// **Request Body:** `AbortRequest`
/// **Path Param:** `dag_id` — DAG execution UUID
/// **Response:** `200 OK` with `AbortResponse`
pub const ABORT_EXECUTION_PATH: &str = "/api/v1/execution/graphs/{dag_id}/abort";
pub const ABORT_EXECUTION_METHOD: &str = "POST";

/// Request body for POST /api/v1/execution/graphs/{dag_id}/abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortRequest {
    /// Reason for aborting the execution.
    pub reason: String,
}

/// Response body for POST /api/v1/execution/graphs/{dag_id}/abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortResponse {
    pub dag_id: Uuid,
    pub aborted: bool,
    pub completed_count: u32,
    pub skipped_count: u32,
    pub aborted_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/execution/history
// ---------------------------------------------------------------------------

/// GET /api/v1/execution/history
///
/// List execution history.
///
/// **Query Params:**
/// - `limit` (optional, default 50) — Maximum number of executions to return
/// - `offset` (optional, default 0) — Pagination offset
///
/// **Response:** `200 OK` with `ExecutionHistoryResponse`
pub const EXECUTION_HISTORY_PATH: &str = "/api/v1/execution/history";
pub const EXECUTION_HISTORY_METHOD: &str = "GET";

/// Response body for GET /api/v1/execution/history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistoryResponse {
    pub executions: Vec<ExecutionHistoryItem>,
    pub total_count: u32,
    pub limit: u32,
    pub offset: u32,
}

/// Individual execution history item for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistoryItem {
    pub dag_id: Uuid,
    pub total_nodes: u32,
    pub completed_count: u32,
    pub failed_count: u32,
    pub skipped_count: u32,
    pub total_duration_ms: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatusResponse,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/execution/graphs/{dag_id}/result
// ---------------------------------------------------------------------------

/// GET /api/v1/execution/graphs/{dag_id}/result
///
/// Get the complete execution result for a completed execution.
///
/// **Path Param:** `dag_id` — DAG execution UUID
/// **Response:** `200 OK` with complete execution result data
pub const EXECUTION_RESULT_PATH: &str = "/api/v1/execution/graphs/{dag_id}/result";
pub const EXECUTION_RESULT_METHOD: &str = "GET";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/execution/health
// ---------------------------------------------------------------------------

/// GET /api/v1/execution/health
///
/// Health check for the Execution Engine.
///
/// **Response:** `200 OK` with `HealthResponse`
pub const HEALTH_PATH: &str = "/api/v1/execution/health";
pub const HEALTH_METHOD: &str = "GET";

/// Response body for GET /api/v1/execution/health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub active_executions: u32,
    pub completed_executions: u64,
    pub total_retries: u64,
    pub max_concurrent: u32,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Execution Engine API endpoints.
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

/// Standardized error codes for Execution Engine API.
pub mod error_codes {
    /// Execution not found.
    pub const EXECUTION_NOT_FOUND: &str = "EXEC_NOT_FOUND";
    /// Graph not sealed for execution.
    pub const GRAPH_NOT_SEALED: &str = "EXEC_GRAPH_NOT_SEALED";
    /// Execution already in progress.
    pub const ALREADY_RUNNING: &str = "EXEC_ALREADY_RUNNING";
    /// Execution already completed.
    pub const ALREADY_COMPLETED: &str = "EXEC_ALREADY_COMPLETED";
    /// Execution not in progress.
    pub const NOT_RUNNING: &str = "EXEC_NOT_RUNNING";
    /// Execution was cancelled.
    pub const EXECUTION_CANCELLED: &str = "EXEC_CANCELLED";
    /// Execution was aborted.
    pub const EXECUTION_ABORTED: &str = "EXEC_ABORTED";
    /// Enforcement limit exceeded.
    pub const ENFORCEMENT_LIMIT: &str = "EXEC_ENFORCEMENT_LIMIT";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "EXEC_INTERNAL_ERROR";
}

/// HTTP status code mappings for Execution Engine errors.
pub mod status_codes {
    pub const EXECUTION_NOT_FOUND: u16 = 404;
    pub const GRAPH_NOT_SEALED: u16 = 400;
    pub const ALREADY_RUNNING: u16 = 409;
    pub const ALREADY_COMPLETED: u16 = 409;
    pub const NOT_RUNNING: u16 = 400;
    pub const EXECUTION_CANCELLED: u16 = 200;
    pub const EXECUTION_ABORTED: u16 = 200;
    pub const ENFORCEMENT_LIMIT: u16 = 429;
    pub const INTERNAL_ERROR: u16 = 500;
}
