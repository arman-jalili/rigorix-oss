//! Service interfaces (use cases) for the Configuration bounded context.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — ConfigService and SecretService traits
//! Issue: #2
//!
//! These traits define the application-level operations that can be performed
//! on configuration and secrets. All methods are async and return domain
//! error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::configuration::domain::ConfigurationError;

use super::dto::{
    LoadConfigInput, LoadConfigOutput, LoadSecretInput, LoadSecretOutput, ValidateConfigInput,
    ValidateConfigOutput,
};

/// Application service for loading, validating, and managing configuration.
///
/// Implementations handle multi-source loading with layered merging
/// (CLI flags > ENV vars > rigorix.toml > ~/.rigorix/config.toml > defaults).
#[async_trait]
pub trait ConfigService: Send + Sync {
    /// Load configuration from all available sources with layered merging.
    ///
    /// Priority (highest wins):
    /// 1. CLI flag overrides (input.cli_overrides)
    /// 2. Environment variables (RIGORIX__*)
    /// 3. rigorix.toml in CWD
    /// 4. ~/.rigorix/config.toml (fallback)
    /// 5. Compiled-in defaults
    async fn load(&self, input: LoadConfigInput) -> Result<LoadConfigOutput, ConfigurationError>;

    /// Validate configuration against safety hard-caps.
    ///
    /// Checks that all values are within acceptable bounds defined by
    /// `SafetyCaps`. Returns structured validation errors for each
    /// violating field.
    async fn validate(
        &self,
        input: ValidateConfigInput,
    ) -> Result<ValidateConfigOutput, ConfigurationError>;

    /// Reload configuration at runtime (only supported sources).
    ///
    /// Returns the new config on success, or an error if reload is not
    /// supported by the current configuration source.
    async fn reload(&self) -> Result<LoadConfigOutput, ConfigurationError>;
}

/// Application service for loading and managing secrets (API keys, tokens).
///
/// Implementations read from environment variables with optional fallback
/// values. Secrets are wrapped in `Secret` type for redacted output.
#[async_trait]
pub trait SecretService: Send + Sync {
    /// Load a secret from an environment variable.
    ///
    /// Returns the secret wrapped in the `Secret` type. If the env var
    /// is not set and a fallback is provided, uses the fallback.
    /// Emits `SecretLoaded` event on successful load.
    async fn load(&self, input: LoadSecretInput) -> Result<LoadSecretOutput, ConfigurationError>;

    /// Validate that all required secrets are available.
    ///
    /// Returns a list of missing required secret names.
    /// Implementations should check env vars without revealing values.
    async fn validate_required(
        &self,
        required_keys: Vec<String>,
    ) -> Result<Vec<String>, ConfigurationError>;
}
