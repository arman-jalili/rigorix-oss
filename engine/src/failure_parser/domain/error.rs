//! Failure parser error types.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — FailureParserError enum
//! Issue: #495
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `FailureParserError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during failure parsing.
#[derive(Debug, Error)]
pub enum FailureParserError {
    /// The raw output could not be parsed as expected (no matching patterns).
    #[error("Parse failed for tool '{tool}': {reason}")]
    ParseError {
        /// The tool whose output failed to parse (e.g., "tsc", "jest").
        tool: String,
        /// Why parsing failed.
        reason: String,
        /// A snippet of the raw output that could not be parsed.
        output_snippet: Option<String>,
    },

    /// The input format is not supported or could not be recognized.
    #[error("Unrecognized output format: {detail}")]
    UnrecognizedFormat {
        /// Details about why the format is unrecognized.
        detail: String,
        /// The tool that produced the output.
        tool: String,
    },

    /// The language/tool parser is not registered in the parser registry.
    #[error("No parser registered for tool: {tool}")]
    UnsupportedTool {
        /// The tool name that has no registered parser.
        tool: String,
        /// List of available parsers.
        available: Vec<String>,
    },

    /// Missing or empty output — nothing to parse.
    #[error("No output to parse for tool '{tool}'")]
    EmptyOutput {
        /// The tool that produced empty output.
        tool: String,
    },

    /// The source context could not be built (symbol extraction failed).
    #[error("Source context error: {detail}")]
    SourceContextError {
        /// Details about the source context error.
        detail: String,
    },

    /// A suggestion could not be generated.
    #[error("Suggestion generation failed: {detail}")]
    SuggestionFailed {
        /// Details about why suggestion generation failed.
        detail: String,
    },
}

impl FailureParserError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            FailureParserError::ParseError { .. } | FailureParserError::SourceContextError { .. }
        )
    }
}
