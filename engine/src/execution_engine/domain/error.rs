//! Execution Engine error types.
//!
//! @canonical .pi/architecture/modules/execution-engine.md#errors
//! Implements: Contract Freeze — ExecutionError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `ExecutionError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during parallel execution and retry operations.
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// The node was not found in the executor's graph.
    #[error("Node not found: {node_id}")]
    NodeNotFound {
        /// The UUID of the node that was not found.
        node_id: Uuid,
    },

    /// The graph has not been sealed and cannot be executed.
    #[error("Graph not sealed: {dag_id}")]
    GraphNotSealed {
        /// The UUID of the graph that has not been sealed.
        dag_id: Uuid,
    },

    /// A node failed execution with the given failure type.
    #[error("Node {node_id} failed: {failure_type} - {message}")]
    NodeExecutionFailed {
        /// The ID of the failing node.
        node_id: Uuid,
        /// The classification of the failure.
        failure_type: String,
        /// Human-readable error message.
        message: String,
        /// The retry attempt number when the failure occurred.
        attempt: u8,
    },

    /// The retry limit was exhausted for a node.
    #[error("Retry limit exhausted for node {node_id}: max_retries={max_retries}, attempts={attempts}")]
    RetryLimitExhausted {
        /// The UUID of the node that exhausted retries.
        node_id: Uuid,
        /// The maximum number of retries allowed.
        max_retries: u8,
        /// The actual number of attempts made.
        attempts: u8,
    },

    /// Fallback execution failed.
    #[error("Fallback execution failed for node {original_node_id}: {message}")]
    FallbackFailed {
        /// The UUID of the original node that triggered the fallback.
        original_node_id: Uuid,
        /// The UUID of the fallback node that was attempted.
        fallback_node_id: Uuid,
        /// Error details from fallback execution.
        message: String,
    },

    /// The executor received a cancellation signal.
    #[error("Execution cancelled: {reason}")]
    ExecutionCancelled {
        /// Human-readable reason for cancellation.
        reason: String,
        /// Number of nodes completed before cancellation.
        completed_count: u32,
        /// Number of nodes remaining when cancelled.
        remaining_count: u32,
    },

    /// The enforcement policy rejected execution.
    #[error("Enforcement rejected execution: {reason}")]
    EnforcementRejected {
        /// Human-readable reason from the enforcer.
        reason: String,
        /// The enforcement limit that was exceeded.
        limit_name: String,
    },

    /// The execution engine is in an invalid state.
    #[error("Invalid execution state: {reason}")]
    InvalidState {
        /// Human-readable explanation.
        reason: String,
    },

    /// An internal invariant was violated.
    #[error("Internal error: {detail}")]
    InternalError {
        /// Details about the internal error.
        detail: String,
    },

    /// A timeout occurred during execution.
    #[error("Execution timeout: {detail}")]
    Timeout {
        /// Details about the timeout.
        detail: String,
        /// The timeout duration in milliseconds.
        timeout_ms: u64,
    },
}
