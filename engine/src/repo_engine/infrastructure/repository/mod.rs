//! Repository interfaces for the Repo Engine bounded context.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — SymbolRepository, SourceRepository, GrammarRepository traits
//! Issue: #138
//!
//! Repositories abstract symbol storage, source file access, and grammar
//! registration behind interfaces, allowing implementations to use filesystem,
//! embedded, or mock storage without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;
use std::path::PathBuf;

use crate::repo_engine::domain::{RepoEngineError, SourceLanguage, SymbolDefinition, SymbolGraph};

// ---------------------------------------------------------------------------
// SymbolRepository
// ---------------------------------------------------------------------------

/// Repository for persisting and loading symbol definitions.
///
/// Abstracts the storage of symbol definitions — whether in-memory, filesystem-based
/// (JSON cache), or database-backed.
///
/// # Contract (Frozen)
/// - Save operations persist individual or batch symbol definitions
/// - Load operations restore the full symbol graph from storage
/// - The repository is responsible for serialization format
/// - No graph-level operations (lookup, search) — those belong in SymbolGraphService
#[async_trait]
pub trait SymbolRepository: Send + Sync {
    /// Save a single symbol definition.
    ///
    /// Returns `RepoEngineError::DuplicateSymbol` if a symbol with the same
    /// name already exists in the storage backend.
    async fn save_symbol(&self, symbol: &SymbolDefinition) -> Result<(), RepoEngineError>;

    /// Save multiple symbol definitions in a batch.
    ///
    /// More efficient than individual saves for bulk operations.
    /// Rejected symbols are returned; the batch does not fail on individual rejects.
    async fn save_symbols_batch(
        &self,
        symbols: &[SymbolDefinition],
    ) -> Result<Vec<SymbolDefinition>, RepoEngineError>;

    /// Load all symbols from storage into a SymbolGraph.
    ///
    /// Restores the full graph state, including symbol definitions and
    /// optionally adjacency data. Returns an empty graph if no data exists.
    async fn load_all(&self) -> Result<SymbolGraph, RepoEngineError>;

    /// Check if a symbol with the given name exists in storage.
    async fn contains(&self, name: &str) -> Result<bool, RepoEngineError>;

    /// Get the total count of stored symbols.
    async fn count(&self) -> Result<usize, RepoEngineError>;

    /// Delete a symbol by name from storage.
    ///
    /// Returns `true` if the symbol existed and was deleted.
    async fn delete(&self, name: &str) -> Result<bool, RepoEngineError>;

    /// Clear all symbols from storage.
    async fn clear(&self) -> Result<(), RepoEngineError>;
}

// ---------------------------------------------------------------------------
// SourceRepository
// ---------------------------------------------------------------------------

/// Repository for reading source file content for indexing.
///
/// Abstracts the source of code files — whether from the local filesystem,
/// a Git repository, a remote API, or an in-memory test fixture.
///
/// # Contract (Frozen)
/// - Read operations return raw source content for parsing by language indexers
/// - Directory listing returns file paths matching supported extensions
/// - Implementations MUST validate file paths against directory traversal attacks
/// - Files exceeding configurable size limits are reported (not read)
#[async_trait]
pub trait SourceRepository: Send + Sync {
    /// Read a source file's content as a string.
    ///
    /// Returns `RepoEngineError::Io` for filesystem errors.
    /// Files exceeding the configured size limit return `RepoEngineError::Internal`.
    async fn read_source(&self, path: &PathBuf) -> Result<String, RepoEngineError>;

    /// List all source files in a directory matching the given extensions.
    ///
    /// Returns file paths. Non-recursive by default.
    async fn list_source_files(
        &self,
        dir: &PathBuf,
        extensions: &[String],
        recursive: bool,
    ) -> Result<Vec<PathBuf>, RepoEngineError>;

    /// Check if a source file exists and is accessible.
    async fn source_exists(&self, path: &PathBuf) -> bool;

    /// Get the size of a source file in bytes.
    async fn file_size(&self, path: &PathBuf) -> Result<u64, RepoEngineError>;

    /// Get the file extension from a path.
    fn extension(&self, path: &PathBuf) -> Option<String>;

    /// Detect the language from a file extension.
    fn detect_language(&self, extension: &str) -> Option<SourceLanguage> {
        match extension.trim_start_matches('.') {
            "rs" => Some(SourceLanguage::Rust),
            "py" => Some(SourceLanguage::Python),
            "ts" | "tsx" => Some(SourceLanguage::TypeScript),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// GrammarRepository
// ---------------------------------------------------------------------------

/// Repository for loading tree-sitter grammar definitions.
///
/// Abstracts the loading of language grammars — whether compiled into the binary,
/// loaded from shared libraries, or fetched on demand.
///
/// # Contract (Frozen)
/// - Grammars are loaded lazily on first use
/// - Each language has a single, immutable grammar
/// - Grammar loading failures are cached (don't retry on every index call)
/// - The repository manages grammar lifecycle (load, unload, language queries)
#[async_trait]
pub trait GrammarRepository: Send + Sync {
    /// Load and return a tree-sitter language for the given source language.
    ///
    /// Returns `RepoEngineError::Internal` if the grammar is not available
    /// or fails to load.
    async fn load_grammar(
        &self,
        language: &SourceLanguage,
    ) -> Result<tree_sitter::Language, RepoEngineError>;

    /// Check if a grammar is available for the given language.
    async fn has_grammar(&self, language: &SourceLanguage) -> bool;

    /// Get all languages with available grammars.
    async fn available_languages(&self) -> Vec<SourceLanguage>;

    /// Register a grammar for a language (for testing or dynamic loading).
    async fn register_grammar(
        &self,
        language: SourceLanguage,
        grammar: tree_sitter::Language,
    );

    /// Unload a grammar, freeing its resources.
    async fn unload_grammar(&self, language: &SourceLanguage) -> Result<(), RepoEngineError>;
}
