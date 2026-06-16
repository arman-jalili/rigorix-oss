//! Event payload schemas for the CLI Configuration module.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — ConfigCliEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted by the CLI configuration module whenever config
//! is loaded, merged, or encounters errors. Consumers (output formatters,
//! TUI, loggers) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - All events are serializable for logging and CI/CD output

use serde::{Deserialize, Serialize};

/// Events emitted by the CLI Configuration module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigCliEvent {
    /// Configuration loading started.
    ConfigLoadStarted {
        /// Paths being searched for config files.
        searched_paths: Vec<String>,
    },

    /// Configuration was successfully loaded and merged.
    ConfigLoaded {
        /// Source that provided the config (default, file, env, flags).
        primary_source: String,
        /// Path to the config file used (if any).
        config_path: Option<String>,
        /// Number of configuration sources that contributed.
        source_count: u32,
    },

    /// Configuration was loaded from an explicit path.
    ConfigLoadedFromPath {
        /// The path that was loaded.
        path: String,
    },

    /// Configuration loading failed.
    ConfigLoadFailed {
        /// Error message describing the failure.
        error: String,
        /// Paths that were searched.
        searched_paths: Vec<String>,
    },

    /// An environment variable was applied as a config override.
    EnvVarApplied {
        /// The environment variable name.
        var: String,
        /// The config field it mapped to.
        field: String,
    },

    /// A CLI flag was applied as a config override.
    CliFlagApplied {
        /// The flag name.
        flag: String,
        /// The config field it mapped to.
        field: String,
    },

    /// API key validation completed.
    ApiKeyValidated {
        /// Whether the API key is configured.
        configured: bool,
        /// The command that required the key.
        command: String,
    },
}
