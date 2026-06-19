//! HTTP API contracts for Failure Parser endpoints.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #495
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::failure_parser::application::dto::{
    ClassifySeverityInput, ClassifySeverityOutput, FormatForLlmInput, FormatForLlmOutput,
    ListParsersResult, ParseOutputInput, ParseOutputResult, ParserMetadata, RegisterParserInput,
    RegisterParserResult, SuggestFixInput, SuggestFixOutput,
};

use crate::failure_parser::domain::{
    FailureDetail,
    TemplateFailure,
    SourceContext,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All failure parser endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/failure-parser";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure-parser/parse
// ---------------------------------------------------------------------------

/// POST /api/v1/failure-parser/parse
///
/// Parse raw compiler/test output into structured failures.
///
/// **Request:** `ParseRequest`
/// **Response:** `200 OK` with `ParseResponse`
pub const PARSE_PATH: &str = "/api/v1/failure-parser/parse";
pub const PARSE_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure-parser/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseRequest {
    /// The tool that produced the output (e.g., "tsc", "jest", "rustc", "pytest").
    pub tool: String,
    /// The raw stdout from the tool execution.
    pub stdout: String,
    /// The raw stderr from the tool execution.
    pub stderr: String,
    /// The process exit code.
    pub exit_code: i32,
    /// Available source context for suggestion generation.
    pub source_context: SourceContext,
    /// Working directory where the tool was executed.
    pub working_directory: String,
}

impl From<ParseRequest> for ParseOutputInput {
    fn from(req: ParseRequest) -> Self {
        Self {
            tool: req.tool,
            stdout: req.stdout,
            stderr: req.stderr,
            exit_code: req.exit_code,
            source_context: req.source_context,
            working_directory: req.working_directory,
        }
    }
}

/// Response body for POST /api/v1/failure-parser/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResponse {
    pub success: bool,
    pub parsed: ParseOutputResult,
}

impl From<ParseOutputResult> for ParseResponse {
    fn from(result: ParseOutputResult) -> Self {
        Self {
            success: result.success,
            parsed: result,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure-parser/suggest-fix
// ---------------------------------------------------------------------------

/// POST /api/v1/failure-parser/suggest-fix
///
/// Generate a suggested fix for a specific failure.
///
/// **Request:** `SuggestFixRequest`
/// **Response:** `200 OK` with `SuggestFixResponse`
pub const SUGGEST_FIX_PATH: &str = "/api/v1/failure-parser/suggest-fix";
pub const SUGGEST_FIX_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure-parser/suggest-fix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestFixRequest {
    /// The failure to generate a fix for (as JSON with type tag).
    pub failure: TemplateFailure,
    /// Available source context.
    pub source_context: SourceContext,
    /// Minimum confidence threshold.
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,
}

fn default_min_confidence() -> f64 {
    0.5
}

impl From<SuggestFixRequest> for SuggestFixInput {
    fn from(req: SuggestFixRequest) -> Self {
        Self {
            failure: req.failure,
            source_context: req.source_context,
            min_confidence: req.min_confidence,
        }
    }
}

/// Response body for POST /api/v1/failure-parser/suggest-fix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestFixResponse {
    pub success: bool,
    pub suggestion: Option<String>,
    pub confidence: f64,
    pub rationale: Option<String>,
}

impl From<SuggestFixOutput> for SuggestFixResponse {
    fn from(output: SuggestFixOutput) -> Self {
        Self {
            success: output.suggestion.is_some(),
            suggestion: output.suggestion,
            confidence: output.confidence,
            rationale: output.rationale,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/failure-parser/parsers
// ---------------------------------------------------------------------------

/// GET /api/v1/failure-parser/parsers
///
/// List all registered parsers.
///
/// **Response:** `200 OK` with `ListParsersResponse`
pub const LIST_PARSERS_PATH: &str = "/api/v1/failure-parser/parsers";
pub const LIST_PARSERS_METHOD: &str = "GET";

/// Response body for GET /api/v1/failure-parser/parsers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListParsersResponse {
    pub parsers: Vec<ParserMetadata>,
    pub total: usize,
}

impl From<ListParsersResult> for ListParsersResponse {
    fn from(result: ListParsersResult) -> Self {
        Self {
            parsers: result.parsers,
            total: result.total,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure-parser/register-parser
// ---------------------------------------------------------------------------

/// POST /api/v1/failure-parser/register-parser
///
/// Register a new parser for a tool.
///
/// **Request:** `RegisterParserRequest`
/// **Response:** `200 OK` with `RegisterParserResponse`
pub const REGISTER_PARSER_PATH: &str = "/api/v1/failure-parser/register-parser";
pub const REGISTER_PARSER_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure-parser/register-parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterParserRequest {
    /// The tool name this parser handles.
    pub tool: String,
    /// Human-readable description.
    pub description: String,
}

impl From<RegisterParserRequest> for RegisterParserInput {
    fn from(req: RegisterParserRequest) -> Self {
        Self {
            tool: req.tool,
            description: req.description,
        }
    }
}

/// Response body for POST /api/v1/failure-parser/register-parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterParserResponse {
    pub success: bool,
    pub total_parsers: usize,
    pub message: String,
}

impl From<RegisterParserResult> for RegisterParserResponse {
    fn from(result: RegisterParserResult) -> Self {
        Self {
            success: result.success,
            total_parsers: result.total_parsers,
            message: result.message,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure-parser/format-for-llm
// ---------------------------------------------------------------------------

/// POST /api/v1/failure-parser/format-for-llm
///
/// Format failures into a human-readable summary for LLM consumption.
///
/// **Request:** `FormatForLlmRequest`
/// **Response:** `200 OK` with `FormatForLlmResponse`
pub const FORMAT_FOR_LLM_PATH: &str = "/api/v1/failure-parser/format-for-llm";
pub const FORMAT_FOR_LLM_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure-parser/format-for-llm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatForLlmRequest {
    /// The failures to format.
    pub failures: Vec<TemplateFailure>,
    /// Optional title.
    pub title: Option<String>,
}

impl From<FormatForLlmRequest> for FormatForLlmInput {
    fn from(req: FormatForLlmRequest) -> Self {
        Self {
            failures: req.failures,
            title: req.title,
        }
    }
}

/// Response body for POST /api/v1/failure-parser/format-for-llm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatForLlmResponse {
    pub formatted: String,
    pub count: usize,
}

impl From<FormatForLlmOutput> for FormatForLlmResponse {
    fn from(output: FormatForLlmOutput) -> Self {
        Self {
            formatted: output.formatted,
            count: output.count,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/failure-parser/classify-severity
// ---------------------------------------------------------------------------

/// POST /api/v1/failure-parser/classify-severity
///
/// Classify the overall severity of a set of failures.
///
/// **Request:** `ClassifySeverityRequest`
/// **Response:** `200 OK` with `ClassifySeverityResponse`
pub const CLASSIFY_SEVERITY_PATH: &str = "/api/v1/failure-parser/classify-severity";
pub const CLASSIFY_SEVERITY_METHOD: &str = "POST";

/// Request body for POST /api/v1/failure-parser/classify-severity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifySeverityRequest {
    /// The failure details to classify.
    pub failures: Vec<FailureDetail>,
}

impl From<ClassifySeverityRequest> for ClassifySeverityInput {
    fn from(req: ClassifySeverityRequest) -> Self {
        Self { failures: req.failures }
    }
}

/// Response body for POST /api/v1/failure-parser/classify-severity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifySeverityResponse {
    pub overall_severity: String,
    pub compile_blocks: usize,
    pub test_blocks: usize,
    pub warnings: usize,
}

impl From<ClassifySeverityOutput> for ClassifySeverityResponse {
    fn from(output: ClassifySeverityOutput) -> Self {
        Self {
            overall_severity: output.overall_severity,
            compile_blocks: output.compile_blocks,
            test_blocks: output.test_blocks,
            warnings: output.warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Failure Parser API endpoints.
///
/// All 4xx/5xx responses use this format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// HTTP status code.
    pub status: u16,
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Detailed error context (optional, may include field-level errors).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available).
    pub request_id: Option<String>,
}

/// Standardized error codes for Failure Parser API.
pub mod error_codes {
    /// Parsing failed — no matching patterns found.
    pub const PARSE_ERROR: &str = "PARSE_ERROR";
    /// The output format was not recognized.
    pub const UNRECOGNIZED_FORMAT: &str = "UNRECOGNIZED_FORMAT";
    /// No parser registered for the given tool.
    pub const UNSUPPORTED_TOOL: &str = "UNSUPPORTED_TOOL";
    /// Empty output provided.
    pub const EMPTY_OUTPUT: &str = "EMPTY_OUTPUT";
    /// Source context could not be built.
    pub const SOURCE_CONTEXT_ERROR: &str = "SOURCE_CONTEXT_ERROR";
    /// Suggestion generation failed.
    pub const SUGGESTION_FAILED: &str = "SUGGESTION_FAILED";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Failure Parser errors.
pub mod status_codes {
    pub const PARSE_ERROR: u16 = 422;
    pub const UNRECOGNIZED_FORMAT: u16 = 422;
    pub const UNSUPPORTED_TOOL: u16 = 400;
    pub const EMPTY_OUTPUT: u16 = 400;
    pub const SOURCE_CONTEXT_ERROR: u16 = 500;
    pub const SUGGESTION_FAILED: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::SourceLocation;

    #[test]
    fn test_parse_request_roundtrip() {
        let req = ParseRequest {
            tool: "tsc".into(),
            stdout: "error TS2339".into(),
            stderr: String::new(),
            exit_code: 2,
            source_context: SourceContext::empty(),
            working_directory: "/project".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ParseRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool, "tsc");
        assert_eq!(deserialized.exit_code, 2);
    }

    #[test]
    fn test_parse_request_to_input() {
        let req = ParseRequest {
            tool: "tsc".into(),
            stdout: "output".into(),
            stderr: String::new(),
            exit_code: 2,
            source_context: SourceContext::empty(),
            working_directory: "/project".into(),
        };
        let input: ParseOutputInput = req.into();
        assert_eq!(input.tool, "tsc");
        assert_eq!(input.exit_code, 2);
    }

    #[test]
    fn test_api_error_response() {
        let err = ApiErrorResponse {
            status: 422,
            code: "PARSE_ERROR".into(),
            message: "Could not parse output".into(),
            details: None,
            request_id: Some("req-123".into()),
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: ApiErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, 422);
        assert_eq!(deserialized.code, "PARSE_ERROR");
        assert_eq!(deserialized.request_id, Some("req-123".into()));
    }

    #[test]
    fn test_suggest_fix_request_default_confidence() {
        let req = SuggestFixRequest {
            failure: TemplateFailure::MissingSymbol {
                symbol: "x".into(),
                available: vec![],
                suggestion: None,
                location: SourceLocation::new("test.ts", 1, None),
            },
            source_context: SourceContext::empty(),
            min_confidence: 0.5,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: SuggestFixRequest = serde_json::from_str(&json).unwrap();
        assert!((deserialized.min_confidence - 0.5).abs() < 1e-10);
    }
}
