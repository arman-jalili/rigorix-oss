//! Repository interfaces for the State Persistence bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — StateRepository and GraphRepository traits
//! Issue: issue-contract-freeze
//!
//! Execution state is typically persisted to the local filesystem via atomic
//! write-rename. However, for advanced use cases — remote state storage,
//! database-backed history, or cluster-wide state sharing — repository
//! interfaces are provided.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;
use uuid::Uuid;

use crate::state_persistence::domain::{
    ExecutionGraph, ExecutionRecord, ExecutionState, StateError,
};

/// Repository for CRUD operations on execution state.
///
/// The default implementation uses the local filesystem with atomic
/// write-rename. Custom implementations may use S3, a database, or
/// any other storage backend.
#[async_trait]
pub trait StateRepository: Send + Sync {
    /// Save an execution state to storage.
    ///
    /// Must be atomic — either the full state is persisted or the
    /// previous state remains intact. Uses write-rename pattern:
    /// `{execution_id}.json.tmp` → `{execution_id}.json`
    async fn save(&self, state: &ExecutionState) -> Result<(), StateError>;

    /// Load an execution state from storage.
    ///
    /// Returns `StateError::StateNotFound` if the state does not exist.
    /// Returns `StateError::CorruptedState` if the file is unreadable.
    async fn load(&self, execution_id: Uuid) -> Result<ExecutionState, StateError>;

    /// Check if an execution state exists in storage.
    async fn exists(&self, execution_id: Uuid) -> Result<bool, StateError>;

    /// Delete an execution state from storage.
    ///
    /// Idempotent — returns `Ok(())` even if the state does not exist.
    async fn delete(&self, execution_id: Uuid) -> Result<(), StateError>;

    /// List all execution IDs available in storage.
    ///
    /// Returns an empty list if no states are stored.
    async fn list_ids(&self) -> Result<Vec<Uuid>, StateError>;

    /// Count the number of execution states in storage.
    async fn count(&self) -> Result<u64, StateError>;
}

/// Repository for CRUD operations on execution graph records.
///
/// Execution graphs are persisted separately from state files because
/// they are larger (include the full DAG structure and node metadata)
/// and are accessed less frequently (primarily by the TUI).
#[async_trait]
pub trait GraphRepository: Send + Sync {
    /// Save an execution graph to storage.
    async fn save_graph(&self, graph: &ExecutionGraph) -> Result<(), StateError>;

    /// Load an execution graph from storage by graph ID.
    ///
    /// Returns `StateError::GraphNotFound` if the graph does not exist.
    async fn load_graph(&self, graph_id: Uuid) -> Result<ExecutionGraph, StateError>;

    /// Load an execution graph by the associated execution ID.
    async fn load_by_execution_id(&self, execution_id: Uuid) -> Result<ExecutionGraph, StateError>;

    /// Delete an execution graph from storage.
    async fn delete_graph(&self, graph_id: Uuid) -> Result<(), StateError>;

    /// List all available graph IDs, most recent first.
    async fn list_graphs(&self, limit: u32, offset: u32) -> Result<Vec<Uuid>, StateError>;

    /// Count the number of execution graphs in storage.
    async fn count(&self) -> Result<u64, StateError>;
}

/// Repository for CRUD operations on execution records.
///
/// Execution records aggregate the final state, all drained events,
/// and the execution graph into a single complete record.
#[async_trait]
pub trait ExecutionRecordRepository: Send + Sync {
    /// Save an execution record to storage.
    async fn save_record(&self, record: &ExecutionRecord) -> Result<(), StateError>;

    /// Load an execution record from storage by record ID.
    async fn load_record(&self, record_id: Uuid) -> Result<ExecutionRecord, StateError>;

    /// Load an execution record by the associated execution ID.
    async fn load_by_execution_id(&self, execution_id: Uuid)
    -> Result<ExecutionRecord, StateError>;

    /// Delete an execution record from storage.
    async fn delete_record(&self, record_id: Uuid) -> Result<(), StateError>;

    /// List all available record IDs, most recent first.
    async fn list_records(&self, limit: u32, offset: u32) -> Result<Vec<Uuid>, StateError>;

    /// Count the number of execution records in storage.
    async fn count(&self) -> Result<u64, StateError>;
}
