//! CLI-specific domain errors.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#errors
//! Implements: Contract Freeze — CliError
//! Issue: issue-contract-freeze
//!
//! Errors that originate in the CLI layer before reaching the engine.
//! Engine errors are passed through as `CliError::Engine` variants.
//!
//! # Contract (Frozen)
//! - `CliError` is the single error type for the CLI boundary
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Maps to exit codes for CLI termination

use thiserror::Error;

/// Errors that can occur in the CLI boundary.
#[derive(Debug, Error)]
pub enum CliError {
    // -----------------------------------------------------------------------
    // Configuration Errors
    // -----------------------------------------------------------------------

    /// No configuration file found and no defaults could be applied.
    #[error("No configuration found: {detail}")]
    ConfigNotFound {
        /// Description of which config locations were searched.
        detail: String,
    },

    /// Configuration file exists but could not be parsed.
    #[error("Failed to parse configuration: {detail}")]
    ConfigParseError {
        /// Path to the config file that failed parsing.
        path: String,
        /// What went wrong during parsing.
        detail: String,
    },

    /// A required configuration value is missing (e.g., API key).
    #[error("Missing required configuration: {field}")]
    MissingConfig {
        /// The name of the missing configuration field.
        field: String,
        /// How to resolve this error (e.g., "set RIGORIX_API_KEY or add api_key to rigorix.toml").
        hint: String,
    },

    // -----------------------------------------------------------------------
    // Command Errors
    // -----------------------------------------------------------------------

    /// An unsupported or unknown command was provided.
    #[error("Unknown command: {command}")]
    UnknownCommand {
        /// The unrecognized command string.
        command: String,
        /// Suggestions for similar valid commands.
        suggestions: Vec<String>,
    },

    /// The command arguments are invalid.
    #[error("Invalid arguments for command '{command}': {detail}")]
    InvalidArguments {
        /// The command that received invalid arguments.
        command: String,
        /// What was wrong with the arguments.
        detail: String,
    },

    /// A required argument is missing.
    #[error("Missing required argument '{argument}' for command '{command}'")]
    MissingArgument {
        /// The command that is missing an argument.
        command: String,
        /// The name of the missing argument.
        argument: String,
    },

    // -----------------------------------------------------------------------
    // Execution Errors
    // -----------------------------------------------------------------------

    /// The execution session failed to start.
    #[error("Failed to start execution session: {detail}")]
    SessionStartFailed {
        /// What went wrong during session initialization.
        detail: String,
    },

    /// The execution session was cancelled by the user.
    #[error("Execution cancelled by user")]
    SessionCancelled,

    /// The execution session timed out.
    #[error("Execution session timed out after {timeout_secs}s")]
    SessionTimeout {
        /// The timeout duration in seconds.
        timeout_secs: u64,
    },

    // -----------------------------------------------------------------------
    // Output Errors
    // -----------------------------------------------------------------------

    /// Failed to render output in the requested format.
    #[error("Failed to render output: {detail}")]
    OutputRenderError {
        /// What went wrong during rendering.
        detail: String,
    },

    /// The terminal is not available for TUI rendering.
    #[error("Terminal not available for TUI: {detail}")]
    TerminalNotAvailable {
        /// Why the terminal is not available.
        detail: String,
    },

    // -----------------------------------------------------------------------
    // Signal Errors
    // -----------------------------------------------------------------------

    /// Failed to install the signal handler.
    #[error("Failed to install signal handler: {detail}")]
    SignalHandlerError {
        /// What went wrong during signal handler setup.
        detail: String,
    },

    // -----------------------------------------------------------------------
    // Engine Errors (passthrough)
    // -----------------------------------------------------------------------

    /// An error originating from the engine crate.
    ///
    /// This wraps the engine's `CoreOrchestratorError` and preserves
    /// the original error context. The error code is extracted from
    /// the engine error for consistent exit codes.
    #[error("Engine error: {0}")]
    Engine(#[from] rigorix_engine::error::CoreOrchestratorError),

    /// An unexpected internal error occurred.
    #[error("Unexpected internal error: {detail}")]
    Internal {
        /// Description of the internal error.
        detail: String,
    },
}

impl CliError {
    /// Returns the suggested exit code for this error.
    ///
    /// Convention:
    /// - 1: General error
    /// - 2: Configuration error
    /// - 3: Invalid command/arguments
    /// - 130: Cancelled (matches SIGINT convention)
    /// - 137: Killed (matches SIGKILL convention)
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::ConfigNotFound { .. }
            | CliError::ConfigParseError { .. }
            | CliError::MissingConfig { .. } => 2,

            CliError::UnknownCommand { .. }
            | CliError::InvalidArguments { .. }
            | CliError::MissingArgument { .. } => 3,

            CliError::SessionCancelled => 130,
            CliError::SessionTimeout { .. } => 137,

            CliError::Engine(_) => 1,
            _ => 1,
        }
    }

    /// Returns `true` if the error is retriable (the user can retry the same command).
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            CliError::ConfigNotFound { .. }
                | CliError::MissingConfig { .. }
                | CliError::MissingArgument { .. }
        )
    }
}
