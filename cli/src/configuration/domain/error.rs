//! CLI-specific configuration domain errors.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — ConfigCliError
//! Issue: issue-contract-freeze
//!
//! Errors that originate from CLI configuration operations (loading, merging,
//! validation). These are distinct from the engine's `ConfigurationError` —
//! they cover CLI-level concerns like file discovery, flag parsing, and
//! source merging.
//!
//! # Contract (Frozen)
//! - `ConfigCliError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Maps to `CliError` at the boundary layer

use thiserror::Error;

/// Errors that can occur during CLI configuration operations.
#[derive(Debug, Error)]
pub enum ConfigCliError {
    /// Configuration file discovery failed (no file found at expected paths).
    #[error("No configuration file found: {detail}")]
    DiscoveryFailed {
        /// Paths that were searched.
        detail: String,
    },

    /// A configuration file was found but could not be parsed.
    #[error("Failed to parse configuration file '{path}': {detail}")]
    ParseFailed {
        /// The path that failed parsing.
        path: String,
        /// What went wrong.
        detail: String,
    },

    /// A required configuration value is missing.
    #[error("Missing required configuration: {field}")]
    MissingValue {
        /// The name of the missing field.
        field: String,
        /// How to resolve the error.
        hint: String,
    },

    /// An environment variable had an invalid value.
    #[error("Invalid environment variable '{var}': {detail}")]
    InvalidEnvVar {
        /// The environment variable name.
        var: String,
        /// What was wrong with the value.
        detail: String,
    },

    /// Source merging detected a conflict that couldn't be resolved.
    #[error("Configuration merge conflict for '{field}'")]
    MergeConflict {
        /// The field with conflicting values.
        field: String,
        /// The conflicting values from different sources.
        sources: Vec<String>,
    },

    /// An unexpected internal error occurred.
    #[error("Internal configuration error: {detail}")]
    Internal {
        /// Description of the internal error.
        detail: String,
    },
}

impl ConfigCliError {
    /// Returns `true` if this error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ConfigCliError::DiscoveryFailed { .. } | ConfigCliError::MissingValue { .. }
        )
    }
}
