//! Code Graph error types.
//!
//! @canonical .pi/architecture/modules/code-graph.md#errors
//! Implements: Contract Freeze — CodeGraphError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `CodeGraphError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;
use uuid::Uuid;

use super::graph::EdgeKind;

/// Errors that can occur during code graph construction, analysis, and persistence.
#[derive(Debug, Error)]
pub enum CodeGraphError {
    /// A node with the same UUID already exists in the graph.
    #[error("Duplicate node ID: {id}")]
    DuplicateNodeId {
        /// The UUID that was duplicated.
        id: Uuid,
    },

    /// An edge with the same source, target, and kind already exists.
    #[error("Duplicate edge: source={source_id}, target={target_id}, kind={kind}")]
    DuplicateEdge {
        /// The source node ID of the duplicate edge.
        source_id: Uuid,
        /// The target node ID of the duplicate edge.
        target_id: Uuid,
        /// The edge kind of the duplicate edge.
        kind: EdgeKind,
    },

    /// A node was not found in the graph.
    #[error("Node not found: {node_id}")]
    NodeNotFound {
        /// The UUID of the node that was not found.
        node_id: Uuid,
    },

    /// The graph is sealed (frozen) and cannot be modified.
    #[error("Graph is sealed and cannot perform operation: {operation}")]
    GraphSealed {
        /// The operation that was attempted on a sealed graph.
        operation: String,
    },

    /// The graph is empty and cannot be sealed or analyzed.
    #[error("Graph is empty")]
    EmptyGraph,

    /// An I/O error occurred during graph persistence or loading.
    #[error("I/O error: {detail}")]
    IoError {
        /// Details about the I/O error.
        detail: String,
    },

    /// Failed to serialize the graph to the requested format.
    #[error("Serialization error: {detail}")]
    SerializationError {
        /// Details about the serialization failure.
        detail: String,
    },

    /// Failed to deserialize graph data.
    #[error("Deserialization error: {detail}")]
    DeserializationError {
        /// Details about the deserialization failure.
        detail: String,
    },

    /// An invalid operation was attempted on the graph.
    #[error("Invalid operation: {reason}")]
    InvalidOperation {
        /// Human-readable explanation of why the operation is invalid.
        reason: String,
    },

    /// A cycle was detected in the graph during analysis.
    #[error("Cycle detected: {detail}")]
    CycleDetected {
        /// Details about the cycle that was detected.
        detail: String,
    },

    /// An internal invariant was violated.
    #[error("Internal error: {detail}")]
    InternalError {
        /// Details about the internal error.
        detail: String,
    },
}

impl CodeGraphError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            CodeGraphError::IoError { .. } | CodeGraphError::InternalError { .. }
        )
    }
}
