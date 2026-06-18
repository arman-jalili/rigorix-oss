//! DAG Engine error types.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#errors
//! Implements: Contract Freeze — DagError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `DagError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during DAG construction, validation, and execution.
#[derive(Debug, Error)]
pub enum DagError {
    /// A cycle was detected during topological sort.
    ///
    /// Contains the number of nodes successfully processed before the cycle was
    /// detected (found) and the total number of nodes in the graph.
    #[error("Cycle detected: processed {found} of {total} nodes")]
    CycleDetected {
        /// Number of nodes successfully processed before cycle detection.
        found: usize,
        /// Total number of nodes in the graph.
        total: usize,
    },

    /// A task/node was not found in the graph.
    #[error("Task not found: {id}")]
    TaskNotFound {
        /// The UUID of the task that was not found.
        id: Uuid,
    },

    /// One or more dependency UUIDs could not be matched to existing nodes.
    #[error("Dependencies not found: {missing:?}")]
    DependencyNotFound {
        /// The UUIDs of dependencies that could not be found.
        missing: Vec<Uuid>,
    },

    /// A node with the same UUID already exists in the graph.
    #[error("Duplicate task ID: {id}")]
    DuplicateTaskId {
        /// The UUID that was duplicated.
        id: Uuid,
    },

    /// The graph is in an invalid state for the requested operation.
    #[error("Invalid graph: {reason}")]
    InvalidGraph {
        /// Human-readable explanation of why the graph is invalid.
        reason: String,
    },

    /// A node failed execution with the given failure type.
    #[error("Node {node_id} failed: {failure_type:?} - {message}")]
    NodeExecutionFailed {
        /// The ID of the failing node.
        node_id: Uuid,
        /// The classification of the failure.
        failure_type: crate::failure_classification::domain::FailureType,
        /// Human-readable error message.
        message: String,
    },

    /// The retry limit was exceeded for a node.
    #[error("Retry limit exceeded for node {node_id}: max_retries={max_retries}")]
    RetryLimitExceeded {
        /// The ID of the node that exceeded its retry limit.
        node_id: Uuid,
        /// The maximum number of retries allowed.
        max_retries: u8,
    },

    /// A fallback node was specified but could not be found in the graph.
    #[error("Fallback node not found: {fallback_id}")]
    FallbackNodeNotFound {
        /// The UUID of the fallback node that was not found.
        fallback_id: Uuid,
    },

    /// A validation rule failed after node execution.
    #[error("Validation failed for node {node_id}: {rule:?} - {message}")]
    ValidationFailed {
        /// The ID of the node that failed validation.
        node_id: Uuid,
        /// The validation rule that failed.
        rule: String,
        /// Details about the validation failure.
        message: String,
    },

    /// An I/O error occurred during graph persistence or loading.
    #[error("I/O error: {detail}")]
    IoError {
        /// Details about the I/O error.
        detail: String,
    },

    /// Failed to serialise the graph to JSON or another format.
    #[error("Failed to serialise graph: {detail}")]
    SerialisationError {
        /// Details about the serialisation failure.
        detail: String,
    },

    /// Failed to deserialise the graph from JSON or another format.
    #[error("Failed to deserialise graph: {detail}")]
    DeserialisationError {
        /// Details about the deserialisation failure.
        detail: String,
    },

    /// An internal invariant was violated.
    #[error("Internal error: {detail}")]
    InternalError {
        /// Details about the internal error.
        detail: String,
    },
}

impl DagError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            DagError::InternalError { .. } | DagError::TaskNotFound { .. }
        )
    }
}
