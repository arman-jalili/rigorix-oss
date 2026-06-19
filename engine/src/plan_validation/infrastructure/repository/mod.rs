//! Repository interfaces for the Plan Validation bounded context.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — ValidationReportRepository, ValidatedTemplateRepository
//! Issue: issue-contract-freeze
//!
//! Validation reports and validated templates are persisted for audit trails,
//! replay verification, and caching. The repositories abstract storage behind
//! traits so that different backends (filesystem, database, S3) can be used.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;
use uuid::Uuid;

use crate::plan_validation::domain::error::ValidationLoopError;
use crate::plan_validation::domain::report::ValidationReport;
use crate::templates::domain::Template;

/// Repository for persisting and loading validation reports.
///
/// Stores the full `ValidationReport` and enables lookup by execution ID
/// or template ID. Supports audit trails and debugging by retaining
/// the full failure history across all iterations.
///
/// # Contract (Frozen)
/// - `save_report` persists a ValidationReport for future reference
/// - `load_report` retrieves a report by execution ID
/// - `list_by_template` retrieves all reports for a given template
/// - `list_recent` returns the most recent N reports
/// - `delete_report` removes a report (e.g., for cleanup)
/// - `count` returns total stored reports for monitoring
#[async_trait]
pub trait ValidationReportRepository: Send + Sync {
    /// Save a validation report.
    ///
    /// Returns `Ok(())` on success. If a report with the same
    /// execution_id already exists, it is overwritten.
    async fn save_report(&self, report: &ValidationReport) -> Result<(), ValidationLoopError>;

    /// Load a validation report by execution ID.
    ///
    /// Returns `None` if no report exists for this execution ID.
    async fn load_report(
        &self,
        execution_id: Uuid,
    ) -> Result<Option<ValidationReport>, ValidationLoopError>;

    /// List all validation reports for a given template ID.
    ///
    /// Returns reports ordered by `created_at` descending (newest first).
    async fn list_by_template(
        &self,
        template_id: &str,
        limit: u32,
    ) -> Result<Vec<ValidationReport>, ValidationLoopError>;

    /// List the most recent validation reports.
    ///
    /// Returns up to `limit` reports ordered by `created_at` descending.
    async fn list_recent(&self, limit: u32) -> Result<Vec<ValidationReport>, ValidationLoopError>;

    /// Delete a validation report by execution ID.
    ///
    /// Returns `Ok(true)` if a report was deleted, `Ok(false)` if
    /// no report existed for this execution ID.
    async fn delete_report(&self, execution_id: Uuid) -> Result<bool, ValidationLoopError>;

    /// Get the total number of stored validation reports.
    async fn count(&self) -> Result<u64, ValidationLoopError>;

    /// Check if a report exists for the given execution ID.
    async fn exists(&self, execution_id: Uuid) -> Result<bool, ValidationLoopError>;
}

// ---------------------------------------------------------------------------
// ValidatedTemplateRepository
// ---------------------------------------------------------------------------

/// Repository for caching and retrieving validated templates.
///
/// Stores validated templates so that they can be reused without
/// re-running the full validation loop. Supports cache invalidation
/// and intent-based lookup.
///
/// A validated template has passed the validation loop with all
/// quality gates satisfied. Its `llm_generate` prompt has been
/// refined through the validation process and is considered
/// production-grade.
///
/// # Contract (Frozen)
/// - Templates are indexed by their intent hash for deterministic lookup
/// - Cache entries have a configurable TTL (default: no expiry)
/// - Lookup by intent returns the most recently validated template
/// - All methods are async and return domain error types
#[async_trait]
pub trait ValidatedTemplateRepository: Send + Sync {
    /// Save a validated template for future reuse.
    ///
    /// If a template with the same intent_hash already exists, it is
    /// overwritten with the new (more recently validated) template.
    ///
    /// # Arguments
    ///
    /// * `intent_hash` — Deterministic hash of the user intent.
    /// * `template` — The validated template to cache.
    /// * `reusable_prompt` — The refined llm_generate prompt, if available.
    async fn save(
        &self,
        intent_hash: &str,
        template: &Template,
        reusable_prompt: Option<&str>,
    ) -> Result<(), ValidationLoopError>;

    /// Load a validated template by its intent hash.
    ///
    /// Returns `None` if no cached template exists for this hash.
    /// Returns `None` if the cache entry has expired (TTL exceeded).
    async fn load_by_intent_hash(
        &self,
        intent_hash: &str,
    ) -> Result<Option<Template>, ValidationLoopError>;

    /// Load a validated template by its template ID.
    ///
    /// Returns the most recently validated version of this template.
    async fn load_by_template_id(
        &self,
        template_id: &str,
    ) -> Result<Option<Template>, ValidationLoopError>;

    /// Get the reusable prompt for a validated template.
    ///
    /// Returns the refined llm_generate prompt that can be used
    /// directly without re-running the validation loop.
    async fn get_reusable_prompt(
        &self,
        intent_hash: &str,
    ) -> Result<Option<String>, ValidationLoopError>;

    /// Delete a cached validated template entry.
    ///
    /// Returns `Ok(true)` if an entry was deleted, `Ok(false)` if no
    /// entry existed for the given intent hash.
    async fn delete(&self, intent_hash: &str) -> Result<bool, ValidationLoopError>;

    /// Clear all cached validated templates.
    async fn clear_cache(&self) -> Result<(), ValidationLoopError>;

    /// Get the number of cached validated templates.
    async fn cache_size(&self) -> Result<u64, ValidationLoopError>;

    /// Check if a cached template exists for the given intent hash.
    async fn exists(&self, intent_hash: &str) -> Result<bool, ValidationLoopError>;
}
