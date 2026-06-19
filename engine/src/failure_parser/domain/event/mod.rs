//! Event payload schemas for the Failure Parser bounded context.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — FailureParserEvent payload schemas
//! Issue: #495
//!
//! These events are emitted on the `EventBus` whenever failures are parsed,
//! suggestions are generated, or parsing errors occur. Consumers
//! (audit, console printer, TUI) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};

use crate::failure_parser::domain::{ParsedFailure, TemplateFailure};

/// Events emitted by the Failure Parser module.
///
/// Wrapped in `ExecutionEvent::FailureParser(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureParserEvent {
    /// Raw compiler/test output was successfully parsed into structured failures.
    OutputParsed {
        /// The tool that produced the output (e.g., "tsc", "jest", "rustc").
        tool: String,
        /// The complete parsed result.
        parsed: ParsedFailure,
        /// Brief human-readable summary of the parse result.
        summary: String,
    },

    /// The parser could not parse the output for a specific tool.
    ParseFailed {
        /// The tool that produced the output.
        tool: String,
        /// Why parsing failed.
        reason: String,
        /// Whether the parser fell back to a generic classification.
        fell_back_to_default: bool,
    },

    /// A suggested fix was generated for a failure.
    FixGenerated {
        /// The failure the fix was generated for.
        failure: TemplateFailure,
        /// The generated fix suggestion.
        suggestion: String,
        /// The source tool.
        tool: String,
        /// Confidence in the fix suggestion (0.0–1.0).
        confidence: f64,
    },

    /// Source context was built for suggestion generation.
    SourceContextBuilt {
        /// The tool being parsed.
        tool: String,
        /// Number of files scanned.
        files_scanned: usize,
        /// Number of symbols extracted.
        symbols_found: usize,
        /// Whether the context was truncated for performance.
        truncated: bool,
    },

    /// A new parser was registered in the parser registry.
    ParserRegistered {
        /// The tool name the parser handles.
        tool: String,
        /// Number of parsers now in the registry.
        total_parsers: usize,
    },
}
