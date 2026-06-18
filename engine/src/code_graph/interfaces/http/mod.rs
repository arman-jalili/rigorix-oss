//! HTTP API contracts for Code Graph endpoints.
//!
//! @canonical .pi/architecture/modules/code-graph.md
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
use std::collections::HashMap;
use uuid::Uuid;

use crate::code_graph::domain::{EdgeKind, ModuleEdge, ModuleNode, NodeKind};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All Code Graph endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/code-graph";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-graph/graphs
// ---------------------------------------------------------------------------

/// POST /api/v1/code-graph/graphs
///
/// Construct a new CodeGraph with metadata.
///
/// **Request Body:** `ConstructGraphRequest`
/// **Response:** `201 Created` with `GraphResponse`
pub const CONSTRUCT_GRAPH_PATH: &str = "/api/v1/code-graph/graphs";
pub const CONSTRUCT_GRAPH_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-graph/graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructGraphRequest {
    /// Name for the graph (e.g., "cargo-deps", "ts-imports").
    pub name: String,
    /// The tool or process that produced this graph.
    pub source: String,
    /// Human-readable description.
    pub description: String,
    /// Total number of modules scanned to produce this graph.
    pub total_modules_scanned: u64,
}

/// Response body for POST /api/v1/code-graph/graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphResponse {
    pub graph_id: Uuid,
    pub name: String,
    pub source: String,
    pub node_count: u32,
    pub edge_count: u32,
    pub sealed: bool,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-graph/graphs/{id}/nodes
// ---------------------------------------------------------------------------

/// POST /api/v1/code-graph/graphs/{id}/nodes
///
/// Add a module node to an existing graph.
///
/// **Path Param:** `id` — Graph UUID
/// **Request Body:** `AddNodeRequest`
/// **Response:** `201 Created` with `AddNodeResponse`
pub const ADD_NODE_PATH: &str = "/api/v1/code-graph/graphs/{id}/nodes";
pub const ADD_NODE_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-graph/graphs/{id}/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeRequest {
    pub name: String,
    pub kind: NodeKind,
    pub path: String,
    pub metadata: HashMap<String, String>,
}

/// Response body for POST /api/v1/code-graph/graphs/{id}/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeResponse {
    pub graph_id: Uuid,
    pub node_id: Uuid,
    pub node_count: u32,
    pub added_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-graph/graphs/{id}/edges
// ---------------------------------------------------------------------------

/// POST /api/v1/code-graph/graphs/{id}/edges
///
/// Add an edge between two module nodes.
///
/// **Path Param:** `id` — Graph UUID
/// **Request Body:** `AddEdgeRequest`
/// **Response:** `201 Created` with `AddEdgeResponse`
pub const ADD_EDGE_PATH: &str = "/api/v1/code-graph/graphs/{id}/edges";
pub const ADD_EDGE_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-graph/graphs/{id}/edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEdgeRequest {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub kind: EdgeKind,
    pub weight: u64,
    pub label: Option<String>,
}

/// Response body for POST /api/v1/code-graph/graphs/{id}/edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEdgeResponse {
    pub graph_id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub edge_count: u32,
    pub added_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-graph/graphs/{id}/seal
// ---------------------------------------------------------------------------

/// POST /api/v1/code-graph/graphs/{id}/seal
///
/// Seal a graph (freeze for analysis).
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `SealGraphResponse`
pub const SEAL_GRAPH_PATH: &str = "/api/v1/code-graph/graphs/{id}/seal";
pub const SEAL_GRAPH_METHOD: &str = "POST";

/// Response body for POST /api/v1/code-graph/graphs/{id}/seal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealGraphResponse {
    pub graph_id: Uuid,
    pub node_count: u32,
    pub edge_count: u32,
    pub sealed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/code-graph/graphs/{id}
// ---------------------------------------------------------------------------

/// GET /api/v1/code-graph/graphs/{id}
///
/// Retrieve a CodeGraph by its ID.
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `GetGraphResponse`
pub const GET_GRAPH_PATH: &str = "/api/v1/code-graph/graphs/{id}";
pub const GET_GRAPH_METHOD: &str = "GET";

/// Response body for GET /api/v1/code-graph/graphs/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGraphResponse {
    pub graph_id: Uuid,
    pub name: String,
    pub source: String,
    pub description: String,
    pub nodes: Vec<ModuleNode>,
    pub edges: Vec<ModuleEdge>,
    pub sealed: bool,
    pub node_count: u32,
    pub edge_count: u32,
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/code-graph/graphs/{id}/nodes/{node_id}
// ---------------------------------------------------------------------------

/// GET /api/v1/code-graph/graphs/{id}/nodes/{node_id}
///
/// Get detailed information about a specific node.
///
/// **Path Params:**
/// - `id` — Graph UUID
/// - `node_id` — Node UUID
///
/// **Response:** `200 OK` with `GetNodeResponse`
pub const GET_NODE_PATH: &str = "/api/v1/code-graph/graphs/{id}/nodes/{node_id}";
pub const GET_NODE_METHOD: &str = "GET";

/// Response body for GET /api/v1/code-graph/graphs/{id}/nodes/{node_id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeResponse {
    pub node: ModuleNode,
    pub incoming_edges: Vec<ModuleEdge>,
    pub outgoing_edges: Vec<ModuleEdge>,
    pub dependency_count: u32,
    pub dependent_count: u32,
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/code-graph/graphs/{id}/nodes
// ---------------------------------------------------------------------------

/// GET /api/v1/code-graph/graphs/{id}/nodes
///
/// List all nodes in a graph.
///
/// **Path Param:** `id` — Graph UUID
/// **Query Params:**
/// - `kind` (optional) — Filter by NodeKind
/// - `limit` (optional, default 100) — Maximum nodes to return
/// - `offset` (optional, default 0) — Pagination offset
///
/// **Response:** `200 OK` with `ListNodesResponse`
pub const LIST_NODES_PATH: &str = "/api/v1/code-graph/graphs/{id}/nodes";
pub const LIST_NODES_METHOD: &str = "GET";

/// Response body for GET /api/v1/code-graph/graphs/{id}/nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<ModuleNode>,
    pub total_count: u32,
    pub limit: u32,
    pub offset: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/code-graph/graphs
// ---------------------------------------------------------------------------

/// GET /api/v1/code-graph/graphs
///
/// List all available graphs with summary information.
///
/// **Query Params:**
/// - `limit` (optional, default 50) — Maximum graphs to return
/// - `offset` (optional, default 0) — Pagination offset
///
/// **Response:** `200 OK` with `ListGraphsResponse`
pub const LIST_GRAPHS_PATH: &str = "/api/v1/code-graph/graphs";
pub const LIST_GRAPHS_METHOD: &str = "GET";

/// Response body for GET /api/v1/code-graph/graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGraphsResponse {
    pub graphs: Vec<GraphSummaryResponse>,
    pub total_count: u32,
    pub limit: u32,
    pub offset: u32,
}

/// Summary of a CodeGraph for listing displays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSummaryResponse {
    pub graph_id: Uuid,
    pub name: String,
    pub source: String,
    pub node_count: u32,
    pub edge_count: u32,
    pub is_sealed: bool,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: DELETE /api/v1/code-graph/graphs/{id}
// ---------------------------------------------------------------------------

/// DELETE /api/v1/code-graph/graphs/{id}
///
/// Delete a CodeGraph from storage.
///
/// **Path Param:** `id` — Graph UUID
/// **Response:** `200 OK` with `DeleteGraphResponse`
pub const DELETE_GRAPH_PATH: &str = "/api/v1/code-graph/graphs/{id}";
pub const DELETE_GRAPH_METHOD: &str = "DELETE";

/// Response body for DELETE /api/v1/code-graph/graphs/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGraphResponse {
    pub graph_id: Uuid,
    pub deleted: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-graph/graphs/{id}/analyze
// ---------------------------------------------------------------------------

/// POST /api/v1/code-graph/graphs/{id}/analyze
///
/// Run dependency analysis on a sealed graph.
///
/// **Path Param:** `id` — Graph UUID
/// **Request Body:** `AnalyzeRequest`
/// **Response:** `200 OK` with `AnalyzeResponse`
pub const ANALYZE_PATH: &str = "/api/v1/code-graph/graphs/{id}/analyze";
pub const ANALYZE_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-graph/graphs/{id}/analyze.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub include_transitive: bool,
    pub scope_node_id: Option<Uuid>,
}

/// Response body for POST /api/v1/code-graph/graphs/{id}/analyze.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeResponse {
    pub graph_id: Uuid,
    pub total_nodes: u32,
    pub total_edges: u32,
    pub cycle_count: u32,
    pub cycle_paths: Vec<Vec<String>>,
    pub root_node_count: u32,
    pub leaf_node_count: u32,
    pub analyzed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-graph/graphs/{id}/impact
// ---------------------------------------------------------------------------

/// POST /api/v1/code-graph/graphs/{id}/impact
///
/// Analyze the impact of changing a specific module.
///
/// **Path Param:** `id` — Graph UUID
/// **Request Body:** `ImpactRequest`
/// **Response:** `200 OK` with `ImpactResponse`
pub const IMPACT_PATH: &str = "/api/v1/code-graph/graphs/{id}/impact";
pub const IMPACT_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-graph/graphs/{id}/impact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactRequest {
    pub node_id: Uuid,
    pub max_depth: u32,
}

/// Response body for POST /api/v1/code-graph/graphs/{id}/impact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactResponse {
    pub graph_id: Uuid,
    pub target_node: ModuleNode,
    pub direct_impact_count: u32,
    pub total_impact_count: u32,
    pub impact_chains: Vec<ImpactChainResponse>,
    pub analyzed_at: DateTime<Utc>,
}

/// An impact chain from target to affected module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactChainResponse {
    pub affected_node: ModuleNode,
    pub depth: u32,
    pub path: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-graph/graphs/{id}/format
// ---------------------------------------------------------------------------

/// POST /api/v1/code-graph/graphs/{id}/format
///
/// Format a sealed graph into the specified output format.
///
/// **Path Param:** `id` — Graph UUID
/// **Request Body:** `FormatRequest`
/// **Response:** `200 OK` with `FormatResponse`
pub const FORMAT_PATH: &str = "/api/v1/code-graph/graphs/{id}/format";
pub const FORMAT_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-graph/graphs/{id}/format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatRequest {
    pub format: String,
    pub include_metadata: bool,
}

/// Response body for POST /api/v1/code-graph/graphs/{id}/format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatResponse {
    pub graph_id: Uuid,
    pub format: String,
    pub output: String,
    pub output_size: u64,
    pub formatted_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/code-graph/health
// ---------------------------------------------------------------------------

/// GET /api/v1/code-graph/health
///
/// Health check for the Code Graph module.
///
/// **Response:** `200 OK` with `HealthResponse`
pub const HEALTH_PATH: &str = "/api/v1/code-graph/health";
pub const HEALTH_METHOD: &str = "GET";

/// Response body for GET /api/v1/code-graph/health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub graph_count: u64,
    pub storage_path: String,
    pub schema_version: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Code Graph API endpoints.
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

/// Standardized error codes for Code Graph API.
pub mod error_codes {
    /// Graph not found.
    pub const GRAPH_NOT_FOUND: &str = "CG_GRAPH_NOT_FOUND";
    /// Node not found within graph.
    pub const NODE_NOT_FOUND: &str = "CG_NODE_NOT_FOUND";
    /// Duplicate node ID.
    pub const DUPLICATE_NODE: &str = "CG_DUPLICATE_NODE";
    /// Duplicate edge.
    pub const DUPLICATE_EDGE: &str = "CG_DUPLICATE_EDGE";
    /// Graph is sealed and cannot be modified.
    pub const GRAPH_SEALED: &str = "CG_GRAPH_SEALED";
    /// Graph is empty.
    pub const EMPTY_GRAPH: &str = "CG_EMPTY_GRAPH";
    /// Graph not sealed (operation requires sealed graph).
    pub const GRAPH_NOT_SEALED: &str = "CG_GRAPH_NOT_SEALED";
    /// Cycle detected during analysis.
    pub const CYCLE_DETECTED: &str = "CG_CYCLE_DETECTED";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "CG_INTERNAL_ERROR";
}

/// HTTP status code mappings for Code Graph errors.
pub mod status_codes {
    pub const GRAPH_NOT_FOUND: u16 = 404;
    pub const NODE_NOT_FOUND: u16 = 404;
    pub const DUPLICATE_NODE: u16 = 409;
    pub const DUPLICATE_EDGE: u16 = 409;
    pub const GRAPH_SEALED: u16 = 409;
    pub const EMPTY_GRAPH: u16 = 400;
    pub const GRAPH_NOT_SEALED: u16 = 400;
    pub const CYCLE_DETECTED: u16 = 409;
    pub const INTERNAL_ERROR: u16 = 500;
}
