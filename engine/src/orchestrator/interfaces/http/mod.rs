//! HTTP API contracts for Orchestrator endpoints.
//!
//! @canonical .pi/architecture/modules/orchestrator.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #338
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

use crate::orchestrator::application::dto::{
    CancelInput, CancelOutput, PlanOnlyInput, PlanOnlyOutput, RunInput, RunOutput, StatusOutput,
};

use crate::orchestrator::domain::record::{ExecutionRecord, ExecutionStatus};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All orchestrator endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/orchestrator";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/orchestrator/run
// ---------------------------------------------------------------------------

/// POST /api/v1/orchestrator/run
///
/// Execute a full orchestrator lifecycle: plan → execute → persist → emit.
///
/// **Request:** `RunRequest`
/// **Response:** `201 Created` with `RunResponse`
pub const RUN_PATH: &str = "/api/v1/orchestrator/run";
pub const RUN_METHOD: &str = "POST";

/// Request body for POST /api/v1/orchestrator/run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRequest {
    /// The user's natural-language intent.
    pub intent: String,

    /// Serialized configuration.
    pub config: serde_json::Value,

    /// Repository root path.
    pub repo_root: String,

    /// Optional enforcement preset.
    pub enforcement_preset: Option<String>,
}

impl From<RunRequest> for RunInput {
    fn from(req: RunRequest) -> Self {
        Self {
            intent: req.intent,
            config: req.config,
            repo_root: req.repo_root,
            enforcement_preset: req.enforcement_preset,
        }
    }
}

/// Response body for POST /api/v1/orchestrator/run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResponse {
    pub success: bool,
    pub execution_id: uuid::Uuid,
    pub record: ExecutionRecord,
}

impl From<RunOutput> for RunResponse {
    fn from(output: RunOutput) -> Self {
        Self {
            success: true,
            execution_id: output.execution_id,
            record: output.record,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/orchestrator/plan
// ---------------------------------------------------------------------------

/// POST /api/v1/orchestrator/plan
///
/// Plan only (no execution). Returns the plan for preview.
///
/// **Request:** `PlanOnlyRequest`
/// **Response:** `200 OK` with `PlanOnlyResponse`
pub const PLAN_PATH: &str = "/api/v1/orchestrator/plan";
pub const PLAN_METHOD: &str = "POST";

/// Request body for POST /api/v1/orchestrator/plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOnlyRequest {
    /// The user's natural-language intent.
    pub intent: String,

    /// Serialized configuration.
    pub config: serde_json::Value,

    /// Repository root path.
    pub repo_root: String,
}

impl From<PlanOnlyRequest> for PlanOnlyInput {
    fn from(req: PlanOnlyRequest) -> Self {
        Self {
            intent: req.intent,
            config: req.config,
            repo_root: req.repo_root,
        }
    }
}

/// Response body for POST /api/v1/orchestrator/plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOnlyResponse {
    pub success: bool,
    pub plan: serde_json::Value,
    pub graph: serde_json::Value,
}

impl From<PlanOnlyOutput> for PlanOnlyResponse {
    fn from(output: PlanOnlyOutput) -> Self {
        Self {
            success: true,
            plan: output.plan,
            graph: output.graph,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/orchestrator/cancel
// ---------------------------------------------------------------------------

/// POST /api/v1/orchestrator/cancel
///
/// Cancel a running execution.
///
/// **Request:** `CancelRequest`
/// **Response:** `200 OK` with `CancelResponse`
pub const CANCEL_PATH: &str = "/api/v1/orchestrator/cancel";
pub const CANCEL_METHOD: &str = "POST";

/// Request body for POST /api/v1/orchestrator/cancel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelRequest {
    /// The execution ID to cancel.
    pub execution_id: uuid::Uuid,

    /// Optional reason for cancellation.
    pub reason: Option<String>,
}

impl From<CancelRequest> for CancelInput {
    fn from(req: CancelRequest) -> Self {
        Self {
            execution_id: req.execution_id,
            reason: req.reason,
        }
    }
}

/// Response body for POST /api/v1/orchestrator/cancel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelResponse {
    pub success: bool,
    pub execution_id: uuid::Uuid,
    pub aborted: bool,
    pub nodes_cancelled: u32,
}

impl From<CancelOutput> for CancelResponse {
    fn from(output: CancelOutput) -> Self {
        Self {
            success: true,
            execution_id: output.execution_id,
            aborted: output.aborted,
            nodes_cancelled: output.nodes_cancelled,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/orchestrator/status
// ---------------------------------------------------------------------------

/// GET /api/v1/orchestrator/status
///
/// Get the current execution status.
///
/// **Response:** `200 OK` with `StatusResponse`
pub const STATUS_PATH: &str = "/api/v1/orchestrator/status";
pub const STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/orchestrator/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub success: bool,
    pub execution_id: uuid::Uuid,
    pub status: ExecutionStatus,
    pub nodes: Vec<NodeStateDto>,
}

impl From<StatusOutput> for StatusResponse {
    fn from(output: StatusOutput) -> Self {
        Self {
            success: true,
            execution_id: output.execution_id,
            status: output.status,
            nodes: output.nodes.into_iter().map(Into::into).collect(),
        }
    }
}

/// DTO for node state in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateDto {
    pub node_id: String,
    pub node_name: String,
    pub status: String,
    pub message: Option<String>,
}

impl From<crate::orchestrator::application::dto::NodeState> for NodeStateDto {
    fn from(state: crate::orchestrator::application::dto::NodeState) -> Self {
        Self {
            node_id: state.node_id,
            node_name: state.node_name,
            status: state.status,
            message: state.message,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Orchestrator API endpoints.
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

/// Standardized error codes for Orchestrator API.
pub mod error_codes {
    pub const PLANNING_FAILED: &str = "ORCH_PLANNING_FAILED";
    pub const EXECUTION_FAILED: &str = "ORCH_EXECUTION_FAILED";
    pub const STATE_PERSISTENCE_FAILED: &str = "ORCH_STATE_PERSISTENCE_FAILED";
    pub const CANCELLATION_FAILED: &str = "ORCH_CANCELLATION_FAILED";
    pub const AUDIT_FAILED: &str = "ORCH_AUDIT_FAILED";
    pub const INTERNAL_ERROR: &str = "ORCH_INTERNAL_ERROR";
}

/// HTTP status code mappings for Orchestrator errors.
pub mod status_codes {
    pub const PLANNING_FAILED: u16 = 400;
    pub const EXECUTION_FAILED: u16 = 500;
    pub const STATE_PERSISTENCE_FAILED: u16 = 500;
    pub const CANCELLATION_FAILED: u16 = 400;
    pub const AUDIT_FAILED: u16 = 502;
    pub const INTERNAL_ERROR: u16 = 500;
}
