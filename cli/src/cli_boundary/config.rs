//! Multi-source config loader — TOML + env vars + CLI flags → engine Config.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#config-loading-priority
//! Implements: Contract Freeze — ConfigLoader component
//! Issue: issue-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Config loading follows a layered priority (highest wins):
//!
//! 1. CLI flag overrides (from `--config-key value`)
//! 2. Environment variables (`RIGORIX_*`)
//! 3. `rigorix.toml` in CWD
//! 4. `~/.rigorix/config.toml` (fallback)
//! 5. Compiled-in engine defaults (lowest)
//!
//! The CLI loads and merges these sources, then passes the result to
//! `engine::configuration::ConfigService::load()`.

use serde::{Deserialize, Serialize};
use rigorix_engine::configuration::domain::config::Config;

use crate::cli_boundary::error::CliError;

// ---------------------------------------------------------------------------
// CLI-specific config wrapper
// ---------------------------------------------------------------------------

/// CLI-level configuration that merges with engine Config.
///
/// Contains CLI-specific settings (format, verbosity, repo_root) plus
/// overrides that feed into the engine's multi-source `Config` loading.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    /// Output format (Pretty, Json, Markdown, Quiet).
    pub format: super::cli::Format,

    /// Verbosity level (0 = default, 1 = debug, 2 = trace).
    pub verbose: u8,

    /// Repository root path for execution context.
    pub repo_root: String,

    /// CLI flag overrides that are merged before engine config loading.
    pub cli_overrides: std::collections::HashMap<String, serde_json::Value>,

    /// Resolved engine `Config` after multi-source merging.
    #[serde(skip)]
    pub engine_config: Option<Config>,
}

impl CliConfig {
    /// Returns a reference to the resolved engine Config, if available.
    pub fn engine_config(&self) -> Result<&Config, CliError> {
        self.engine_config
            .as_ref()
            .ok_or_else(|| CliError::Config("Engine config not loaded".into()))
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load and merge configuration from all sources.
///
/// Priority (highest wins):
/// 1. CLI flag overrides
/// 2. Environment variables (`RIGORIX_*`)
/// 3. `rigorix.toml` in CWD
/// 4. `~/.rigorix/config.toml` (fallback)
/// 5. Compiled-in defaults
///
/// # Returns
///
/// A fully resolved `CliConfig` containing both CLI-specific settings
/// and the merged engine `Config`.
///
/// # Errors
///
/// Returns `CliError::Config` if loading or validation fails (e.g.,
/// invalid TOML syntax, missing critical configuration).
pub fn load_config() -> CliConfig {
    // Placeholder: returns default config.
    // Implementation issue: implement TOML loading, env var reading,
    // layered merging, and engine ConfigService::load() integration.
    CliConfig::default()
}
