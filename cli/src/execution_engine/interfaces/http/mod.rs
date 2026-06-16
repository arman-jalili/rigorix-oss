use crate::execution_engine::application::dto::{
    AbortOutput, ExecuteOutput, ExecutionStatusOutput,
};
use serde::{Deserialize, Serialize};

pub const API_BASE_PATH: &str = "/api/v1/cli/execution";
pub const EXECUTE_PATH: &str = "/api/v1/cli/execution/execute";
pub const EXECUTE_METHOD: &str = "POST";
pub const STATUS_PATH: &str = "/api/v1/cli/execution/status/{id}";
pub const STATUS_METHOD: &str = "GET";
pub const ABORT_PATH: &str = "/api/v1/cli/execution/abort";
pub const ABORT_METHOD: &str = "POST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteApiRequest {
    pub execution_id: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteApiResponse {
    pub execution_id: String,
    pub success: bool,
    pub total_nodes: u32,
    pub completed: u32,
    pub failed: u32,
    pub duration_ms: u64,
}

impl From<ExecuteOutput> for ExecuteApiResponse {
    fn from(o: ExecuteOutput) -> Self {
        Self {
            execution_id: o.execution_id,
            success: o.success,
            total_nodes: o.total_nodes,
            completed: o.completed,
            failed: o.failed,
            duration_ms: o.duration_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusApiResponse {
    pub execution_id: String,
    pub status: String,
    pub nodes: Vec<NodeStatusItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatusItem {
    pub node_id: String,
    pub status: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl From<ExecutionStatusOutput> for StatusApiResponse {
    fn from(o: ExecutionStatusOutput) -> Self {
        Self {
            execution_id: o.execution_id,
            status: format!("{:?}", o.status),
            nodes: o
                .nodes
                .into_iter()
                .map(|n| NodeStatusItem {
                    node_id: n.node_id,
                    status: n.status,
                    duration_ms: n.duration_ms,
                    error: n.error,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortApiRequest {
    pub execution_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortApiResponse {
    pub execution_id: String,
    pub aborted: bool,
    pub nodes_cancelled: u32,
}

impl From<AbortOutput> for AbortApiResponse {
    fn from(o: AbortOutput) -> Self {
        Self {
            execution_id: o.execution_id,
            aborted: o.aborted,
            nodes_cancelled: o.nodes_cancelled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliApiErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

pub mod error_codes {
    pub const EXECUTION_FAILED: &str = "EXECUTION_FAILED";
    pub const NOT_FOUND: &str = "EXECUTION_NOT_FOUND";
    pub const INTERNAL_ERROR: &str = "EXECUTION_INTERNAL_ERROR";
}

pub mod status_codes {
    pub const EXECUTION_FAILED: u16 = 422;
    pub const NOT_FOUND: u16 = 404;
    pub const INTERNAL_ERROR: u16 = 500;
}
