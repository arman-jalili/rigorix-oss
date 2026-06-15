//! State persistence error types.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#errors
//! Implements: Contract Freeze — StateError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `StateError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during state persistence operations.
#[derive(Debug, Error)]
pub enum StateError {
    /// The state file was not found for the given execution ID.
    #[error("State not found for execution {execution_id}")]
    StateNotFound {
        /// The execution ID whose state was requested.
        execution_id: String,
    },

    /// A node was not found in the execution state.
    #[error("Node {node_id} not found in execution {execution_id}")]
    NodeNotFound {
        /// The node ID that was not found.
        node_id: String,
        /// The execution ID being queried.
        execution_id: String,
    },

    /// An invalid state transition was attempted.
    #[error("Invalid state transition: {from} -> {to}. {detail}")]
    InvalidTransition {
        /// The current state/status.
        from: String,
        /// The attempted target state/status.
        to: String,
        /// Human-readable explanation of why the transition is invalid.
        detail: String,
    },

    /// An invalid node state transition was attempted.
    #[error("Invalid node state transition for node {node_id}: {from} -> {to}. {detail}")]
    InvalidNodeTransition {
        /// The node ID where the invalid transition was attempted.
        node_id: String,
        /// The current state/status of the node.
        from: String,
        /// The attempted target state/status.
        to: String,
        /// Human-readable explanation of why the transition is invalid.
        detail: String,
    },

    /// Retry limit was exceeded for a node.
    #[error("Retry limit exceeded for node {node_id}: {retries} retries, max {max_retries}")]
    RetryLimitExceeded {
        /// The node ID that exceeded its retry limit.
        node_id: String,
        /// The number of retries attempted.
        retries: u8,
        /// The maximum number of retries allowed.
        max_retries: u8,
    },

    /// Failed to serialise state to JSON.
    #[error("Failed to serialise state: {detail}")]
    SerialisationError {
        /// Details about the serialisation failure.
        detail: String,
    },

    /// Failed to deserialise state from JSON.
    #[error("Failed to deserialise state: {detail}")]
    DeserialisationError {
        /// Details about the deserialisation failure.
        detail: String,
    },

    /// An I/O error occurred during state persistence.
    #[error("I/O error during state persistence: {detail}")]
    IoError {
        /// Details about the I/O error.
        detail: String,
    },

    /// A file lock could not be acquired.
    #[error("Could not acquire file lock for {path}: {detail}")]
    LockError {
        /// The path that could not be locked.
        path: String,
        /// Details about the lock failure.
        detail: String,
    },

    /// The state directory could not be created or accessed.
    #[error("State directory error: {detail}")]
    DirectoryError {
        /// Details about the directory error.
        detail: String,
    },

    /// An execution graph was not found for the given ID.
    #[error("Execution graph not found: {graph_id}")]
    GraphNotFound {
        /// The graph ID that was not found.
        graph_id: String,
    },

    /// An invalid or corrupted state file was encountered.
    #[error("Corrupted state file: {path}. {detail}")]
    CorruptedState {
        /// The path to the corrupted state file.
        path: String,
        /// Details about the corruption.
        detail: String,
    },

    /// A cross-process locking error occurred.
    #[error("Cross-process lock error: {detail}")]
    FdLockError {
        /// Details about the lock error.
        detail: String,
    },
}
impl StateError {
    pub fn is_retriable(&self) -> bool {
        matches!(self, StateError::InvalidTransition { .. })
    }
}
