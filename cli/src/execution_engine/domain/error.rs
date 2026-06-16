//! CLI-specific execution engine errors.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — ExecutionCliError
//! Issue: issue-contract-freeze

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionCliError {
    #[error("Execution failed: {detail}")]
    ExecutionFailed { detail: String },

    #[error("Node execution failed for '{node_id}': {detail}")]
    NodeExecutionFailed { node_id: String, detail: String },

    #[error("Execution aborted: {detail}")]
    Aborted { detail: String },

    #[error("Execution not found: {execution_id}")]
    NotFound { execution_id: String },

    #[error("Execution already completed: {execution_id}")]
    AlreadyCompleted { execution_id: String },

    #[error("Internal error: {detail}")]
    Internal { detail: String },
}

impl ExecutionCliError {
    pub fn is_retriable(&self) -> bool {
        matches!(self, ExecutionCliError::ExecutionFailed { .. })
    }
}
