use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionCliEvent {
    ExecutionStarted {
        execution_id: String,
    },
    ExecutionCompleted {
        execution_id: String,
        success: bool,
    },
    ExecutionFailed {
        execution_id: String,
        error: String,
    },
    NodeStarted {
        execution_id: String,
        node_id: String,
    },
    NodeCompleted {
        execution_id: String,
        node_id: String,
    },
    NodeFailed {
        execution_id: String,
        node_id: String,
        error: String,
    },
    ExecutionAborted {
        execution_id: String,
        reason: String,
    },
}
