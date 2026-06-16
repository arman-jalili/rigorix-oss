//! Repository interfaces for the CLI Templates module.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — TemplateCliRepository trait
//! Issue: issue-contract-freeze
//!
//! Repositories abstract CLI-level template data storage behind interfaces,
//! allowing implementations to use in-memory caching, filesystem persistence,
//! or mock storage without coupling CLI logic to infrastructure.
//!
//! These repositories handle CLI-level concerns (cached summaries, user
//! preferences for template display). They are distinct from the engine's
//! `TemplateRepository` which handles template source file storage.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::templates::domain::TemplateCliError;

use crate::templates::application::dto::TemplateSummary;

/// Repository for CLI-level template data.
///
/// Handles caching and persistence of template summaries at the CLI
/// layer. This is separate from the engine's source template repository
/// and is used for optimizing repeated list/show operations.
///
/// # Contract (Frozen)
/// - Read operations return cached data or delegate to the engine
/// - Cache invalidation is explicit (not time-based)
/// - All methods are safe to call concurrently
#[async_trait]
pub trait TemplateCliRepository: Send + Sync {
    /// Store a template summary in the repository.
    ///
    /// Replaces any existing entry with the same template ID.
    async fn store_summary(&self, summary: TemplateSummary) -> Result<(), TemplateCliError>;

    /// Retrieve a stored template summary by ID.
    ///
    /// Returns `None` if no summary with that ID is cached.
    async fn get_summary(&self, id: &str) -> Result<Option<TemplateSummary>, TemplateCliError>;

    /// Retrieve all stored template summaries.
    ///
    /// Returns an empty vec if no summaries are cached.
    async fn list_summaries(&self) -> Result<Vec<TemplateSummary>, TemplateCliError>;

    /// Remove a template summary from the repository.
    ///
    /// Returns `true` if the entry existed and was removed.
    async fn remove_summary(&self, id: &str) -> Result<bool, TemplateCliError>;

    /// Clear all cached template summaries.
    async fn clear(&self) -> Result<(), TemplateCliError>;

    /// Get the number of cached template summaries.
    async fn count(&self) -> Result<usize, TemplateCliError>;
}
