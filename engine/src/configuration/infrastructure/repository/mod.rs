//! Repository interfaces for the Configuration bounded context.
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use filesystem, environment, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::configuration::domain::ConfigurationError;

use super::super::application::dto::ConfigDto;

/// Repository for reading configuration sources.
///
/// Implementations handle reading from filesystem (TOML files),
/// environment variables, or other config sources.
///
/// # Security
/// - Implementations MUST redact secrets in all log output
/// - File paths must be validated against directory traversal
#[async_trait]
pub trait ConfigRepository: Send + Sync {
    /// Read a TOML configuration file.
    ///
    /// Returns the raw TOML content as a string for parsing by `ConfigFactory`.
    /// Returns `ConfigurationError::NotFound` if the file doesn't exist.
    async fn read_toml_file(&self, path: &str) -> Result<String, ConfigurationError>;

    /// Resolve the configuration file path.
    ///
    /// Checks the explicit path first, then CWD (`rigorix.toml`),
    /// then home directory (`~/.rigorix/config.toml`).
    /// Returns the first found path or `None`.
    async fn resolve_config_path(&self, explicit_path: Option<&str>) -> Option<String>;

    /// Read all environment variables matching a prefix.
    ///
    /// Returns a map of variable name (without prefix) to value.
    /// E.g. `RIGORIX__LOGGING__LEVEL=debug` returns `{"logging.level": "debug"}`.
    async fn read_env_vars(&self, prefix: &str) -> std::collections::HashMap<String, String>;

    /// Read a single environment variable.
    ///
    /// Returns `None` if the variable is not set.
    async fn read_env_var(&self, name: &str) -> Option<String>;
}

/// Repository for persisting resolved configuration.
///
/// Optional — only needed if runtime config persistence is required.
#[async_trait]
pub trait ConfigWriteRepository: Send + Sync {
    /// Write resolved configuration to a cache file.
    ///
    /// Uses atomic write-rename pattern (.tmp → rename) for crash safety.
    async fn write_cached(&self, config: &ConfigDto) -> Result<(), ConfigurationError>;

    /// Read cached configuration.
    ///
    /// Returns `None` if no cache exists or cache is stale.
    async fn read_cached(&self) -> Result<Option<ConfigDto>, ConfigurationError>;

    /// Invalidate the cached configuration.
    async fn invalidate_cache(&self) -> Result<(), ConfigurationError>;
}
