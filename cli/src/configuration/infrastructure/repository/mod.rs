//! Repository interfaces for the CLI Configuration module.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — ConfigCliRepository trait
//! Issue: issue-contract-freeze
//!
//! Repositories abstract CLI-level configuration data storage behind interfaces,
//! allowing implementations to use in-memory caching, filesystem persistence,
//! or mock storage without coupling CLI logic to infrastructure.
//!
//! These repositories handle CLI-level concerns (cached config values, user
//! preferences for output format). They are distinct from the engine's config
//! repositories which handle full configuration persistence.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::configuration::domain::ConfigCliError;
use crate::configuration::domain::config::CliConfig;

/// Repository for CLI-level configuration data.
///
/// Handles caching and persistence of CLI configuration values.
/// This is separate from the engine's configuration system and is
/// used for persisting user preferences and CLI state across sessions.
///
/// # Contract (Frozen)
/// - Read operations return cached data or delegate to the engine
/// - All methods are safe to call concurrently
#[async_trait]
pub trait ConfigCliRepository: Send + Sync {
    /// Store the current CLI configuration.
    async fn store_config(&self, config: CliConfig) -> Result<(), ConfigCliError>;

    /// Retrieve the stored CLI configuration.
    async fn get_config(&self) -> Result<Option<CliConfig>, ConfigCliError>;

    /// Store a single configuration override for the next session.
    async fn store_override(&self, key: &str, value: &str) -> Result<(), ConfigCliError>;

    /// Retrieve all stored overrides.
    async fn get_overrides(&self) -> Result<Vec<(String, String)>, ConfigCliError>;

    /// Clear all stored configuration and overrides.
    async fn clear(&self) -> Result<(), ConfigCliError>;

    /// Get the list of config file paths that have been used.
    async fn get_searched_paths(&self) -> Result<Vec<String>, ConfigCliError>;
}
