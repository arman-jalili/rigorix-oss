//! Repository interfaces for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: Contract Freeze — TaskGraphRepository and PlanDiffRepository traits
//! Issue: issue-contract-freeze
//!
//! TaskGraph records are typically persisted to the local filesystem for
//! crash recovery and execution replay. Plan diffs are persisted for audit
//! trail analysis.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;
use uuid::Uuid;

use crate::dag_engine::domain::{DagError, PlanDiff, TaskGraph};

/// Repository for CRUD operations on TaskGraph records.
///
/// The default implementation uses the local filesystem. Custom
/// implementations may use a database, S3, or any other storage backend.
///
/// # Contract (Frozen)
/// - `save` persists a TaskGraph for later retrieval
/// - `load` retrieves a TaskGraph by its ID
/// - `delete` removes a TaskGraph (idempotent)
/// - `list_ids` returns all available graph IDs
#[async_trait]
pub trait TaskGraphRepository: Send + Sync {
    /// Persist a TaskGraph to storage.
    ///
    /// Must be atomic — either the full graph is persisted or the
    /// previous state remains intact.
    async fn save(&self, graph: &TaskGraph) -> Result<(), DagError>;

    /// Load a TaskGraph from storage by its ID.
    ///
    /// The graph ID is typically embedded as a metadata field in the
    /// serialized graph file. Returns `DagError::InvalidGraph` with
    /// reason "Graph not found" if the graph does not exist.
    async fn load(&self, dag_id: Uuid) -> Result<TaskGraph, DagError>;

    /// Check if a TaskGraph exists in storage.
    async fn exists(&self, dag_id: Uuid) -> Result<bool, DagError>;

    /// Delete a TaskGraph from storage.
    ///
    /// Idempotent — returns `Ok(())` even if the graph does not exist.
    async fn delete(&self, dag_id: Uuid) -> Result<(), DagError>;

    /// List all available graph IDs in storage.
    async fn list_ids(&self) -> Result<Vec<Uuid>, DagError>;

    /// Count the number of TaskGraph records in storage.
    async fn count(&self) -> Result<u64, DagError>;
}

/// Repository for CRUD operations on plan diff records (audit trail).
///
/// Plan diffs capture the structural differences between plan versions
/// for audit and compliance review. Each diff is timestamped and linked
/// to an execution context.
///
/// # Contract (Frozen)
/// - Every plan comparison can be persisted via `save_diff`
/// - Diffs are retrievable by execution ID for audit review
/// - Old diffs can be pruned via configurable retention
#[async_trait]
pub trait PlanDiffRepository: Send + Sync {
    /// Persist a PlanDiff for audit trail recording.
    async fn save_diff(&self, diff: &PlanDiff) -> Result<(), DagError>;

    /// Load a PlanDiff by its associated execution/dag ID.
    ///
    /// Returns `DagError::InvalidGraph` if no diff exists for the given ID.
    async fn load_diff(&self, dag_id: Uuid) -> Result<PlanDiff, DagError>;

    /// Delete a PlanDiff by its associated execution/dag ID.
    ///
    /// Idempotent — returns `Ok(())` even if the diff does not exist.
    async fn delete_diff(&self, dag_id: Uuid) -> Result<(), DagError>;

    /// List all available diff IDs, most recent first.
    async fn list_diffs(&self, limit: u32, offset: u32) -> Result<Vec<Uuid>, DagError>;

    /// Count the number of plan diffs in storage.
    async fn count(&self) -> Result<u64, DagError>;

    /// Prune diffs older than the specified limit, keeping at most `max_diffs`.
    async fn prune(&self, max_diffs: u64) -> Result<u64, DagError>;
}
