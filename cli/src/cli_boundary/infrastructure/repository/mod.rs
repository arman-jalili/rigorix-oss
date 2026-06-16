//! Repository interfaces for CLI state persistence.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CliBoundaryRepository trait
//! Issue: issue-contract-freeze
//!
//! Repositories abstract CLI-level state persistence behind interfaces,
//! allowing implementations to use in-memory or filesystem storage.
//! In v1, all execution state is persisted by the engine's
//! `StatePersistenceService` — these interfaces are for CLI-specific
//! metadata (session caches, UI preferences, recent templates).
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;

/// Repository for CLI-level session metadata.
///
/// Handles caching and persistence of CLI session information,
/// recent commands, and UI preferences. All persistent execution
/// state is handled by the engine's StatePersistenceService.
///
/// # Contract (Frozen)
/// - Read operations return cached or persisted data
/// - All methods are safe to call concurrently
/// - Methods may return empty/default values if no data exists
///   (not an error)
#[async_trait]
pub trait CliBoundaryRepository: Send + Sync {
    /// Store a recent session ID with its command.
    async fn store_recent_session(&self, session_id: &str, command: &str) -> Result<(), CliError>;

    /// Get the most recent session IDs, newest first.
    async fn get_recent_sessions(&self, limit: usize) -> Result<Vec<(String, String)>, CliError>;

    /// Store a UI preference (key-value).
    async fn store_preference(&self, key: &str, value: &str) -> Result<(), CliError>;

    /// Get a UI preference by key.
    async fn get_preference(&self, key: &str) -> Result<Option<String>, CliError>;

    /// Clear all stored CLI metadata.
    async fn clear(&self) -> Result<(), CliError>;
}
