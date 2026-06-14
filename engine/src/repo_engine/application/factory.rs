//! Factory interfaces for constructing Repo Engine domain objects.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — SymbolFactory, GraphFactory, IndexerFactory traits
//! Issue: #138
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;
use std::path::PathBuf;

use crate::repo_engine::domain::{
    Location, RepoEngineError, SourceLanguage, SymbolDefinition, SymbolGraph, SymbolKind,
    SymbolVisibility,
};

use super::dto::{AddSymbolInput, RepoEngineConfig};

// ---------------------------------------------------------------------------
// SymbolFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `SymbolDefinition` instances.
///
/// Implementations handle creating SymbolDefinition objects from raw input,
/// applying defaults for unset fields, and validating the result.
#[async_trait]
pub trait SymbolFactory: Send + Sync {
    /// Construct a `SymbolDefinition` from an `AddSymbolInput`.
    ///
    /// Generates a UUID, sets defaults for optional fields,
    /// and validates required fields.
    async fn build_symbol(
        &self,
        input: AddSymbolInput,
    ) -> Result<SymbolDefinition, RepoEngineError>;

    /// Construct a `Location` from a file path and position.
    fn build_location(&self, file: PathBuf, line: u32, column: u32) -> Location;

    /// Create a minimal symbol for testing purposes.
    ///
    /// This is a convenience method for test fixtures.
    /// Production code should use `build_symbol`.
    fn build_test_symbol(
        &self,
        name: &str,
        kind: SymbolKind,
        file: PathBuf,
        line: u32,
    ) -> SymbolDefinition;
}

// ---------------------------------------------------------------------------
// GraphFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `SymbolGraph` instances.
///
/// Implementations handle creating configured SymbolGraph instances with
/// capacity limits, pre-populated built-in symbols, and default settings.
#[async_trait]
pub trait GraphFactory: Send + Sync {
    /// Create an empty symbol graph with default settings.
    async fn empty_graph(&self) -> SymbolGraph;

    /// Create a symbol graph with the given capacity.
    async fn with_capacity(&self, max: usize) -> SymbolGraph;

    /// Create a symbol graph from configuration.
    async fn from_config(&self, config: &RepoEngineConfig) -> SymbolGraph;

    /// Batch-add multiple symbol definitions to a graph.
    ///
    /// Processes symbols in order, collecting successes and failures.
    /// Unlike individual `add_symbol`, this does not fail on the first error.
    async fn batch_add(
        &self,
        graph: &mut SymbolGraph,
        symbols: Vec<SymbolDefinition>,
    ) -> BatchAddResult;
}

/// Result from batch-adding symbols to a graph.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchAddResult {
    /// Number of symbols successfully added.
    pub added: usize,
    /// Number of symbols rejected (duplicates, capacity).
    pub rejected: usize,
    /// Details of rejected symbols.
    pub rejections: Vec<SymbolRejection>,
}

/// Details about a rejected symbol addition.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolRejection {
    /// The symbol name that was rejected.
    pub name: String,
    /// The reason for rejection.
    pub reason: String,
}

// ---------------------------------------------------------------------------
// IndexerFactory
// ---------------------------------------------------------------------------

/// Factory for constructing language-specific indexers.
///
/// Implementations handle creating RustIndexer, PythonIndexer, and
/// TypeScriptIndexer instances, registering them with the indexer service,
/// and managing their lifecycle.
///
/// # Contract (Frozen)
/// - Each supported language gets a dedicated indexer instance
/// - Indexers are stateless — they parse without maintaining state
/// - Tree-sitter grammars are loaded at indexer construction time
#[async_trait]
pub trait IndexerFactory: Send + Sync {
    /// Create a Rust source file indexer.
    async fn create_rust_indexer(&self) -> Result<Box<dyn LanguageIndexer>, RepoEngineError>;

    /// Create a Python source file indexer.
    async fn create_python_indexer(&self) -> Result<Box<dyn LanguageIndexer>, RepoEngineError>;

    /// Create a TypeScript source file indexer.
    async fn create_typescript_indexer(&self) -> Result<Box<dyn LanguageIndexer>, RepoEngineError>;

    /// Create all registered indexers.
    ///
    /// Returns a map of language → indexer for all configured languages.
    async fn create_all_indexers(
        &self,
    ) -> Result<Vec<(SourceLanguage, Box<dyn LanguageIndexer>)>, RepoEngineError>;

    /// Get the indexer for a specific language.
    ///
    /// Returns `None` if the language is not supported.
    async fn get_indexer(
        &self,
        language: &SourceLanguage,
    ) -> Option<Box<dyn LanguageIndexer>>;
}

// ---------------------------------------------------------------------------
// LanguageIndexer
// ---------------------------------------------------------------------------

/// Interface for a language-specific source code indexer.
///
/// Each language indexer uses the appropriate tree-sitter grammar to parse
/// source files and extract `SymbolDefinition` instances.
///
/// # Contract (Frozen)
/// - Indexers are stateless — they parse one file at a time
/// - The returned symbols are candidates — the caller decides which to add
/// - Parse errors are returned per-file, not fatal to the indexing session
#[async_trait]
pub trait LanguageIndexer: Send + Sync {
    /// Index a single source file and extract symbol definitions.
    ///
    /// Returns all symbols found in the file, without validation against
    /// an existing graph. Validation happens at the service layer.
    async fn index_source(
        &self,
        path: &PathBuf,
        source: &str,
    ) -> Result<Vec<SymbolDefinition>, RepoEngineError>;

    /// Get the language this indexer handles.
    fn language(&self) -> SourceLanguage;

    /// Get the file extensions this indexer supports.
    fn supported_extensions(&self) -> Vec<String>;

    /// Check if this indexer can handle the given file extension.
    fn can_handle(&self, extension: &str) -> bool {
        self.supported_extensions()
            .iter()
            .any(|ext| ext == extension.trim_start_matches('.'))
    }
}
