//! Configuration error types.
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `ConfigurationError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during configuration loading and validation.
#[derive(Debug, Error)]
pub enum ConfigurationError {
    /// Configuration file not found at the expected path.
    #[error("Configuration file not found: {path} (source: {config_source:?})")]
    NotFound {
        /// The path that was searched.
        path: String,
        /// Additional context about where the lookup was attempted.
        config_source: ConfigSource,
    },

    /// Failed to parse the configuration file (TOML syntax errors).
    #[error("Failed to parse configuration: {detail}")]
    ParseError {
        /// Human-readable parse error description.
        detail: String,
        /// Source line number if available.
        line: Option<u32>,
    },

    /// Configuration is structurally valid but semantically invalid.
    #[error("Invalid configuration: {field}: {reason}")]
    InvalidConfig {
        /// The field that failed validation.
        field: String,
        /// Why the value is invalid.
        reason: String,
        /// The invalid value, if representable.
        value: Option<String>,
    },

    /// Environment variable parsing failed.
    #[error("Failed to parse environment variable {var}: {detail}")]
    EnvVarError {
        /// The environment variable name (e.g. "RIGORIX__LOGGING__LEVEL").
        var: String,
        /// Why the value could not be parsed.
        detail: String,
    },

    /// IO error when reading configuration files.
    #[error("IO error reading configuration: {io_error}")]
    Io {
        /// The underlying IO error.
        #[from]
        io_error: std::io::Error,
    },
}

/// Describes where a configuration source was expected.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// File in the current working directory.
    CwdFile,
    /// File in the user's home directory.
    HomeFile,
    /// Environment variable.
    EnvironmentVariable,
    /// CLI argument.
    CliArg,
    /// Compiled-in default.
    Default,
}
