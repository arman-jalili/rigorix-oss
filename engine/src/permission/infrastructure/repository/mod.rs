//! Repository interfaces for the Permission Enforcer bounded context.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — repository interfaces for permission data
//! Issue: issue-contract-freeze
//!
//! Permission configuration is typically loaded from `.rigorix/permissions.toml`
//! at startup. For advanced use cases — runtime policy updates, per-session
//! permission overrides, or persistence of override state — repository
//! interfaces are provided.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;

use crate::permission::domain::{PermissionConfig, PermissionError};

/// Repository for loading and persisting permission configuration.
///
/// The default implementation loads from the `.rigorix/permissions.toml` file.
/// Custom implementations may load from a database, remote API, or
/// environment variables.
#[async_trait]
pub trait PermissionConfigRepository: Send + Sync {
    /// Load the permission configuration.
    ///
    /// Returns the full `PermissionConfig` including default mode,
    /// allow/deny/ask rules, and per-tool permission mappings.
    /// If no configuration is found, returns the default configuration.
    async fn load_config(&self) -> Result<PermissionConfig, PermissionError>;

    /// Save the permission configuration.
    ///
    /// Persists any runtime modifications to the permission config.
    /// If persistence is not supported, returns `Ok(())` without error.
    async fn save_config(&self, config: &PermissionConfig) -> Result<(), PermissionError>;

    /// Reload the configuration from the original source.
    ///
    /// Discards any in-memory overrides and reloads from the
    /// original configuration source.
    async fn reload_config(&self) -> Result<PermissionConfig, PermissionError>;
}
