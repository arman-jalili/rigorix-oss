//! Data Transfer Objects for the Repo Engine module.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — DTO schemas for symbol graph, indexing, lookups
//! Issue: #138
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
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::repo_engine::domain::{
    Location, SourceLanguage, SymbolDefinition, SymbolKind, SymbolVisibility, SymbolWorkspaceIntent,
};

// ---------------------------------------------------------------------------
// Symbol Registration DTOs
// ---------------------------------------------------------------------------

/// Input for adding a symbol to the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddSymbolInput {
    /// Fully qualified symbol name.
    pub name: String,

    /// The kind of symbol.
    pub kind: SymbolKind,

    /// Source file location.
    pub location: Location,

    /// Full signature text.
    pub signature: String,

    /// Full source text of the definition.
    pub definition_text: String,

    /// Language of the source file.
    pub language: SourceLanguage,

    /// Optional documentation.
    pub documentation: Option<String>,

    /// Symbol visibility.
    #[serde(default)]
    pub visibility: SymbolVisibility,

    /// Optional metadata tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Output from adding a symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddSymbolOutput {
    /// The assigned symbol UUID.
    pub symbol_id: Uuid,

    /// The symbol name.
    pub name: String,

    /// Total symbols in the graph after addition.
    pub total_symbols: usize,

    /// Whether the symbol was accepted (false if duplicate rejected).
    pub accepted: bool,
}

// ---------------------------------------------------------------------------
// Symbol Lookup DTOs
// ---------------------------------------------------------------------------

/// Input for looking up a symbol by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupSymbolInput {
    /// Fully qualified symbol name to look up.
    pub name: String,

    /// Whether to include adjacency info (references from/to).
    #[serde(default)]
    pub include_adjacency: bool,

    /// Maximum depth for reference traversal (0 = no traversal).
    #[serde(default)]
    pub reference_depth: u32,
}

/// Output from looking up a symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LookupSymbolOutput {
    /// The symbol definition (if found).
    pub symbol: Option<SymbolDefinition>,

    /// Symbols referenced by this symbol (outgoing edges).
    pub references_from: Vec<String>,

    /// Symbols that reference this symbol (incoming edges).
    pub references_to: Vec<String>,

    /// Whether the symbol was found.
    pub found: bool,
}

// ---------------------------------------------------------------------------
// Symbol Search DTOs
// ---------------------------------------------------------------------------

/// Input for searching symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSymbolsInput {
    /// Search pattern (case-insensitive substring match on name/signature/doc).
    pub pattern: String,

    /// Optional filter by symbol kind.
    pub kind_filter: Option<SymbolKind>,

    /// Optional filter by source language.
    pub language_filter: Option<SourceLanguage>,

    /// Optional maximum results to return.
    pub max_results: Option<usize>,
}

/// Output from searching symbols.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchSymbolsOutput {
    /// Matching symbol definitions.
    pub symbols: Vec<SymbolDefinition>,

    /// Total number of matches found (before max_results limit).
    pub total_matches: usize,

    /// The search pattern used.
    pub pattern: String,

    /// Whether results were truncated by max_results.
    pub truncated: bool,
}

// ---------------------------------------------------------------------------
// File Indexing DTOs
// ---------------------------------------------------------------------------

/// Input for indexing a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexFileInput {
    /// Path to the file to index.
    pub path: PathBuf,

    /// The language to use for parsing (auto-detected if None).
    pub language: Option<SourceLanguage>,

    /// Whether to skip files exceeding this size (in bytes, 0 = no limit).
    pub max_file_size: u64,
}

/// Output from indexing a single file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexFileOutput {
    /// The file path that was indexed.
    pub path: PathBuf,

    /// Language detected/used for indexing.
    pub language: SourceLanguage,

    /// Symbols extracted from this file.
    pub symbols: Vec<SymbolDefinition>,

    /// Number of symbols added to the graph.
    pub symbols_added: usize,

    /// Number of symbols rejected (duplicates, capacity).
    pub symbols_rejected: usize,

    /// Duration of the indexing operation in milliseconds.
    pub duration_ms: u64,

    /// Whether indexing was successful.
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Directory Indexing DTOs
// ---------------------------------------------------------------------------

/// Input for indexing a directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDirectoryInput {
    /// Root directory path to index.
    pub root_dir: PathBuf,

    /// File extensions to include (empty = use defaults per language).
    pub extensions: Option<Vec<String>>,

    /// Glob patterns to exclude.
    pub exclude_patterns: Option<Vec<String>>,

    /// Maximum file size in bytes to index (0 = unlimited).
    #[serde(default)]
    pub max_file_size: u64,

    /// Maximum number of files to index (0 = unlimited).
    #[serde(default)]
    pub max_files: usize,

    /// Whether to recursively index subdirectories.
    #[serde(default = "default_true")]
    pub recursive: bool,

    /// Whether to detect project type from manifest files first.
    #[serde(default = "default_true")]
    pub detect_project: bool,
}

fn default_true() -> bool {
    true
}

/// Output from indexing a directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexDirectoryOutput {
    /// The root directory that was indexed.
    pub root_dir: PathBuf,

    /// Total files found.
    pub total_files: usize,

    /// Files successfully indexed.
    pub files_indexed: usize,

    /// Files that failed to index.
    pub files_failed: Vec<IndexFailure>,

    /// Files skipped (unsupported extensions, too large, etc.).
    pub files_skipped: Vec<SkippedFile>,

    /// Total symbols added to the graph.
    pub symbols_added: usize,

    /// Symbols indexed per language.
    pub symbols_by_language: HashMap<String, usize>,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// Detected languages in the project.
    pub languages: Vec<String>,

    /// Whether the complete indexing was successful.
    pub success: bool,
}

/// A file that failed to index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexFailure {
    /// The file path that failed.
    pub path: PathBuf,

    /// Error message.
    pub error: String,

    /// Line number of the error, if available.
    pub line: Option<u32>,
}

/// A file that was skipped during indexing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkippedFile {
    /// The file path that was skipped.
    pub path: PathBuf,

    /// Reason for skipping.
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Project Detection DTOs
// ---------------------------------------------------------------------------

/// Input for detecting project type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectProjectInput {
    /// Root directory to detect project type from.
    pub root_dir: PathBuf,

    /// Whether to scan subdirectories for nested projects.
    #[serde(default)]
    pub scan_subdirs: bool,
}

/// Output from detecting project type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectProjectOutput {
    /// The detected project type (language).
    pub project_type: String,

    /// The detected source languages.
    pub languages: Vec<SourceLanguage>,

    /// File extensions to index for this project.
    pub extensions: Vec<String>,

    /// The manifest file path that triggered detection.
    pub manifest_file: Option<PathBuf>,

    /// Whether detection was successful.
    pub detected: bool,
}

// ---------------------------------------------------------------------------
// Symbol Graph Query DTOs
// ---------------------------------------------------------------------------

/// Input for querying symbols by file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolsByFileInput {
    /// File path to query.
    pub file: PathBuf,

    /// Optional filter by symbol kind.
    pub kind_filter: Option<SymbolKind>,
}

/// Output from querying symbols by file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolsByFileOutput {
    /// File path queried.
    pub file: PathBuf,

    /// Symbols defined in this file.
    pub symbols: Vec<SymbolDefinition>,

    /// Total number of symbols in this file.
    pub total: usize,
}

/// Input for getting graph statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatsInput {
    /// Whether to include detailed per-kind breakdown.
    #[serde(default)]
    pub detailed: bool,
}

/// Statistics about the symbol graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphStatsOutput {
    /// Total symbols in the graph.
    pub total_symbols: usize,

    /// Total symbols indexed over the graph's lifetime.
    pub total_indexed: usize,

    /// Per-kind symbol counts (if detailed requested).
    pub by_kind: HashMap<String, usize>,

    /// Per-language symbol counts.
    pub by_language: HashMap<String, usize>,

    /// Maximum capacity (0 = unlimited).
    pub max_capacity: usize,

    /// Number of reference edges recorded.
    pub reference_count: usize,
}

// ---------------------------------------------------------------------------
// Symbol Validation DTOs
// ---------------------------------------------------------------------------

/// Input for validating symbols in a workspace context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateWorkspaceInput {
    /// File paths with changes to validate.
    pub changed_files: Vec<PathBuf>,

    /// The workspace intent of the task.
    pub intent: SymbolWorkspaceIntent,

    /// Whether to check for orphaned symbol references.
    #[serde(default)]
    pub check_references: bool,

    /// Whether to detect naming conflicts.
    #[serde(default)]
    pub check_conflicts: bool,
}

/// Output from workspace validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateWorkspaceOutput {
    /// Whether the workspace state is valid for the given intent.
    pub valid: bool,

    /// Validation messages.
    pub messages: Vec<ValidationMessage>,

    /// Number of errors found.
    pub error_count: usize,

    /// Number of warnings found.
    pub warning_count: usize,
}

/// A validation message with severity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationMessage {
    /// Severity level.
    pub severity: ValidationSeverity,

    /// Human-readable message.
    pub message: String,

    /// Source file or symbol related to this message.
    pub source: Option<String>,

    /// Error code for machine handling.
    pub code: Option<String>,
}

/// Severity of a validation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// Error — must be resolved for the operation to proceed.
    Error,
    /// Warning — should be addressed but doesn't block the operation.
    Warning,
    /// Info — informational message.
    Info,
}

// ---------------------------------------------------------------------------
// Repo Engine Configuration DTOs
// ---------------------------------------------------------------------------

/// Configuration for the Repo Engine module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoEngineConfig {
    /// File extensions to include for each language.
    pub language_extensions: HashMap<SourceLanguage, Vec<String>>,

    /// Glob patterns to exclude from indexing.
    pub exclude_patterns: Vec<String>,

    /// Maximum file size in bytes to index.
    pub max_file_size: u64,

    /// Maximum number of symbols in the graph (0 = unlimited).
    pub max_symbol_capacity: usize,

    /// Whether to build adjacency (reference) graph.
    #[serde(default = "default_true")]
    pub build_adjacency: bool,

    /// Maximum files to index per directory scan (0 = unlimited).
    pub max_files_per_scan: usize,

    /// Whether to index on orchestrator startup.
    #[serde(default = "default_true")]
    pub index_on_startup: bool,

    /// Comma-separated directories to exclude from indexing (e.g. node_modules, target).
    pub exclude_dirs: Vec<String>,
}

impl Default for RepoEngineConfig {
    fn default() -> Self {
        let mut language_extensions = HashMap::new();
        language_extensions.insert(SourceLanguage::Rust, vec!["rs".to_string()]);
        language_extensions.insert(SourceLanguage::Python, vec!["py".to_string()]);
        language_extensions.insert(
            SourceLanguage::TypeScript,
            vec!["ts".to_string(), "tsx".to_string()],
        );

        Self {
            language_extensions,
            exclude_patterns: vec!["*.min.*".to_string(), "*.generated.*".to_string()],
            max_file_size: 1_048_576, // 1 MB
            max_symbol_capacity: 0,   // unlimited
            build_adjacency: true,
            max_files_per_scan: 100_000,
            index_on_startup: true,
            exclude_dirs: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                ".next".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".venv".to_string(),
                "__pycache__".to_string(),
            ],
        }
    }
}
