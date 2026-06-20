//! HTTP API contracts for Code Generation Pipeline endpoints.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #424
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

use crate::code_gen::application::dto::SyntaxGateConfig;
use crate::code_gen::application::dto::{
    EditFileInput, EditFileOutput, ReadFileInput, ReadFileOutput,
};
use crate::code_gen::domain::result::SyntaxGateResult;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All code generation endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/code-gen";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-gen/edit
// ---------------------------------------------------------------------------

/// POST /api/v1/code-gen/edit
///
/// Execute an edit_file operation: replace exact text in a file.
///
/// **Request:** `EditFileRequest`
/// **Response:** `200 OK` with `EditFileResponse`
pub const EDIT_FILE_PATH: &str = "/api/v1/code-gen/edit";
pub const EDIT_FILE_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-gen/edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileRequest {
    /// File path relative to workspace root.
    pub path: String,
    /// Exact text to find and replace.
    pub old_string: String,
    /// Replacement text.
    pub new_string: String,
    /// Replace all occurrences (default: first match only).
    #[serde(default)]
    pub replace_all: Option<bool>,
}

impl From<EditFileRequest> for EditFileInput {
    fn from(req: EditFileRequest) -> Self {
        Self {
            path: req.path,
            old_string: req.old_string,
            new_string: req.new_string,
            replace_all: req.replace_all,
        }
    }
}

/// Response body for POST /api/v1/code-gen/edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileResponse {
    pub success: bool,
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    pub original_file: String,
    pub updated_content: String,
    pub unified_diff: String,
    pub replace_all: bool,
    pub occurrences_replaced: usize,
    pub syntax_gate_result: Option<SyntaxGateResult>,
}

impl From<EditFileOutput> for EditFileResponse {
    fn from(output: EditFileOutput) -> Self {
        Self {
            success: true,
            file_path: output.file_path,
            old_string: output.old_string,
            new_string: output.new_string,
            original_file: output.original_file,
            updated_content: output.updated_content,
            unified_diff: output.unified_diff,
            replace_all: output.replace_all,
            occurrences_replaced: output.occurrences_replaced,
            syntax_gate_result: output.syntax_gate_result,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-gen/edit/preview
// ---------------------------------------------------------------------------

/// POST /api/v1/code-gen/edit/preview
///
/// Preview an edit without applying it. Returns the diff and updated
/// content but does not write to disk.
///
/// **Request:** `EditFileRequest`
/// **Response:** `200 OK` with `EditFilePreviewResponse`
pub const EDIT_FILE_PREVIEW_PATH: &str = "/api/v1/code-gen/edit/preview";
pub const EDIT_FILE_PREVIEW_METHOD: &str = "POST";

/// Response body for POST /api/v1/code-gen/edit/preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFilePreviewResponse {
    pub success: bool,
    pub file_path: String,
    pub unified_diff: String,
    pub updated_content: String,
    pub occurrences_would_replace: usize,
    pub syntax_errors: Vec<SyntaxErrorResponse>,
}

/// A syntax error in API response format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxErrorResponse {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub context: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-gen/read
// ---------------------------------------------------------------------------

/// POST /api/v1/code-gen/read
///
/// Read a file with optional offset/limit and binary detection.
///
/// **Request:** `ReadFileRequest`
/// **Response:** `200 OK` with `ReadFileResponse`
pub const READ_FILE_PATH: &str = "/api/v1/code-gen/read";
pub const READ_FILE_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-gen/read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileRequest {
    /// File path relative to workspace root.
    pub path: String,
    /// Starting line offset (1-indexed).
    pub offset: Option<usize>,
    /// Maximum number of lines to return.
    pub limit: Option<usize>,
}

impl From<ReadFileRequest> for ReadFileInput {
    fn from(req: ReadFileRequest) -> Self {
        Self {
            path: req.path,
            offset: req.offset,
            limit: req.limit,
            max_file_size: None,
        }
    }
}

/// Response body for POST /api/v1/code-gen/read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileResponse {
    pub success: bool,
    pub file_path: String,
    pub content: String,
    pub start_line: usize,
    pub total_lines: usize,
    pub total_bytes: u64,
    pub is_binary: bool,
}

impl From<ReadFileOutput> for ReadFileResponse {
    fn from(output: ReadFileOutput) -> Self {
        Self {
            success: true,
            file_path: output.file_path,
            content: output.content,
            start_line: output.start_line,
            total_lines: output.total_lines,
            total_bytes: output.total_bytes,
            is_binary: output.is_binary,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/code-gen/verify
// ---------------------------------------------------------------------------

/// POST /api/v1/code-gen/verify
///
/// Run syntax verification on a file.
///
/// **Request:** `VerifySyntaxRequest`
/// **Response:** `200 OK` with `VerifySyntaxResponse`
pub const VERIFY_SYNTAX_PATH: &str = "/api/v1/code-gen/verify";
pub const VERIFY_SYNTAX_METHOD: &str = "POST";

/// Request body for POST /api/v1/code-gen/verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySyntaxRequest {
    /// File path (used for language detection).
    pub file_path: String,
    /// File content to verify.
    pub content: String,
}

/// Response body for POST /api/v1/code-gen/verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySyntaxResponse {
    pub success: bool,
    pub passed: bool,
    pub errors: Vec<SyntaxErrorResponse>,
    pub detected_language: Option<String>,
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/code-gen/config
// ---------------------------------------------------------------------------

/// GET /api/v1/code-gen/config
///
/// Get the current code generation configuration.
///
/// **Response:** `200 OK` with `CodeGenConfigResponse`
pub const CODE_GEN_CONFIG_PATH: &str = "/api/v1/code-gen/config";
pub const CODE_GEN_CONFIG_METHOD: &str = "GET";

/// Response body for GET /api/v1/code-gen/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenConfigResponse {
    pub success: bool,
    pub edit_file: EditFileConfigResponse,
    pub syntax_gate: SyntaxGateConfig,
}

/// EditFile configuration in API response format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileConfigResponse {
    pub max_file_size: u64,
    pub enable_identity_check: bool,
    pub require_syntax_gate: bool,
    pub max_replacements: usize,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Code Gen API endpoints.
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
    /// Detailed error context (optional).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available).
    pub request_id: Option<String>,
}

/// Standardized error codes for Code Gen API.
pub mod error_codes {
    pub const OLD_STRING_NOT_FOUND: &str = "CODE_GEN_OLD_STRING_NOT_FOUND";
    pub const IDENTITY_EDIT: &str = "CODE_GEN_IDENTITY_EDIT";
    pub const BINARY_FILE: &str = "CODE_GEN_BINARY_FILE";
    pub const FILE_TOO_LARGE: &str = "CODE_GEN_FILE_TOO_LARGE";
    pub const WORKSPACE_ESCAPE: &str = "CODE_GEN_WORKSPACE_ESCAPE";
    pub const SYNTAX_ERROR: &str = "CODE_GEN_SYNTAX_ERROR";
    pub const PATH_VALIDATION_FAILED: &str = "CODE_GEN_PATH_VALIDATION_FAILED";
    pub const INTERNAL_ERROR: &str = "CODE_GEN_INTERNAL_ERROR";
}

/// HTTP status code mappings for Code Gen errors.
pub mod status_codes {
    pub const OLD_STRING_NOT_FOUND: u16 = 404;
    pub const IDENTITY_EDIT: u16 = 400;
    pub const BINARY_FILE: u16 = 400;
    pub const FILE_TOO_LARGE: u16 = 413;
    pub const WORKSPACE_ESCAPE: u16 = 403;
    pub const SYNTAX_ERROR: u16 = 422;
    pub const PATH_VALIDATION_FAILED: u16 = 403;
    pub const INTERNAL_ERROR: u16 = 500;
}
