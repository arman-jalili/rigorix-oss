//! HTTP API contracts for DAG Engine endpoints.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
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

use crate::dag_engine::application::dto::PlanSummary;
use crate::dag_engine::domain::graph::TaskNode;
use crate::dag_engine::domain::plan::NodeDiff;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All DAG Engine endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/dag";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/dag/graphs
// ---------------------------------------------------------------------------

/// POST /api/v1/dag/graphs
///
/// Construct a new TaskGraph from a list of nodes.
///
/// **Request Body:** `ConstructGraphRequest`
/// **Response:** `201 Created` with `GraphResponse`
pub const CONSTRUCT_GRAPH_PATH: &str = "/api/v1/dag/graphs";
pub const CONSTRUCT_GRAPH_METHOD: &str = "POST";

/// Request body for POST /api/v1/dag/graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructGraphRequest {
    /// The nodes to add to the graph.
    pub nodes: Vec<TaskNode>,
}

/// Response body for POST /api/v1/dag/graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphResponse {
    pub dag_id: Uuid,
    pub node_count: u32,
    pub sealed: bool,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/dag/graphs/{id}/seal
// ---------------------------------------------------------------------------

/// POST /api/v1/dag/graphs/{id}/seal
///
/// Seal a graph and run topological sort with cycle detection.
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `SealGraphResponse`
pub const SEAL_GRAPH_PATH: &str = "/api/v1/dag/graphs/{id}/seal";
pub const SEAL_GRAPH_METHOD: &str = "POST";

/// Response body for POST /api/v1/dag/graphs/{id}/seal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealGraphResponse {
    pub dag_id: Uuid,
    pub topological_order: Vec<Uuid>,
    pub processed_count: u32,
    pub total_nodes: u32,
    pub sealed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/dag/graphs/{id}
// ---------------------------------------------------------------------------

/// GET /api/v1/dag/graphs/{id}
///
/// Retrieve a TaskGraph by its ID.
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `GetGraphResponse`
pub const GET_GRAPH_PATH: &str = "/api/v1/dag/graphs/{id}";
pub const GET_GRAPH_METHOD: &str = "GET";

/// Response body for GET /api/v1/dag/graphs/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGraphResponse {
    pub dag_id: Uuid,
    pub nodes: Vec<TaskNode>,
    pub topological_order: Option<Vec<Uuid>>,
    pub sealed: bool,
    pub node_count: u32,
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/dag/graphs/{id}/nodes
// ---------------------------------------------------------------------------

/// GET /api/v1/dag/graphs/{id}/nodes
///
/// List all nodes in a graph.
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `ListNodesResponse`
pub const LIST_NODES_PATH: &str = "/api/v1/dag/graphs/{id}/nodes";
pub const LIST_NODES_METHOD: &str = "GET";

/// Response body for GET /api/v1/dag/graphs/{id}/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<TaskNode>,
    pub total_count: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/dag/graphs/{id}/ready-nodes
// ---------------------------------------------------------------------------

/// GET /api/v1/dag/graphs/{id}/ready-nodes
///
/// Get nodes whose dependencies are all satisfied (ready for execution).
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `ReadyNodesResponse`
pub const READY_NODES_PATH: &str = "/api/v1/dag/graphs/{id}/ready-nodes";
pub const READY_NODES_METHOD: &str = "GET";

/// Response body for GET /api/v1/dag/graphs/{id}/ready-nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyNodesResponse {
    pub dag_id: Uuid,
    pub ready_node_ids: Vec<Uuid>,
    pub ready_count: u32,
    pub total_pending: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/dag/graphs/{id}/nodes/{node_id}/complete
// ---------------------------------------------------------------------------

/// POST /api/v1/dag/graphs/{id}/nodes/{node_id}/complete
///
/// Mark a node as completed during execution.
///
/// **Path Params:**
/// - `id` — Graph UUID
/// - `node_id` — Node UUID
///
/// **Response:** `200 OK`
pub const MARK_COMPLETE_PATH: &str = "/api/v1/dag/graphs/{id}/nodes/{node_id}/complete";
pub const MARK_COMPLETE_METHOD: &str = "POST";

// ---------------------------------------------------------------------------
// Endpoint: DELETE /api/v1/dag/graphs/{id}
// ---------------------------------------------------------------------------

/// DELETE /api/v1/dag/graphs/{id}
///
/// Delete a TaskGraph.
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `DeleteGraphResponse`
pub const DELETE_GRAPH_PATH: &str = "/api/v1/dag/graphs/{id}";
pub const DELETE_GRAPH_METHOD: &str = "DELETE";

/// Response body for DELETE /api/v1/dag/graphs/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGraphResponse {
    pub dag_id: Uuid,
    pub deleted: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/dag/plans/compare
// ---------------------------------------------------------------------------

/// POST /api/v1/dag/plans/compare
///
/// Compare two execution plans and compute a structured diff.
///
/// **Request Body:** `ComparePlansRequest`
/// **Response:** `200 OK` with `ComparePlansResponse`
pub const COMPARE_PLANS_PATH: &str = "/api/v1/dag/plans/compare";
pub const COMPARE_PLANS_METHOD: &str = "POST";

/// Request body for POST /api/v1/dag/plans/compare.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparePlansRequest {
    /// The old plan's nodes.
    pub old_nodes: Vec<TaskNode>,
    /// The new plan's nodes.
    pub new_nodes: Vec<TaskNode>,
}

/// Response body for POST /api/v1/dag/plans/compare.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparePlansResponse {
    pub added_count: u32,
    pub removed_count: u32,
    pub modified_count: u32,
    pub unchanged_count: u32,
    pub impact_level: String,
    pub added_nodes: Vec<TaskNode>,
    pub removed_nodes: Vec<TaskNode>,
    pub modified_nodes: Vec<NodeDiff>,
    pub compared_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/dag/graphs
// ---------------------------------------------------------------------------

/// GET /api/v1/dag/graphs
///
/// List all available graphs with summary information.
///
/// **Query Params:**
/// - `limit` (optional, default 50) — Maximum number of graphs to return
/// - `offset` (optional, default 0) — Pagination offset
///
/// **Response:** `200 OK` with `ListGraphsResponse`
pub const LIST_GRAPHS_PATH: &str = "/api/v1/dag/graphs";
pub const LIST_GRAPHS_METHOD: &str = "GET";

/// Response body for GET /api/v1/dag/graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGraphsResponse {
    pub graphs: Vec<PlanSummary>,
    pub total_count: u32,
    pub limit: u32,
    pub offset: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/dag/health
// ---------------------------------------------------------------------------

/// GET /api/v1/dag/health
///
/// Health check for the DAG Engine system.
///
/// **Response:** `200 OK` with `HealthResponse`
pub const HEALTH_PATH: &str = "/api/v1/dag/health";
pub const HEALTH_METHOD: &str = "GET";

/// Response body for GET /api/v1/dag/health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub graph_count: u64,
    pub plan_diff_count: u64,
    pub storage_path: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all DAG Engine API endpoints.
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

/// Standardized error codes for DAG Engine API.
pub mod error_codes {
    /// Graph not found.
    pub const GRAPH_NOT_FOUND: &str = "DAG_GRAPH_NOT_FOUND";
    /// Node not found within graph.
    pub const NODE_NOT_FOUND: &str = "DAG_NODE_NOT_FOUND";
    /// Cycle detected during topological sort.
    pub const CYCLE_DETECTED: &str = "DAG_CYCLE_DETECTED";
    /// Duplicate task ID.
    pub const DUPLICATE_TASK_ID: &str = "DAG_DUPLICATE_TASK_ID";
    /// Dependency not found.
    pub const DEPENDENCY_NOT_FOUND: &str = "DAG_DEPENDENCY_NOT_FOUND";
    /// Invalid graph state for the requested operation.
    pub const INVALID_GRAPH: &str = "DAG_INVALID_GRAPH";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "DAG_INTERNAL_ERROR";
}

/// HTTP status code mappings for DAG Engine errors.
pub mod status_codes {
    pub const GRAPH_NOT_FOUND: u16 = 404;
    pub const NODE_NOT_FOUND: u16 = 404;
    pub const CYCLE_DETECTED: u16 = 409;
    pub const DUPLICATE_TASK_ID: u16 = 409;
    pub const DEPENDENCY_NOT_FOUND: u16 = 400;
    pub const INVALID_GRAPH: u16 = 400;
    pub const INTERNAL_ERROR: u16 = 500;
}
