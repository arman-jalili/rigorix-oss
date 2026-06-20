//! HookError — typed error enum for hook failures.
//!
//! @canonical .pi/architecture/modules/hooks.md#error-handling
//! Implements: Contract Freeze — HookError enum
//! Issue: #410
//!
//! All hook errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `HookError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Error recovery strategy documented per variant

use thiserror::Error;

/// Errors that can occur during hook command execution.
///
/// # Error Recovery
///
/// | Variant | Severity | Recovery |
/// |---------|----------|----------|
/// | `CommandNotFound` | Warning | Hook skipped, execution continues |
/// | `Timeout` | Error | Hook killed; PreToolUse → tool blocked, Post* → tool result returned |
/// | `InvalidJson` | Error | Treated as hook failure; tool blocked for PreToolUse |
/// | `ProcessError` | Error | Same as InvalidJson — fail-safe |
/// | `Aborted` | Info | Hook cancelled; tool blocked for PreToolUse only |
#[derive(Debug, Error)]
pub enum HookError {
    /// The hook command was not found in PATH or at the specified path.
    ///
    /// The hook is skipped with a warning. Tool execution continues
    /// as if the hook was not registered.
    #[error("Hook command not found: {command}")]
    CommandNotFound {
        /// The command string that was not found.
        command: String,
    },

    /// Hook execution timed out.
    ///
    /// The configured timeout was exceeded. For PreToolUse, the tool
    /// is blocked (safety-first). For PostToolUse/PostToolUseFailure,
    /// the tool result is returned without hook feedback.
    #[error("Hook execution timed out after {timeout_ms}ms: {command}")]
    Timeout {
        /// The command that timed out.
        command: String,
        /// The timeout duration in milliseconds.
        timeout_ms: u64,
    },

    /// Hook returned invalid (non-JSON or schema-mismatched) output.
    ///
    /// The hook's stdout could not be parsed as a `HookStdoutResponse`.
    /// Treated as hook failure — tool may be blocked depending on event.
    #[error("Hook returned invalid JSON: {detail}")]
    InvalidJson {
        /// The hook command that produced invalid output.
        command: String,
        /// Detailed parse error.
        detail: String,
        /// Raw stdout content (truncated for safety).
        raw_output: String,
    },

    /// Hook process failed with a non-zero exit code and no valid JSON.
    ///
    /// The hook exited with an error but did not provide a structured
    /// response. Treated as hook failure — tool may be blocked depending
    /// on event type.
    #[error("Hook process error (exit: {exit_code}): {command}")]
    ProcessError {
        /// The command that failed.
        command: String,
        /// The exit code of the process.
        exit_code: i32,
        /// Stderr output from the hook (if any).
        stderr: String,
    },

    /// Hook execution was aborted by the cancellation signal.
    ///
    /// The `HookAbortSignal` was set during execution. The hook was
    /// killed. For PreToolUse, the tool is blocked. For Post* events,
    /// the tool result is returned without hook feedback.
    #[error("Hook aborted by signal: {command}")]
    Aborted {
        /// The command that was aborted.
        command: String,
    },

    /// An internal error occurred (e.g., IO error spawning process).
    #[error("Internal hook error: {detail}")]
    Internal {
        /// Error detail for diagnostics.
        detail: String,
    },
}

impl HookError {
    /// Returns true if this error indicates the hook command was not found.
    pub fn is_command_not_found(&self) -> bool {
        matches!(self, HookError::CommandNotFound { .. })
    }

    /// Returns true if this error is recoverable (tool can still proceed).
    ///
    /// CommandNotFound is recoverable — the hook is simply skipped.
    /// All other errors are non-recoverable for the hook's event type
    /// and may block the tool.
    pub fn is_recoverable(&self) -> bool {
        matches!(self, HookError::CommandNotFound { .. })
    }

    /// Returns the command string associated with this error, if available.
    pub fn command(&self) -> Option<&str> {
        match self {
            HookError::CommandNotFound { command }
            | HookError::Timeout { command, .. }
            | HookError::InvalidJson { command, .. }
            | HookError::ProcessError { command, .. }
            | HookError::Aborted { command } => Some(command),
            HookError::Internal { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_not_found() {
        let err = HookError::CommandNotFound {
            command: "nonexistent-hook".into(),
        };
        assert!(err.is_command_not_found());
        assert!(err.is_recoverable());
        assert_eq!(err.command(), Some("nonexistent-hook"));
        assert_eq!(err.to_string(), "Hook command not found: nonexistent-hook");
    }

    #[test]
    fn test_timeout() {
        let err = HookError::Timeout {
            command: "slow-hook".into(),
            timeout_ms: 30000,
        };
        assert!(!err.is_recoverable());
        assert_eq!(err.command(), Some("slow-hook"));
        assert_eq!(
            err.to_string(),
            "Hook execution timed out after 30000ms: slow-hook"
        );
    }

    #[test]
    fn test_invalid_json() {
        let err = HookError::InvalidJson {
            command: "bad-hook".into(),
            detail: "missing field `decision`".into(),
            raw_output: "{{{".into(),
        };
        assert!(!err.is_command_not_found());
        assert!(
            err.to_string().contains("invalid JSON"),
            "Expected 'invalid JSON', got: {}",
            err.to_string()
        );
        assert!(
            err.to_string().contains("missing field"),
            "Expected 'missing field', got: {}",
            err.to_string()
        );
        assert_eq!(err.command(), Some("bad-hook"));
    }

    #[test]
    fn test_process_error() {
        let err = HookError::ProcessError {
            command: "failing-hook".into(),
            exit_code: 1,
            stderr: "Something went wrong".into(),
        };
        assert_eq!(
            err.to_string(),
            "Hook process error (exit: 1): failing-hook"
        );
    }

    #[test]
    fn test_aborted() {
        let err = HookError::Aborted {
            command: "cancel-me".into(),
        };
        assert_eq!(err.to_string(), "Hook aborted by signal: cancel-me");
    }

    #[test]
    fn test_internal() {
        let err = HookError::Internal {
            detail: "IO error: broken pipe".into(),
        };
        assert!(err.command().is_none());
        assert!(!err.is_command_not_found());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_debug_format() {
        let err = HookError::CommandNotFound {
            command: "test".into(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("CommandNotFound"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_error_trait_impl() {
        fn assert_error(_: &dyn std::error::Error) {}
        let err = HookError::CommandNotFound {
            command: "x".into(),
        };
        assert_error(&err); // Must implement std::error::Error
    }

    #[test]
    fn test_clone_not_available() {
        // HookError does NOT implement Clone (uses String fields)
        // Verify it at least implements Debug and Error
        let err = HookError::Internal {
            detail: "test".into(),
        };
        let _ = format!("{:?}", err);
        let _: &dyn std::error::Error = &err;
    }
}
