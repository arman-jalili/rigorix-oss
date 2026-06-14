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

// ---------------------------------------------------------------------------
// GeneratedTemplateRepository
// ---------------------------------------------------------------------------

/// Repository for caching and retrieving generated template definitions.
///
/// Stores generated template TOML content and metadata so that
/// previously generated templates can be reused without calling the
/// LLM again. Supports cache invalidation and intent-based lookup.
///
/// # Contract (Frozen)
/// - Templates are indexed by their intent hash for deterministic lookup
/// - Cache entries have a configurable TTL
/// - Lookup by intent returns the most recently generated template
/// - All methods are async and return domain error types
#[async_trait]
pub trait GeneratedTemplateRepository: Send + Sync {
    /// Save a generated template for future reuse.
    ///
    /// If a template with the same intent_hash already exists, it is
    /// overwritten with the new result.
    async fn save(
        &self,
        intent_hash: &str,
        generated: &crate::planning::domain::generator::GeneratedTemplate,
    ) -> Result<(), PlanningError>;

    /// Load a generated template by its intent hash.
    ///
    /// Returns `None` if no cached template exists for this hash.
    /// Returns `None` if the cache entry has expired (TTL exceeded).
    async fn load_by_intent_hash(
        &self,
        intent_hash: &str,
    ) -> Result<Option<crate::planning::domain::generator::GeneratedTemplate>, PlanningError>;

    /// Load a generated template by its suggested ID.
    ///
    /// Returns the most recently generated version of this template.
    async fn load_by_template_id(
        &self,
        template_id: &str,
    ) -> Result<Option<crate::planning::domain::generator::GeneratedTemplate>, PlanningError>;

    /// Delete a cached template entry.
    ///
    /// Returns `Ok(true)` if an entry was deleted, `Ok(false)` if no
    /// entry existed for the given intent hash.
    async fn delete(&self, intent_hash: &str) -> Result<bool, PlanningError>;

    /// Clear all cached generated templates.
    async fn clear_cache(&self) -> Result<(), PlanningError>;

    /// Get the number of cached generated templates.
    async fn cache_size(&self) -> Result<u64, PlanningError>;

    /// Check if a cached template exists for the given intent hash.
    async fn exists(&self, intent_hash: &str) -> Result<bool, PlanningError>;
}
