//! CompilerOutput — raw compiler/test output wrapper.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#input
//! Implements: Contract Freeze — CompilerOutput struct
//! Issue: #495
//!
//! # Contract (Frozen)
//! - Wraps the raw stdout/stderr from a tool execution
//! - Carries exit code and tool metadata for context-aware parsing
//! - Serialization support for eventing and API responses

use serde::{Deserialize, Serialize};

/// Raw compiler or test runner output to be parsed.
///
/// This is the input to `FailureParserService::parse()`. It wraps
/// the raw stdout/stderr along with metadata about the execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerOutput {
    /// The raw stdout from the tool execution.
    pub stdout: String,

    /// The raw stderr from the tool execution (may be empty).
    pub stderr: String,

    /// The process exit code.
    pub exit_code: i32,

    /// The tool that produced this output (e.g., "tsc", "jest", "rustc", "pytest").
    pub tool: String,

    /// Working directory where the tool was executed (for resolving relative paths).
    pub working_directory: String,
}

impl CompilerOutput {
    /// Create a new CompilerOutput.
    pub fn new(
        stdout: impl Into<String>,
        stderr: impl Into<String>,
        exit_code: i32,
        tool: impl Into<String>,
        working_directory: impl Into<String>,
    ) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_code,
            tool: tool.into(),
            working_directory: working_directory.into(),
        }
    }

    /// Returns `true` if the exit code indicates failure (non-zero).
    pub fn is_failure(&self) -> bool {
        self.exit_code != 0
    }

    /// Returns the combined stdout + stderr output.
    pub fn combined(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }

    /// Returns `true` if both stdout and stderr are empty.
    pub fn is_empty(&self) -> bool {
        self.stdout.is_empty() && self.stderr.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_output_is_failure() {
        let output = CompilerOutput::new("ok", "", 0, "tsc", "/project");
        assert!(!output.is_failure());

        let output = CompilerOutput::new("", "error", 1, "tsc", "/project");
        assert!(output.is_failure());
    }

    #[test]
    fn test_compiler_output_combined() {
        let output = CompilerOutput::new("stdout", "stderr", 1, "tsc", "/project");
        assert_eq!(output.combined(), "stdout\nstderr");
    }

    #[test]
    fn test_compiler_output_combined_stdout_only() {
        let output = CompilerOutput::new("stdout", "", 0, "tsc", "/project");
        assert_eq!(output.combined(), "stdout");
    }

    #[test]
    fn test_compiler_output_combined_stderr_only() {
        let output = CompilerOutput::new("", "stderr", 1, "tsc", "/project");
        assert_eq!(output.combined(), "stderr");
    }

    #[test]
    fn test_compiler_output_is_empty() {
        let output = CompilerOutput::new("", "", 0, "tsc", "/project");
        assert!(output.is_empty());

        let output = CompilerOutput::new("x", "", 0, "tsc", "/project");
        assert!(!output.is_empty());
    }
}
