//! Repo Engine error types.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#errors
//! Implements: Contract Freeze — RepoEngineError enum
//! Issue: #138
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `RepoEngineError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during symbol graph and indexing operations.
#[derive(Debug, Error)]
pub enum RepoEngineError {
    /// A symbol with the same name already exists in the graph.
    #[error("Symbol already exists: {name}")]
    DuplicateSymbol {
        /// The symbol name that already exists.
        name: String,
    },

    /// The requested symbol was not found in the graph.
    #[error("Symbol not found: {name}")]
    SymbolNotFound {
        /// The symbol name that was requested.
        name: String,
        /// Nearby symbol names for user guidance.
        suggestions: Vec<String>,
    },

    /// An IO error occurred during file indexing.
    #[error("IO error indexing file: {io_error}")]
    Io {
        /// The underlying IO error.
        #[from]
        io_error: std::io::Error,
    },

    /// Unsupported file extension encountered during indexing.
    #[error("Unsupported file extension: {extension} (path: {path})")]
    UnsupportedExtension {
        /// The file extension that is not supported.
        extension: String,
        /// The full file path.
        path: String,
    },

    /// A parsing error occurred during indexing.
    #[error("Parse error in {path}: {detail}")]
    ParseError {
        /// The file path that failed to parse.
        path: String,
        /// Human-readable parse error description.
        detail: String,
        /// Source line number of the error, if available.
        line: Option<u32>,
    },

    /// Language detection failed for a file.
    #[error("Could not detect language for file: {path}")]
    LanguageDetectionFailed {
        /// The file path that could not be classified.
        path: String,
    },

    /// Project type detection failed (no manifest file found).
    #[error("No project manifest found in directory: {directory}")]
    ProjectTypeDetectionFailed {
        /// The directory that was searched.
        directory: String,
        /// Manifest files that were expected (e.g. Cargo.toml, pyproject.toml, tsconfig.json).
        expected_manifests: Vec<String>,
    },

    /// A cycle was detected in symbol references.
    #[error("Cycle detected in symbol references")]
    CycleDetected {
        /// Symbols involved in the cycle.
        symbols: Vec<String>,
    },

    /// The symbol graph has reached its maximum capacity.
    #[error("Symbol graph capacity exceeded: {capacity}")]
    CapacityExceeded {
        /// Maximum capacity that was reached.
        capacity: usize,
    },

    /// An invalid symbol name was provided.
    #[error("Invalid symbol name: {name} — {reason}")]
    InvalidSymbolName {
        /// The invalid symbol name.
        name: String,
        /// Why the name is invalid.
        reason: String,
    },

    /// Indexing was cancelled or interrupted.
    #[error("Indexing cancelled: {detail}")]
    IndexingCancelled {
        /// Human-readable description of the cancellation reason.
        detail: String,
    },

    /// Internal error (should not happen under normal operation).
    #[error("Internal error: {detail}")]
    Internal {
        /// Internal error detail.
        detail: String,
    },
}
impl RepoEngineError {
    pub fn is_retriable(&self) -> bool {
        matches!(self, RepoEngineError::Io { .. })
    }
}
