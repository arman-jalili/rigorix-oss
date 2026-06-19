//! Repository interfaces for the Hook System.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: Contract Freeze — HookCommandRepository trait
//! Issue: #410
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use various storage backends (filesystem TOML,
//! database, in-memory) without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::hooks::domain::config::HookConfig;
use crate::hooks::domain::error::HookError;

/// Repository for persisting and retrieving hook command configurations.
///
/// Provides access to hook command lists stored in configuration sources
/// (e.g., `.rigorix/hooks.toml`, database, environment variables).
///
/// # Security
/// - Implementations MUST validate command paths against allowed directories
/// - All inputs must be validated against size limits
#[async_trait]
pub trait HookCommandRepository: Send + Sync {
    /// Load hook configuration from the default source.
    ///
    /// Returns the parsed `HookConfig` or an error if loading fails.
    /// If no configuration source exists, returns an empty `HookConfig`.
    async fn load(&self) -> Result<HookConfig, HookError>;

    /// Load hook configuration from a specific path.
    ///
    /// Useful for testing or when configuration is in a non-default location.
    async fn load_from(&self, path: &str) -> Result<HookConfig, HookError>;

    /// Save hook configuration to the default source.
    ///
    /// Persists the provided `HookConfig` to the configuration storage.
    async fn save(&self, config: &HookConfig) -> Result<(), HookError>;

    /// Check whether a hook configuration source exists.
    async fn exists(&self) -> Result<bool, HookError>;

    /// Reset hook configuration to defaults (empty).
    ///
    /// Removes all registered hook commands.
    async fn reset(&self) -> Result<(), HookError>;

    /// Get the source path of the hook configuration.
    fn source_path(&self) -> &str;
}
