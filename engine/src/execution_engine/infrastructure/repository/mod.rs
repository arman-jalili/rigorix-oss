//! Repository interfaces for the Execution Engine bounded context.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — ExecutionResultRepository trait
//! Issue: issue-contract-freeze
//!
//! Execution results are persisted for:
//! - Crash recovery (restore in-flight executions after restart)
//! - Execution history and audit trails
//! - Replay and debugging
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;
use uuid::Uuid;

use crate::execution_engine::domain::{ExecutionError, ExecutionResult, NodeExecutionState};

/// Repository for CRUD operations on execution results.
///
/// The default implementation serialises to the local filesystem.
/// Custom implementations may use a database or any other storage backend.
///
/// # Contract (Frozen)
/// - `save_result` persists an ExecutionResult for later retrieval
/// - `load_result` retrieves an ExecutionResult by its dag_id
/// - `save_state` persists the in-flight execution state (for crash recovery)
/// - `load_state` retrieves the in-flight execution state
/// - `delete_execution` removes all execution data for a dag_id
#[async_trait]
pub trait ExecutionResultRepository: Send + Sync {
    /// Persist a completed ExecutionResult to storage.
    ///
    /// Must be atomic — either the full result is persisted or nothing changes.
    async fn save_result(&self, result: &ExecutionResult) -> Result<(), ExecutionError>;

    /// Load an ExecutionResult from storage by its dag_id.
    ///
    /// Returns `ExecutionError::NodeNotFound` if the result does not exist.
    async fn load_result(&self, dag_id: Uuid) -> Result<ExecutionResult, ExecutionError>;

    /// Check if an ExecutionResult exists in storage.
    async fn exists(&self, dag_id: Uuid) -> Result<bool, ExecutionError>;

    /// Persist the in-flight execution state for crash recovery.
    ///
    /// Called periodically during execution and after each node completes.
    async fn save_state(
        &self,
        dag_id: Uuid,
        node_states: &[NodeExecutionState],
    ) -> Result<(), ExecutionError>;

    /// Load the in-flight execution state for crash recovery.
    ///
    /// Returns the saved node states if the execution was interrupted.
    /// Returns an empty Vec if no in-flight state exists.
    async fn load_state(&self, dag_id: Uuid) -> Result<Vec<NodeExecutionState>, ExecutionError>;

    /// Delete all execution data (result + state) for a dag_id.
    ///
    /// Idempotent — returns `Ok(())` even if no data exists.
    async fn delete_execution(&self, dag_id: Uuid) -> Result<(), ExecutionError>;

    /// List all available execution IDs in storage.
    async fn list_executions(&self) -> Result<Vec<Uuid>, ExecutionError>;

    /// Count the number of execution records in storage.
    async fn count(&self) -> Result<u64, ExecutionError>;
}

/// Repository for persisting retry decisions (audit trail).
///
/// Every retry decision made during execution can be persisted for
/// audit and analysis of retry effectiveness.
///
/// # Contract (Frozen)
/// - Each retry decision is recorded with the dag_id and node_id
/// - Decisions are retrievable by execution ID for analysis
/// - Old records can be pruned via configurable retention
#[async_trait]
pub trait RetryDecisionRepository: Send + Sync {
    /// Persist a retry decision.
    async fn save_decision(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
        decision: &crate::execution_engine::domain::RetryDecision,
    ) -> Result<(), ExecutionError>;

    /// Load all retry decisions for a given DAG execution.
    async fn load_decisions(
        &self,
        dag_id: Uuid,
    ) -> Result<Vec<(Uuid, crate::execution_engine::domain::RetryDecision)>, ExecutionError>;

    /// Delete all retry decisions for a given DAG execution.
    async fn delete_decisions(&self, dag_id: Uuid) -> Result<(), ExecutionError>;

    /// Prune retry decisions older than the specified limit.
    async fn prune(&self, max_records: u64) -> Result<u64, ExecutionError>;
}
