//! Config loading interface for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CliConfigLoader trait
//! Issue: issue-contract-freeze
//!
//! Loads and merges CLI-specific configuration from multiple sources:
//! CLI flags → env vars (`RIGORIX_*`) → `rigorix.toml` → engine defaults.
//!
//! # Contract (Frozen)
//! - `load()` returns the fully merged `CliConfig`
//! - Merge order: CLI flags override env vars, which override file config,
//!   which override engine defaults
//! - Missing non-critical values use sensible defaults
//! - Missing critical values (e.g., API key) return `MissingConfig` error

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

/// Loads and merges CLI configuration from multiple sources.
#[async_trait]
pub trait CliConfigLoader: Send + Sync {
    /// Load configuration from all sources and merge.
    ///
    /// Merge order (later overrides earlier):
    /// 1. Engine defaults
    /// 2. `rigorix.toml` from cwd or `--config` path
    /// 3. Environment variables (`RIGORIX_*`)
    /// 4. CLI flags (passed via `cli_overrides`)
    ///
    /// Returns `CliError::ConfigNotFound` if no config file is found
    /// and no engine defaults apply.
    /// Returns `CliError::MissingConfig` if a required value is missing.
    async fn load(&self, cli_overrides: CliConfig) -> Result<CliConfig, CliError>;

    /// Load configuration from an explicit file path.
    ///
    /// Skips automatic config discovery and uses the given path.
    /// Still applies env var and CLI flag overrides on top.
    async fn load_from_path(
        &self,
        path: &str,
        cli_overrides: CliConfig,
    ) -> Result<CliConfig, CliError>;

    /// Check whether a configuration file exists at the default locations.
    async fn has_default_config(&self) -> bool;

    /// Get the list of config file paths that were searched.
    async fn searched_paths(&self) -> Vec<String>;
}
