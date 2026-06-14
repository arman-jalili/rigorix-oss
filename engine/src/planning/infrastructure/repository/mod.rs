//! Repository interfaces for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanningResultRepository trait
//! Issue: issue-contract-freeze
//!
//! Planning results are persisted for audit trails, replay verification,
//! and debugging. The repository abstracts storage behind a trait so
//! that different backends (filesystem, database, S3) can be used.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;
use uuid::Uuid;

use crate::planning::domain::error::PlanningError;
use crate::planning::domain::result::{PlanningHash, PlanningResult};

/// Repository for persisting and loading planning results.
///
/// Stores the full `PlanningResult` and enables lookup by execution ID,
/// template ID, or planning hash. Supports audit replay by loading
/// a result by its deterministic hash.
///
/// # Contract (Frozen)
/// - `save_result` persists a PlanningResult for future reference
/// - `load_result` retrieves a result by execution ID
/// - `find_by_hash` enables deterministic audit replay lookup
/// - `list_by_template` enables analytics by template usage
/// - `delete_result` removes a result (e.g., for cleanup)
/// - `count` returns total stored results for monitoring
#[async_trait]
pub trait PlanningResultRepository: Send + Sync {
    /// Save a planning result.
    ///
    /// Returns `Ok(())` on success. If a result with the same
    /// execution_id already exists, it is overwritten.
    async fn save_result(&self, result: &PlanningResult) -> Result<(), PlanningError>;

    /// Load a planning result by execution ID.
    ///
    /// Returns `None` if no result exists for this execution ID.
    async fn load_result(&self, execution_id: Uuid) -> Result<Option<PlanningResult>, PlanningError>;

    /// Find a planning result by its deterministic hash.
    ///
    /// Enables audit replay — given the same input, the same hash
    /// should always be produced. Returns all results matching the
    /// hash (typically one, but multiple if hash collisions occur).
    async fn find_by_hash(
        &self,
        hash: &PlanningHash,
    ) -> Result<Vec<PlanningResult>, PlanningError>;

    /// List all planning results for a given template.
    ///
    /// Returns results ordered by `planned_at` descending (newest first).
    async fn list_by_template(
        &self,
        template_id: &str,
        limit: u32,
    ) -> Result<Vec<PlanningResult>, PlanningError>;

    /// Delete a planning result by execution ID.
    ///
    /// Returns `Ok(true)` if a result was deleted, `Ok(false)` if
    /// no result existed for this execution ID.
    async fn delete_result(&self, execution_id: Uuid) -> Result<bool, PlanningError>;

    /// Get the total number of stored planning results.
    async fn count(&self) -> Result<u64, PlanningError>;

    /// Check if a result exists for the given execution ID.
    async fn exists(&self, execution_id: Uuid) -> Result<bool, PlanningError>;
}
