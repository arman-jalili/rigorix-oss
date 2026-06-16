use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteInput {
    pub execution_id: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteOutput {
    pub execution_id: String,
    pub success: bool,
    pub total_nodes: u32,
    pub completed: u32,
    pub failed: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatusInput {
    pub execution_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatusOutput {
    pub execution_id: String,
    pub status: ExecutionStatus,
    pub nodes: Vec<NodeStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    pub node_id: String,
    pub status: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortInput {
    pub execution_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortOutput {
    pub execution_id: String,
    pub aborted: bool,
    pub nodes_cancelled: u32,
}
