//! Event payload schemas for the Repo Engine bounded context.
//!
//! @canonical .pi/architecture/decisions/ADR-005-event-bus-persistence.md
//! Implements: Contract Freeze — RepoEngineEvent payload schemas
//! Issue: #138
//!
//! These events are emitted on the `EventBus` whenever symbol graph operations
//! and indexing actions occur. Consumers (audit, console printer, TUI) subscribe
//! to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Events emitted by the Repo Engine module.
///
/// Wrapped in `ExecutionEvent::RepoEngine(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RepoEngineEvent {
    /// A symbol was successfully indexed and added to the graph.
    SymbolIndexed {
        /// The symbol name that was indexed.
        name: String,
        /// The kind of the indexed symbol.
        kind: String,
        /// The source file where the symbol was found.
        source_file: PathBuf,
        /// The language of the indexed file.
        language: String,
        /// Total symbols in the graph after this addition.
        total_symbols: usize,
    },

    /// A symbol was looked up by name.
    SymbolLookedUp {
        /// The symbol name that was queried.
        name: String,
        /// Whether the lookup was successful.
        found: bool,
    },

    /// A file was indexed and its symbols extracted.
    FileIndexed {
        /// The file path that was indexed.
        path: PathBuf,
        /// The language detected for the file.
        language: String,
        /// Number of symbols extracted from this file.
        symbol_count: usize,
        /// Duration in milliseconds for the indexing operation.
        duration_ms: u64,
    },

    /// A file indexing encountered an error.
    FileIndexFailed {
        /// The file path that failed.
        path: PathBuf,
        /// Error detail for diagnostics.
        error: String,
    },

    /// An indexing session began for a directory.
    IndexingStarted {
        /// The root directory being indexed.
        root_dir: PathBuf,
        /// Estimated number of files to index.
        estimated_file_count: usize,
        /// Languages detected in the project.
        languages: Vec<String>,
    },

    /// An indexing session completed.
    IndexingCompleted {
        /// The root directory that was indexed.
        root_dir: PathBuf,
        /// Total files indexed.
        files_indexed: usize,
        /// Files that failed to index.
        files_failed: usize,
        /// Total symbols added to the graph.
        symbols_added: usize,
        /// Total duration in milliseconds.
        duration_ms: u64,
    },

    /// The symbol graph was cleared or reset.
    SymbolGraphCleared {
        /// Previous symbol count before clear.
        previous_count: usize,
        /// Whether this was a full reset or selective clear.
        full_reset: bool,
    },

    /// A symbol was removed from the graph.
    SymbolRemoved {
        /// The removed symbol name.
        name: String,
        /// The kind of the removed symbol.
        kind: String,
    },

    /// A reference relationship between symbols was recorded.
    ReferenceRecorded {
        /// The source symbol name.
        from: String,
        /// The target symbol name.
        to: String,
    },

    /// Project type was detected for a repository.
    ProjectTypeDetected {
        /// The repository root directory.
        root_dir: PathBuf,
        /// Detected project types (languages).
        project_types: Vec<String>,
        /// Manifest file that triggered the detection.
        manifest_file: Option<PathBuf>,
    },

    /// An unsupported file was skipped during indexing.
    FileSkipped {
        /// The skipped file path.
        path: PathBuf,
        /// Reason for skipping.
        reason: String,
    },
}
