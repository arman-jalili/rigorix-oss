//! HTTP API contracts for Repo Engine endpoints.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #138
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
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::repo_engine::application::dto::{
    IndexDirectoryOutput, IndexFileOutput, LookupSymbolOutput, SearchSymbolsOutput,
    SymbolsByFileOutput,
};
use crate::repo_engine::domain::{SourceLanguage, SymbolDefinition, SymbolKind, SymbolVisibility};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All repo engine endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/repo-engine";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/repo-engine/symbols
// ---------------------------------------------------------------------------

/// GET /api/v1/repo-engine/symbols
///
/// Search for symbols in the graph.
///
/// **Query Parameters:**
/// - `q` — Search pattern (case-insensitive)
/// - `kind` — Optional symbol kind filter
/// - `language` — Optional language filter
/// - `limit` — Max results (default: 50)
///
/// **Response:** `200 OK` with `SearchSymbolsResponse`
pub const SEARCH_SYMBOLS_PATH: &str = "/api/v1/repo-engine/symbols";
pub const SEARCH_SYMBOLS_METHOD: &str = "GET";

/// Response for GET /api/v1/repo-engine/symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSymbolsResponse {
    pub symbols: Vec<SymbolSummary>,
    pub total_matches: usize,
    pub pattern: String,
    pub truncated: bool,
}

impl From<SearchSymbolsOutput> for SearchSymbolsResponse {
    fn from(output: SearchSymbolsOutput) -> Self {
        Self {
            symbols: output
                .symbols
                .into_iter()
                .map(SymbolSummary::from)
                .collect(),
            total_matches: output.total_matches,
            pattern: output.pattern,
            truncated: output.truncated,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/repo-engine/symbols/{name}
// ---------------------------------------------------------------------------

/// GET /api/v1/repo-engine/symbols/{name}
///
/// Look up a symbol by fully qualified name.
///
/// **Response:** `200 OK` with `LookupSymbolResponse`
/// **Error:** `404 Not Found` with `ApiErrorResponse`
pub const GET_SYMBOL_PATH: &str = "/api/v1/repo-engine/symbols/{name}";
pub const GET_SYMBOL_METHOD: &str = "GET";

/// Response for GET /api/v1/repo-engine/symbols/{name}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupSymbolResponse {
    pub symbol: Option<SymbolDetail>,
    pub references_from: Vec<String>,
    pub references_to: Vec<String>,
    pub found: bool,
}

impl From<LookupSymbolOutput> for LookupSymbolResponse {
    fn from(output: LookupSymbolOutput) -> Self {
        Self {
            symbol: output.symbol.map(SymbolDetail::from),
            references_from: output.references_from,
            references_to: output.references_to,
            found: output.found,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/repo-engine/symbols/by-file
// ---------------------------------------------------------------------------

/// GET /api/v1/repo-engine/symbols/by-file
///
/// Get all symbols defined in a file.
///
/// **Query Parameters:**
/// - `path` — File path (required)
/// - `kind` — Optional symbol kind filter
///
/// **Response:** `200 OK` with `SymbolsByFileResponse`
/// **Error:** `400 Bad Request` if `path` is missing
pub const SYMBOLS_BY_FILE_PATH: &str = "/api/v1/repo-engine/symbols/by-file";
pub const SYMBOLS_BY_FILE_METHOD: &str = "GET";

/// Response for GET /api/v1/repo-engine/symbols/by-file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolsByFileResponse {
    pub file: PathBuf,
    pub symbols: Vec<SymbolSummary>,
    pub total: usize,
}

impl From<SymbolsByFileOutput> for SymbolsByFileResponse {
    fn from(output: SymbolsByFileOutput) -> Self {
        Self {
            file: output.file,
            symbols: output
                .symbols
                .into_iter()
                .map(SymbolSummary::from)
                .collect(),
            total: output.total,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/repo-engine/index/file
// ---------------------------------------------------------------------------

/// POST /api/v1/repo-engine/index/file
///
/// Index a single file and extract its symbol definitions.
///
/// **Request:** `IndexFileRequest`
/// **Response:** `200 OK` with `IndexFileResponse`
/// **Error:** `400 Bad Request` if the file doesn't exist or is unsupported
pub const INDEX_FILE_PATH: &str = "/api/v1/repo-engine/index/file";
pub const INDEX_FILE_METHOD: &str = "POST";

/// Request body for POST /api/v1/repo-engine/index/file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexFileRequest {
    /// Path to the file to index.
    pub path: PathBuf,
    /// Optional language override (auto-detected if None).
    pub language: Option<SourceLanguage>,
    /// Maximum file size in bytes (0 = use config default).
    #[serde(default)]
    pub max_file_size: u64,
}

/// Response for POST /api/v1/repo-engine/index/file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexFileResponse {
    pub success: bool,
    pub path: PathBuf,
    pub language: SourceLanguage,
    pub symbols_found: usize,
    pub symbols_added: usize,
    pub symbols_rejected: usize,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl From<IndexFileOutput> for IndexFileResponse {
    fn from(output: IndexFileOutput) -> Self {
        Self {
            success: output.success,
            path: output.path,
            language: output.language,
            symbols_found: output.symbols.len(),
            symbols_added: output.symbols_added,
            symbols_rejected: output.symbols_rejected,
            duration_ms: output.duration_ms,
            error: if output.success {
                None
            } else {
                Some("Indexing failed".to_string())
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/repo-engine/index/directory
// ---------------------------------------------------------------------------

/// POST /api/v1/repo-engine/index/directory
///
/// Index an entire directory and add symbols to the graph.
///
/// **Request:** `IndexDirectoryRequest`
/// **Response:** `200 OK` with `IndexDirectoryResponse`
pub const INDEX_DIRECTORY_PATH: &str = "/api/v1/repo-engine/index/directory";
pub const INDEX_DIRECTORY_METHOD: &str = "POST";

/// Request body for POST /api/v1/repo-engine/index/directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDirectoryRequest {
    /// Root directory to index.
    pub root_dir: PathBuf,
    /// Whether to recursively scan subdirectories.
    #[serde(default = "default_true")]
    pub recursive: bool,
    /// Whether to detect project type first.
    #[serde(default = "default_true")]
    pub detect_project: bool,
    /// Maximum files to index (0 = no limit).
    #[serde(default)]
    pub max_files: usize,
}

fn default_true() -> bool {
    true
}

/// Response for POST /api/v1/repo-engine/index/directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDirectoryResponse {
    pub success: bool,
    pub root_dir: PathBuf,
    pub files_found: usize,
    pub files_indexed: usize,
    pub files_failed: usize,
    pub files_skipped: usize,
    pub symbols_added: usize,
    pub languages: Vec<String>,
    pub duration_ms: u64,
}

impl From<IndexDirectoryOutput> for IndexDirectoryResponse {
    fn from(output: IndexDirectoryOutput) -> Self {
        Self {
            success: output.success,
            root_dir: output.root_dir,
            files_found: output.total_files,
            files_indexed: output.files_indexed,
            files_failed: output.files_failed.len(),
            files_skipped: output.files_skipped.len(),
            symbols_added: output.symbols_added,
            languages: output.languages,
            duration_ms: output.duration_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/repo-engine/stats
// ---------------------------------------------------------------------------

/// GET /api/v1/repo-engine/stats
///
/// Get symbol graph statistics.
///
/// **Query Parameters:**
/// - `detailed` — Include per-kind breakdown (default: false)
///
/// **Response:** `200 OK` with `GraphStatsApiResponse`
pub const GRAPH_STATS_PATH: &str = "/api/v1/repo-engine/stats";
pub const GRAPH_STATS_METHOD: &str = "GET";

/// Response for GET /api/v1/repo-engine/stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatsApiResponse {
    pub total_symbols: usize,
    pub total_indexed: usize,
    pub by_kind: HashMap<String, usize>,
    pub by_language: HashMap<String, usize>,
    pub max_capacity: usize,
    pub reference_count: usize,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/repo-engine/clear
// ---------------------------------------------------------------------------

/// POST /api/v1/repo-engine/clear
///
/// Clear all symbols from the graph.
///
/// **Response:** `200 OK` with `ClearGraphResponse`
pub const CLEAR_GRAPH_PATH: &str = "/api/v1/repo-engine/clear";
pub const CLEAR_GRAPH_METHOD: &str = "POST";

/// Response for POST /api/v1/repo-engine/clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearGraphResponse {
    pub success: bool,
    pub previous_count: usize,
    pub full_reset: bool,
}

// ---------------------------------------------------------------------------
// Shared Response Types
// ---------------------------------------------------------------------------

/// Lightweight symbol summary for list responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSummary {
    pub id: Uuid,
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: u32,
    pub language: SourceLanguage,
    pub visibility: SymbolVisibility,
}

impl From<SymbolDefinition> for SymbolSummary {
    fn from(s: SymbolDefinition) -> Self {
        Self {
            id: s.id,
            name: s.name,
            kind: s.kind,
            file: s.location.file.clone(),
            line: s.location.line,
            language: s.language,
            visibility: s.visibility,
        }
    }
}

/// Full symbol detail for single-symbol responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDetail {
    pub id: Uuid,
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub signature: String,
    pub documentation: Option<String>,
    pub source_files: Vec<PathBuf>,
    pub definition_text: String,
    pub language: SourceLanguage,
    pub visibility: SymbolVisibility,
    pub tags: Vec<String>,
}

impl From<SymbolDefinition> for SymbolDetail {
    fn from(s: SymbolDefinition) -> Self {
        let source_files: Vec<PathBuf> = s.source_files.into_iter().collect();
        Self {
            id: s.id,
            name: s.name,
            kind: s.kind,
            file: s.location.file.clone(),
            line: s.location.line,
            column: s.location.column,
            signature: s.signature,
            documentation: s.documentation,
            source_files,
            definition_text: s.definition_text,
            language: s.language,
            visibility: s.visibility,
            tags: s.tags,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Repo Engine API endpoints.
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

/// Standardized error codes for Repo Engine API.
pub mod error_codes {
    pub const SYMBOL_NOT_FOUND: &str = "SYMBOL_NOT_FOUND";
    pub const DUPLICATE_SYMBOL: &str = "DUPLICATE_SYMBOL";
    pub const INDEXING_FAILED: &str = "INDEXING_FAILED";
    pub const UNSUPPORTED_EXTENSION: &str = "UNSUPPORTED_EXTENSION";
    pub const PARSE_ERROR: &str = "PARSE_ERROR";
    pub const PROJECT_DETECTION_FAILED: &str = "PROJECT_DETECTION_FAILED";
    pub const CAPACITY_EXCEEDED: &str = "CAPACITY_EXCEEDED";
    pub const VALIDATION_FAILED: &str = "VALIDATION_FAILED";
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Repo Engine errors.
pub mod status_codes {
    pub const SYMBOL_NOT_FOUND: u16 = 404;
    pub const DUPLICATE_SYMBOL: u16 = 409;
    pub const INDEXING_FAILED: u16 = 422;
    pub const UNSUPPORTED_EXTENSION: u16 = 400;
    pub const PARSE_ERROR: u16 = 422;
    pub const PROJECT_DETECTION_FAILED: u16 = 404;
    pub const CAPACITY_EXCEEDED: u16 = 507;
    pub const VALIDATION_FAILED: u16 = 422;
    pub const INTERNAL_ERROR: u16 = 500;
}
