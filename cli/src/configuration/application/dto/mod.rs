//! Data Transfer Objects for the CLI Configuration module.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — CLI config DTO schemas
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for CLI configuration operations.
//! They are used by the `CliConfigLoader` trait and related services.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for CI/CD output)
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config Load DTOs
// ---------------------------------------------------------------------------

/// Input for loading configuration from an explicit path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfigInput {
    /// Path to the config file to load.
    pub path: String,
}

/// Output from a configuration load operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfigOutput {
    /// Path to the config file that was loaded (if any).
    pub config_path: Option<String>,
    /// The primary source of configuration values.
    pub primary_source: ConfigSource,
    /// List of all sources that contributed.
    pub sources_used: Vec<ConfigSource>,
    /// Whether an API key was found in any source.
    pub api_key_configured: bool,
}

/// Describes the source of a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigSource {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "env")]
    Environment,
    #[serde(rename = "flags")]
    CliFlags,
}

// ---------------------------------------------------------------------------
// Config Validation DTOs
// ---------------------------------------------------------------------------

/// Input for validating the current configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigInput {
    /// Whether to check for the API key.
    pub check_api_key: bool,
    /// The command that will be executed (for context-specific validation).
    pub command: Option<String>,
}

impl Default for ValidateConfigInput {
    fn default() -> Self {
        Self {
            check_api_key: true,
            command: None,
        }
    }
}

/// Output from configuration validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigOutput {
    /// Whether the configuration is valid.
    pub valid: bool,
    /// List of validation errors (empty if valid).
    pub errors: Vec<String>,
    /// List of warnings (non-blocking).
    pub warnings: Vec<String>,
    /// Whether an API key is configured.
    pub api_key_configured: bool,
}
