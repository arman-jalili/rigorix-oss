//! CLI error type — maps all CLI errors to exit codes.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#exit-codes
//! Implements: Contract Freeze — CliError component
//! Issue: issue-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Exit code mapping:
//!
//! | Code | Meaning | Source |
//! |------|---------|--------|
//! | 0 | Success | All successful operations |
//! | 1 | General error / engine error | Engine operation failure |
//! | 2 | Configuration error | Config loading or validation |
//! | 3 | Invalid command or arguments | CLI parsing failure |
//! | 130 | Cancelled by user (Ctrl+C) | Signal handler / orchestrator cancel |
//! | 137 | Killed / timeout | Immediate abort / SIGTERM |

use std::fmt;

use rigorix_engine::configuration::domain::ConfigurationError;
use rigorix_engine::orchestrator::domain::OrchestratorError;

/// Root CLI error type mapping all failure modes to exit codes.
#[derive(Debug)]
pub enum CliError {
    /// General error — wraps engine errors and unknown failures.
    General(String),

    /// Configuration loading or validation failed.
    Config(String),

    /// Invalid CLI arguments or command syntax.
    InvalidArgs(String),

    /// Engine operation failed.
    Engine(OrchestratorError),

    /// Operation was cancelled by user (Ctrl+C).
    Cancelled,

    /// Process was killed or timed out.
    Killed,

    /// Feature not yet implemented (placeholder for contract freeze).
    NotImplemented(String),
}

impl CliError {
    /// Return the exit code for this error variant.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::General(_) => 1,
            CliError::Config(_) => 2,
            CliError::InvalidArgs(_) => 3,
            CliError::Engine(_) => 1,
            CliError::Cancelled => 130,
            CliError::Killed => 137,
            CliError::NotImplemented(_) => 1,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::General(msg) => write!(f, "Error: {msg}"),
            CliError::Config(msg) => write!(f, "Configuration error: {msg}"),
            CliError::InvalidArgs(msg) => write!(f, "Invalid arguments: {msg}"),
            CliError::Engine(err) => write!(f, "Engine error: {err}"),
            CliError::Cancelled => write!(f, "Cancelled by user"),
            CliError::Killed => write!(f, "Killed by signal"),
            CliError::NotImplemented(feature) => {
                write!(f, "Not implemented: {feature} (contract placeholder)")
            }
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CliError::Engine(err) => Some(err),
            _ => None,
        }
    }
}

impl From<OrchestratorError> for CliError {
    fn from(err: OrchestratorError) -> Self {
        CliError::Engine(err)
    }
}

impl From<ConfigurationError> for CliError {
    fn from(err: ConfigurationError) -> Self {
        CliError::Config(err.to_string())
    }
}

impl From<String> for CliError {
    fn from(msg: String) -> Self {
        CliError::General(msg)
    }
}

impl From<&str> for CliError {
    fn from(msg: &str) -> Self {
        CliError::General(msg.to_string())
    }
}
