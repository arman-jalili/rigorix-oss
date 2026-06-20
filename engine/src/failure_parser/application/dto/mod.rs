//! Data Transfer Objects for the Failure Parser module.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — DTO schemas for failure parsing
//! Issue: #495
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::failure_parser::domain::{
    ParsedFailure, SourceContext, detail::FailureDetail, failure::TemplateFailure,
};

// ---------------------------------------------------------------------------
// Parse Output DTOs
// ---------------------------------------------------------------------------

/// Input for parsing raw compiler/test output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseOutputInput {
    /// The tool that produced the output (e.g., "tsc", "jest", "rustc", "pytest").
    /// Must be a known tool with a registered parser.
    pub tool: String,

    /// The raw stdout from the tool execution.
    pub stdout: String,

    /// The raw stderr from the tool execution (may be empty).
    pub stderr: String,

    /// The process exit code. Non-zero indicates failure.
    pub exit_code: i32,

    /// Available source context for suggestion generation.
    pub source_context: SourceContext,

    /// Working directory where the tool was executed.
    /// Used for resolving relative paths in error output.
    pub working_directory: String,
}

/// Result from parsing raw compiler/test output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseOutputResult {
    /// The complete parsed result with all failures.
    pub parsed: ParsedFailure,

    /// Human-readable analysis summary for LLM consumption.
    pub llm_summary: String,

    /// Whether the output was successfully parsed.
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Format for LLM DTOs
// ---------------------------------------------------------------------------

/// Input for formatting failures into LLM-readable summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatForLlmInput {
    /// The failures to format.
    pub failures: Vec<TemplateFailure>,

    /// Optional title for the analysis block.
    pub title: Option<String>,
}

/// Output from formatting failures for LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatForLlmOutput {
    /// The formatted analysis string.
    pub formatted: String,

    /// Total failures included in the analysis.
    pub count: usize,
}

// ---------------------------------------------------------------------------
// Suggest Fix DTOs
// ---------------------------------------------------------------------------

/// Input for generating a suggested fix for a failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestFixInput {
    /// The failure to generate a fix for.
    pub failure: TemplateFailure,

    /// Available source context for generating suggestions.
    pub source_context: SourceContext,

    /// Minimum confidence threshold (0.0–1.0) for returning a suggestion.
    /// Suggestions below this threshold are returned as None.
    #[serde(default = "default_confidence_threshold")]
    pub min_confidence: f64,
}

fn default_confidence_threshold() -> f64 {
    0.5
}

/// Output from generating a suggested fix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuggestFixOutput {
    /// The generated fix suggestion, if one could be determined.
    pub suggestion: Option<String>,

    /// Confidence in the suggestion (0.0–1.0).
    pub confidence: f64,

    /// Why this suggestion was chosen (for traceability).
    pub rationale: Option<String>,
}

// ---------------------------------------------------------------------------
// Register Parser DTOs
// ---------------------------------------------------------------------------

/// Input for registering a new parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterParserInput {
    /// The tool name this parser handles.
    pub tool: String,
    /// Human-readable description of the parser.
    pub description: String,
}

/// Result from registering a new parser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterParserResult {
    /// Whether the registration was successful.
    pub success: bool,
    /// Total number of parsers now in the registry.
    pub total_parsers: usize,
    /// Human-readable message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Parser Metadata DTOs
// ---------------------------------------------------------------------------

/// Metadata about a registered parser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParserMetadata {
    /// The tool name this parser handles.
    pub tool: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this is a built-in parser.
    pub is_builtin: bool,
}

/// Result from listing available parsers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListParsersResult {
    /// Available parsers.
    pub parsers: Vec<ParserMetadata>,
    /// Total count.
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Classify Failure DTOs
// ---------------------------------------------------------------------------

/// Input for classifying a failure's severity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifySeverityInput {
    /// The failures to classify.
    pub failures: Vec<FailureDetail>,
}

/// Output from classifying failure severity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifySeverityOutput {
    /// The computed overall severity.
    pub overall_severity: String,
    /// Number of compile-blocking errors.
    pub compile_blocks: usize,
    /// Number of test-blocking errors.
    pub test_blocks: usize,
    /// Number of warnings.
    pub warnings: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_output_input_serialization() {
        let input = ParseOutputInput {
            tool: "tsc".into(),
            stdout: "error TS2339: ...".into(),
            stderr: String::new(),
            exit_code: 2,
            source_context: SourceContext::empty(),
            working_directory: "/project".into(),
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: ParseOutputInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool, "tsc");
        assert_eq!(deserialized.exit_code, 2);
    }

    #[test]
    fn test_default_confidence_threshold() {
        let input = SuggestFixInput {
            failure: TemplateFailure::MissingSymbol {
                symbol: "x".into(),
                available: vec![],
                suggestion: None,
                location: crate::failure_parser::domain::failure::SourceLocation::new(
                    "test.ts", 1, None,
                ),
            },
            source_context: SourceContext::empty(),
            min_confidence: 0.5,
        };
        assert_eq!(input.min_confidence, 0.5);
    }

    #[test]
    fn test_suggest_fix_output_serialization() {
        let output = SuggestFixOutput {
            suggestion: Some("Use 'add' instead".into()),
            confidence: 0.95,
            rationale: Some("Exact substring match".into()),
        };
        let json = serde_json::to_string(&output).unwrap();
        let deserialized: SuggestFixOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.suggestion, Some("Use 'add' instead".into()));
        assert!((deserialized.confidence - 0.95).abs() < 1e-10);
    }
}
