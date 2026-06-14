//! HTTP API contracts for State Persistence endpoints.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
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

use crate::state_persistence::application::dto::ExecutionSummary;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All state persistence endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/state";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/state/executions
// ---------------------------------------------------------------------------

/// GET /api/v1/state/executions
///
/// List all available executions with their status and summary.
///
/// **Query Params:**
/// - `limit` (optional, default 50) — Maximum number of executions to return
/// - `offset` (optional, default 0) — Pagination offset
/// - `status` (optional) — Filter by execution status
///
/// **Response:** `200 OK` with `ListExecutionsResponse`
pub const LIST_EXECUTIONS_PATH: &str = "/api/v1/state/executions";
pub const LIST_EXECUTIONS_METHOD: &str = "GET";

/// Response body for GET /api/v1/state/executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListExecutionsResponse {
    pub executions: Vec<ExecutionSummary>,
    pub total_count: u32,
    pub limit: u32,
    pub offset: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/state/executions/{id}
// ---------------------------------------------------------------------------

/// GET /api/v1/state/executions/{id}
///
/// Get the full execution state for a specific execution.
///
/// **Path Param:** `id` — Execution UUID
/// **Response:** `200 OK` with `GetExecutionStateResponse`
pub const GET_EXECUTION_STATE_PATH: &str = "/api/v1/state/executions/{id}";
pub const GET_EXECUTION_STATE_METHOD: &str = "GET";

/// Response body for GET /api/v1/state/executions/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetExecutionStateResponse {
    pub execution_id: Uuid,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub node_count: u32,
    pub nodes: Vec<NodeStateResponse>,
    pub symbol_graph_hash: String,
}

/// A single node's state in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateResponse {
    pub node_id: Uuid,
    pub status: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub retries: u8,
    pub duration_ms: Option<u64>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/state/executions/{id}/nodes/{node_id}
// ---------------------------------------------------------------------------

/// GET /api/v1/state/executions/{id}/nodes/{node_id}
///
/// Get the state of a specific node within an execution.
///
/// **Path Params:**
/// - `id` — Execution UUID
/// - `node_id` — Node UUID
/// **Response:** `200 OK` with `NodeStateResponse`
pub const GET_NODE_STATE_PATH: &str = "/api/v1/state/executions/{id}/nodes/{node_id}";
pub const GET_NODE_STATE_METHOD: &str = "GET";

// ---------------------------------------------------------------------------
// Endpoint: DELETE /api/v1/state/executions/{id}
// ---------------------------------------------------------------------------

/// DELETE /api/v1/state/executions/{id}
///
/// Delete an execution state and its associated graph.
///
/// **Path Param:** `id` — Execution UUID
/// **Response:** `200 OK` with `DeleteExecutionResponse`
pub const DELETE_EXECUTION_PATH: &str = "/api/v1/state/executions/{id}";
pub const DELETE_EXECUTION_METHOD: &str = "DELETE";

/// Response body for DELETE /api/v1/state/executions/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteExecutionResponse {
    pub execution_id: Uuid,
    pub deleted: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/state/graphs
// ---------------------------------------------------------------------------

/// GET /api/v1/state/graphs
///
/// List persisted execution graphs (for TUI history view).
///
/// **Query Params:**
/// - `limit` (optional, default 20) — Maximum number of graphs to return
/// - `offset` (optional, default 0) — Pagination offset
///
/// **Response:** `200 OK` with `ListGraphsResponse`
pub const LIST_GRAPHS_PATH: &str = "/api/v1/state/graphs";
pub const LIST_GRAPHS_METHOD: &str = "GET";

/// Response body for GET /api/v1/state/graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGraphsResponse {
    pub graphs: Vec<GraphSummaryResponse>,
    pub total_count: u32,
    pub limit: u32,
    pub offset: u32,
}

/// Summary of a graph for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSummaryResponse {
    pub graph_id: Uuid,
    pub execution_id: Uuid,
    pub name: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_node_count: u32,
    pub completed_node_count: u32,
    pub failed_node_count: u32,
    pub skipped_node_count: u32,
    pub total_duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/state/graphs/{id}
// ---------------------------------------------------------------------------

/// GET /api/v1/state/graphs/{id}
///
/// Get the full execution graph for a specific graph record.
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `GetGraphResponse`
pub const GET_GRAPH_PATH: &str = "/api/v1/state/graphs/{id}";
pub const GET_GRAPH_METHOD: &str = "GET";

/// Response body for GET /api/v1/state/graphs/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGraphResponse {
    pub graph_id: Uuid,
    pub execution_id: Uuid,
    pub name: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub nodes: Vec<GraphNodeResponse>,
    pub total_node_count: u32,
    pub completed_node_count: u32,
    pub failed_node_count: u32,
    pub skipped_node_count: u32,
    pub total_duration_ms: u64,
}

/// A single node within a graph in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeResponse {
    pub node_id: Uuid,
    pub name: String,
    pub status: String,
    pub output_summary: Option<String>,
    pub error: Option<String>,
    pub retries: u8,
    pub duration_ms: Option<u64>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub dependencies: Vec<Uuid>,
    pub action_type: String,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/state/health
// ---------------------------------------------------------------------------

/// GET /api/v1/state/health
///
/// Health check for the state persistence system.
///
/// **Response:** `200 OK` with `HealthResponse`
pub const HEALTH_PATH: &str = "/api/v1/state/health";
pub const HEALTH_METHOD: &str = "GET";

/// Response body for GET /api/v1/state/health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub state_count: u64,
    pub graph_count: u64,
    pub state_dir: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all State Persistence API endpoints.
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

/// Standardized error codes for State Persistence API.
pub mod error_codes {
    /// Execution state not found.
    pub const STATE_NOT_FOUND: &str = "STATE_NOT_FOUND";
    /// Node not found within execution state.
    pub const NODE_NOT_FOUND: &str = "NODE_NOT_FOUND";
    /// Execution graph not found.
    pub const GRAPH_NOT_FOUND: &str = "GRAPH_NOT_FOUND";
    /// Corrupted state file encountered.
    pub const CORRUPTED_STATE: &str = "CORRUPTED_STATE";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "STATE_INTERNAL_ERROR";
}

/// HTTP status code mappings for State Persistence errors.
pub mod status_codes {
    pub const STATE_NOT_FOUND: u16 = 404;
    pub const NODE_NOT_FOUND: u16 = 404;
    pub const GRAPH_NOT_FOUND: u16 = 404;
    pub const CORRUPTED_STATE: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
