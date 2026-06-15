//! Service interfaces (use cases) for the Repo Engine bounded context.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — SymbolGraphService, IndexerService, ProjectDetectionService traits
//! Issue: #138
//!
//! These traits define the application-level operations for symbol graph management,
//! source code indexing, and project type detection. All methods are async and return
//! domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::repo_engine::domain::{RepoEngineError, SymbolGraph};

use super::dto::{
    AddSymbolInput, AddSymbolOutput, DetectProjectInput, DetectProjectOutput, GraphStatsInput,
    GraphStatsOutput, IndexDirectoryInput, IndexDirectoryOutput, IndexFileInput, IndexFileOutput,
    LookupSymbolInput, LookupSymbolOutput, SearchSymbolsInput, SearchSymbolsOutput,
    SymbolsByFileInput, SymbolsByFileOutput, ValidateWorkspaceInput, ValidateWorkspaceOutput,
};

// ---------------------------------------------------------------------------
// SymbolGraphService
// ---------------------------------------------------------------------------

/// Application service for managing the symbol graph.
///
/// Handles adding, looking up, searching, and removing symbols from the
/// in-memory symbol graph. Provides O(1) lookups by name and O(n) lookups
/// by file path.
///
/// # Contract (Frozen)
/// - All graph mutations go through this service
/// - Duplicate detection is performed on add
/// - Capacity limits are enforced
/// - References/adjacency relationships are built by this service
#[async_trait]
pub trait SymbolGraphService: Send + Sync {
    /// Add a symbol definition to the graph.
    ///
    /// Returns `RepoEngineError::DuplicateSymbol` if a symbol with the same
    /// name already exists. Returns `RepoEngineError::CapacityExceeded` if
    /// the graph has reached its maximum capacity.
    async fn add_symbol(&self, input: AddSymbolInput) -> Result<AddSymbolOutput, RepoEngineError>;

    /// Look up a symbol by its fully qualified name.
    ///
    /// Returns the symbol definition with optional adjacency information.
    async fn lookup_symbol(
        &self,
        input: LookupSymbolInput,
    ) -> Result<LookupSymbolOutput, RepoEngineError>;

    /// Search for symbols matching a pattern.
    ///
    /// Performs case-insensitive substring matching on symbol name, signature,
    /// and documentation. Supports optional kind/language filters.
    async fn search_symbols(
        &self,
        input: SearchSymbolsInput,
    ) -> Result<SearchSymbolsOutput, RepoEngineError>;

    /// Get all symbols defined in a file.
    async fn symbols_by_file(
        &self,
        input: SymbolsByFileInput,
    ) -> Result<SymbolsByFileOutput, RepoEngineError>;

    /// Remove a symbol from the graph by name.
    ///
    /// Also removes any adjacency entries referencing this symbol.
    /// Returns `RepoEngineError::SymbolNotFound` if the symbol doesn't exist.
    async fn remove_symbol(&self, name: &str) -> Result<bool, RepoEngineError>;

    /// Clear all symbols from the graph.
    ///
    /// Resets the graph to an empty state. This is a destructive operation.
    async fn clear_graph(&self) -> Result<(), RepoEngineError>;

    /// Get graph statistics (total symbols, per-kind breakdown, etc.).
    async fn graph_stats(
        &self,
        input: GraphStatsInput,
    ) -> Result<GraphStatsOutput, RepoEngineError>;

    /// Record a reference relationship between two symbols.
    ///
    /// Returns `RepoEngineError::SymbolNotFound` if either symbol doesn't exist.
    async fn add_reference(
        &self,
        from: &str,
        to: &str,
    ) -> Result<bool, RepoEngineError>;

    /// Get access to the underlying `SymbolGraph` for direct inspection.
    ///
    /// Returns the graph behind its thread-safe wrapper, allowing callers
    /// to perform batch operations without repeated async calls.
    ///
    /// # Contract
    /// - The returned reference is valid only within the scope of this method
    /// - Callers must not retain references across await points
    fn graph(&self) -> &SymbolGraph;
}

// ---------------------------------------------------------------------------
// IndexerService
// ---------------------------------------------------------------------------

/// Application service for indexing source files and extracting symbols.
///
/// Handles language-specific parsing using tree-sitter grammars (Rust, Python,
/// TypeScript), extracting `SymbolDefinition` instances from parsed source files,
/// and adding them to the symbol graph.
///
/// # Contract (Frozen)
/// - Language detection is automatic (based on file extension or manifest analysis)
/// - Unsupported file extensions are skipped (not errored)
/// - Files exceeding `max_file_size` are skipped
/// - Indexing is bounded — large repos may skip files beyond the configured limit
/// - Binary files are never parsed (filtered by extension)
#[async_trait]
pub trait IndexerService: Send + Sync {
    /// Index a single file and extract its symbol definitions.
    ///
    /// Returns the parsed symbols regardless of whether they were added to the
    /// graph. The caller (orchestrator) decides which symbols to add.
    async fn index_file(&self, input: IndexFileInput) -> Result<IndexFileOutput, RepoEngineError>;

    /// Index an entire directory recursively.
    ///
    /// Scans the directory for supported files, indexes each one, and adds
    /// symbols to the graph. Returns aggregated results including failures.
    async fn index_directory(
        &self,
        input: IndexDirectoryInput,
    ) -> Result<IndexDirectoryOutput, RepoEngineError>;

    /// Detect project type from manifest files in a directory.
    ///
    /// Checks for Cargo.toml (Rust), pyproject.toml (Python), tsconfig.json (TypeScript),
    /// and returns the detected languages and file extensions to index.
    async fn detect_project_type(
        &self,
        input: DetectProjectInput,
    ) -> Result<DetectProjectOutput, RepoEngineError>;

    /// Check if a file extension is supported.
    fn is_extension_supported(&self, extension: &str) -> bool;

    /// Get the supported file extensions.
    fn supported_extensions(&self) -> Vec<String>;

    /// Get the count of files indexed in the current session.
    async fn indexed_file_count(&self) -> usize;
}

// ---------------------------------------------------------------------------
// WorkspaceValidationService
// ---------------------------------------------------------------------------

/// Application service for validating workspace operations against the symbol graph.
///
/// Used in Phase 3 (pre-execution validation) to ensure that task operations
/// are consistent with the current state of the symbol graph. This prevents
/// operations that would leave the graph in an inconsistent state.
///
/// # Contract (Frozen)
/// - Validates that existing symbols are present before Modification/Deletion
/// - Detects naming conflicts for new symbol additions
/// - Checks reference integrity when symbols are modified or deleted
/// - All validation is read-only — no graph mutations
#[async_trait]
pub trait WorkspaceValidationService: Send + Sync {
    /// Validate workspace state for a given intent and set of changed files.
    ///
    /// Checks:
    /// - If `intent` is `Modification` or `Deletion`, all affected symbols exist
    /// - If `intent` is `ReadWrite`, no naming conflicts with existing symbols
    /// - If `check_references`, no orphaned references are created
    /// - If `check_conflicts`, no conflicting symbols across files
    async fn validate_workspace(
        &self,
        input: ValidateWorkspaceInput,
    ) -> Result<ValidateWorkspaceOutput, RepoEngineError>;
}
