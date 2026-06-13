//! Factory interfaces for constructing Configuration domain objects.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — ConfigFactory and SecretFactory traits
//! Issue: #2
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::configuration::domain::{ConfigurationError, Secret};

use super::dto::ConfigDto;

/// Factory for constructing `Config` aggregates.
///
/// Implementations handle merging partial configurations from different
/// sources and applying default values for unset fields.
#[async_trait]
pub trait ConfigFactory: Send + Sync {
    /// Build a `ConfigDto` from a partial TOML configuration.
    ///
    /// Merges the provided partial config with defaults.
    /// Returns error on structural validation failure (e.g. unknown fields).
    async fn build_from_toml(&self, toml_content: &str) -> Result<ConfigDto, ConfigurationError>;

    /// Build a `ConfigDto` from environment variable overrides.
    ///
    /// Reads environment variables matching the given prefix (default `RIGORIX__`)
    /// and applies them as overrides to the provided base config.
    async fn apply_env_overrides(
        &self,
        base: ConfigDto,
        prefix: &str,
    ) -> Result<ConfigDto, ConfigurationError>;

    /// Build a `ConfigDto` from CLI flag overrides.
    ///
    /// Applies flat CLI override pairs (e.g. `orchestrator.max_parallel_tasks=8`)
    /// to the provided base config.
    async fn apply_cli_overrides(
        &self,
        base: ConfigDto,
        overrides: std::collections::HashMap<String, String>,
    ) -> Result<ConfigDto, ConfigurationError>;

    /// Create a `ConfigDto` with all default values.
    fn defaults(&self) -> ConfigDto;
}

/// Factory for constructing `Secret` values.
///
/// Implementations handle reading from environment variables and
/// applying fallback logic.
#[async_trait]
pub trait SecretFactory: Send + Sync {
    /// Load a secret from an environment variable.
    ///
    /// Returns `None` if the variable is not set and no fallback is provided.
    async fn load_from_env(&self, env_var: &str, fallback: Option<String>) -> Option<Secret>;

    /// Create a secret from a string value.
    ///
    /// Used for secrets that come from configuration files or CLI args.
    fn create_from_value(&self, value: impl Into<String>) -> Secret;
}
