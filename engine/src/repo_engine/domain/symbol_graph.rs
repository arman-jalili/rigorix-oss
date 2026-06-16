//! SymbolGraph, SymbolDefinition, SymbolKind, Location, and SharedSymbolGraph.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#graph
//! Implements: Contract Freeze — SymbolGraph aggregate, SymbolDefinition value object,
//!   SymbolKind enum, Location value object, SharedSymbolGraph wrapper
//! Issue: #138
//!
//! Defines the core domain types for the in-memory symbol graph that maintains
//! O(1) definition lookups and reference traversal across Rust, Python, and
//! TypeScript source files.
//!
//! # Contract (Frozen)
//! - SymbolGraph is the root aggregate holding all symbol definitions
//! - All fields are public for direct access by application services
//! - Construction happens via SymbolGraphService (from indexed sources)
//! - Thread-safe access is achieved through SharedSymbolGraph (Arc<RwLock<>>)

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// SymbolKind
// ---------------------------------------------------------------------------

/// The kind of a code symbol.
///
/// Represents the type of definition in the source code. Used for filtering
/// and categorizing symbols in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    /// A function or method definition.
    Function,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition (Rust).
    Trait,
    /// A constant or immutable global.
    Constant,
    /// A type alias or type definition.
    Type,
    /// A module or namespace.
    Module,
    /// An impl block (Rust).
    Impl,
    /// A class definition (Python, TypeScript).
    Class,
    /// An interface definition (TypeScript).
    Interface,
    /// A decorator (Python, TypeScript).
    Decorator,
    /// A macro invocation or definition.
    Macro,
}

// ---------------------------------------------------------------------------
// Location
// ---------------------------------------------------------------------------

/// Source code location of a symbol definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    /// Absolute or relative path to the source file.
    pub file: PathBuf,
    /// 0-indexed line number where the definition starts.
    pub line: u32,
    /// 0-indexed column number where the definition starts.
    pub column: u32,
}

impl Location {
    /// Create a new location.
    pub fn new(file: PathBuf, line: u32, column: u32) -> Self {
        Self { file, line, column }
    }
}

// ---------------------------------------------------------------------------
// SymbolDefinition
// ---------------------------------------------------------------------------

/// A single code symbol definition in the graph.
///
/// Represents any named code entity (function, struct, enum, trait, etc.)
/// that has been indexed from a source file. Each symbol has a unique ID,
/// name, kind, source location, and optional documentation.
///
/// # Contract (Frozen)
/// - `id` is a UUIDv4 assigned at creation time
/// - `name` is the fully qualified name (module::path::Name)
/// - `kind` categorizes the symbol type
/// - `location` points to the primary definition site
/// - `source_files` includes the primary file and any additional files
///   that contribute to this symbol (e.g., impl blocks in separate files)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolDefinition {
    /// Globally unique identifier for this symbol.
    pub id: Uuid,

    /// Fully qualified symbol name (e.g. "my_module::MyStruct").
    pub name: String,

    /// The kind of code symbol (Function, Struct, Enum, etc.).
    pub kind: SymbolKind,

    /// Primary source location of the definition.
    pub location: Location,

    /// Full signature text (e.g. "fn foo<T>(x: T) -> Result<T, Error>").
    pub signature: String,

    /// Optional documentation comment (docstring/doc comment text).
    pub documentation: Option<String>,

    /// All source files that contribute to this symbol's definition.
    pub source_files: HashSet<PathBuf>,

    /// Full source text of the definition body.
    pub definition_text: String,

    /// Language the symbol was indexed from.
    pub language: SourceLanguage,

    /// Visibility/access level of the symbol.
    pub visibility: SymbolVisibility,

    /// Tags for additional metadata.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl SymbolDefinition {
    /// Create a new symbol definition with minimal required fields.
    ///
    /// Generates a UUIDv4 for `id`, initializes `source_files` with the
    /// location's file, and sets `visibility` to `Public` by default.
    pub fn new(
        name: String,
        kind: SymbolKind,
        location: Location,
        signature: String,
        definition_text: String,
        language: SourceLanguage,
    ) -> Self {
        let id = Uuid::new_v4();
        let source_files = {
            let mut set = HashSet::new();
            set.insert(location.file.clone());
            set
        };

        Self {
            id,
            name,
            kind,
            location,
            signature,
            documentation: None,
            source_files,
            definition_text,
            language,
            visibility: SymbolVisibility::Public,
            tags: Vec::new(),
        }
    }

    /// Check if this symbol is at the given file location.
    pub fn is_at(&self, file: &std::path::Path, line: u32) -> bool {
        self.location.file == file && self.location.line == line
    }

    /// Check if this symbol spans the given line (definition start ≤ line ≤ definition end).
    /// Note: end line is approximated from `definition_text` line count.
    pub fn spans_line(&self, file: &std::path::Path, line: u32) -> bool {
        if self.location.file != file || line < self.location.line {
            return false;
        }
        let line_count = self.definition_text.lines().count() as u32;
        line <= self.location.line + line_count.saturating_sub(1)
    }

    /// Add a source file that contributes to this symbol's definition.
    pub fn add_source_file(&mut self, path: PathBuf) {
        self.source_files.insert(path);
    }
}

// ---------------------------------------------------------------------------
// SourceLanguage
// ---------------------------------------------------------------------------

/// The programming language a symbol was indexed from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceLanguage {
    /// Rust source file (.rs).
    Rust,
    /// Python source file (.py).
    Python,
    /// TypeScript source file (.ts, .tsx).
    TypeScript,
}

// ---------------------------------------------------------------------------
// SymbolVisibility
// ---------------------------------------------------------------------------

/// Visibility/access level of a code symbol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum SymbolVisibility {
    /// Public — accessible from outside the module.
    #[default]
    Public,
    /// Private — only accessible within the module.
    Private,
    /// Protected — accessible within the module and sub-modules.
    Protected,
    /// Crate-only access (pub(crate) in Rust).
    Crate,
}


// ---------------------------------------------------------------------------
// SymbolGraph
// ---------------------------------------------------------------------------

/// In-memory graph of code symbols with O(1) lookups.
///
/// Maintains a map of symbol names to their definitions, plus adjacency
/// for reference traversal. Thread-safe access is achieved via the
/// `SharedSymbolGraph` wrapper (Arc<RwLock<Self>>).
///
/// # Contract (Frozen)
/// - O(1) lookup by fully qualified symbol name
/// - O(n) lookup by file path (returns all symbols in a file)
/// - Pattern-based search iterates over all symbols
/// - No thread safety built in — use SharedSymbolGraph for concurrent access
/// - Adjacency tracking is optional and implementation-dependent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolGraph {
    /// Map of fully qualified symbol name → SymbolDefinition.
    definitions: HashMap<String, SymbolDefinition>,

    /// Adjacency map: symbol name → set of referenced symbol names (outgoing edges).
    adjacency: HashMap<String, HashSet<String>>,

    /// Total number of symbols added (including removed, for metrics).
    total_indexed: usize,

    /// Optional maximum symbol count (0 = unlimited).
    pub max_capacity: usize,
}

impl SymbolGraph {
    /// Create a new empty symbol graph.
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            adjacency: HashMap::new(),
            total_indexed: 0,
            max_capacity: 0,
        }
    }

    /// Create a new empty symbol graph with a maximum capacity.
    pub fn with_capacity(max: usize) -> Self {
        Self {
            definitions: HashMap::with_capacity(max),
            adjacency: HashMap::new(),
            total_indexed: 0,
            max_capacity: max,
        }
    }

    /// Add a symbol definition to the graph.
    ///
    /// Returns `RepoEngineError::DuplicateSymbol` if a symbol with the same
    /// name already exists. Returns `RepoEngineError::CapacityExceeded` if
    /// the graph has reached its maximum capacity.
    pub fn add_symbol(
        &mut self,
        def: SymbolDefinition,
    ) -> Result<(), crate::repo_engine::domain::RepoEngineError> {
        // Check capacity
        if self.max_capacity > 0 && self.definitions.len() >= self.max_capacity {
            return Err(
                crate::repo_engine::domain::RepoEngineError::CapacityExceeded {
                    capacity: self.max_capacity,
                },
            );
        }

        let name = def.name.clone();

        // Check for duplicates
        if self.definitions.contains_key(&name) {
            return Err(crate::repo_engine::domain::RepoEngineError::DuplicateSymbol { name });
        }

        self.definitions.insert(name, def);
        self.total_indexed += 1;
        Ok(())
    }

    /// Look up a symbol by its fully qualified name.
    ///
    /// Returns `None` if no symbol with that name exists in the graph.
    pub fn lookup(&self, name: &str) -> Option<&SymbolDefinition> {
        self.definitions.get(name)
    }

    /// Look up all symbols defined in a given file.
    ///
    /// Returns a vector of symbol definitions sorted by line number.
    pub fn lookup_by_file(&self, file: &std::path::Path) -> Vec<&SymbolDefinition> {
        let mut result: Vec<&SymbolDefinition> = self
            .definitions
            .values()
            .filter(|def| def.location.file == file || def.source_files.contains(file))
            .collect();

        result.sort_by_key(|def| def.location.line);
        result
    }

    /// Search for symbols whose name or signature matches a pattern.
    ///
    /// Pattern matching is case-insensitive substring matching on the
    /// fully qualified symbol name and its signature.
    /// More sophisticated search (regex, fuzzy) is handled at the application layer.
    pub fn search(&self, pattern: &str) -> Vec<&SymbolDefinition> {
        let lower = pattern.to_lowercase();
        let mut result: Vec<&SymbolDefinition> = self
            .definitions
            .values()
            .filter(|def| {
                def.name.to_lowercase().contains(&lower)
                    || def.signature.to_lowercase().contains(&lower)
                    || def
                        .documentation
                        .as_ref()
                        .is_some_and(|doc| doc.to_lowercase().contains(&lower))
            })
            .collect();

        result.sort_by_key(|def| def.name.clone());
        result
    }

    /// Search for symbols matching a kind filter.
    pub fn filter_by_kind(&self, kind: SymbolKind) -> Vec<&SymbolDefinition> {
        let mut result: Vec<&SymbolDefinition> = self
            .definitions
            .values()
            .filter(|def| def.kind == kind)
            .collect();

        result.sort_by_key(|def| def.name.clone());
        result
    }

    /// Search for symbols matching a language filter.
    pub fn filter_by_language(&self, language: SourceLanguage) -> Vec<&SymbolDefinition> {
        let mut result: Vec<&SymbolDefinition> = self
            .definitions
            .values()
            .filter(|def| def.language == language)
            .collect();

        result.sort_by_key(|def| def.name.clone());
        result
    }

    /// Remove a symbol from the graph by name.
    ///
    /// Also removes any adjacency entries referencing this symbol.
    /// Returns `true` if the symbol existed and was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let existed = self.definitions.remove(name).is_some();
        self.adjacency.remove(name);
        // Remove references to this symbol from other entries
        for edges in self.adjacency.values_mut() {
            edges.remove(name);
        }
        existed
    }

    /// Get the number of symbols currently in the graph.
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Get the total number of symbols indexed (including removed, for metrics).
    pub fn total_indexed(&self) -> usize {
        self.total_indexed
    }

    /// Get a reference to the internal definitions map.
    ///
    /// For iteration and inspection by application services.
    pub fn all_definitions(&self) -> &HashMap<String, SymbolDefinition> {
        &self.definitions
    }

    /// Add an adjacency edge from `from` to `to`.
    ///
    /// Records that the symbol `from` references the symbol `to`.
    /// Both symbols must exist in the graph.
    pub fn add_reference(&mut self, from: &str, to: &str) -> bool {
        if !self.definitions.contains_key(from) || !self.definitions.contains_key(to) {
            return false;
        }
        self.adjacency
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
        true
    }

    /// Get all symbols referenced by the given symbol (outgoing edges).
    pub fn references_from(&self, name: &str) -> Option<&HashSet<String>> {
        self.adjacency.get(name)
    }

    /// Get all symbols that reference the given symbol (incoming edges).
    ///
    /// This is O(n) — computed by scanning all adjacency entries.
    pub fn references_to(&self, name: &str) -> Vec<String> {
        self.adjacency
            .iter()
            .filter(|(_, edges)| edges.contains(name))
            .map(|(from, _)| from.clone())
            .collect()
    }

    /// Wrap this `SymbolGraph` in an `Arc<RwLock<>>` for thread-safe sharing.
    pub fn into_shared(self) -> SharedSymbolGraph {
        SharedSymbolGraph(Arc::new(RwLock::new(self)))
    }

    /// Get all symbol names in the graph.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.definitions.keys()
    }

    /// Check if a symbol with the given name exists in the graph.
    pub fn contains_key(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }
}

impl Default for SymbolGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SharedSymbolGraph
// ---------------------------------------------------------------------------

/// Thread-safe wrapper around `SymbolGraph` using `Arc<RwLock<>>`.
///
/// Provides interior mutability for concurrent read/write access across
/// multiple tasks. The `RwLock` allows multiple concurrent readers or a
/// single writer.
///
/// # Contract (Frozen)
/// - `read()` acquires a read lock (blocking)
/// - `write()` acquires a write lock (blocking)
/// - `try_read()` / `try_write()` for non-blocking access
/// - Clone is cheap (Arc increment)
#[derive(Debug, Clone)]
pub struct SharedSymbolGraph(Arc<RwLock<SymbolGraph>>);

impl SharedSymbolGraph {
    /// Create a new `SharedSymbolGraph` wrapping an empty `SymbolGraph`.
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(SymbolGraph::new())))
    }

    /// Create a new `SharedSymbolGraph` wrapping a `SymbolGraph` with the given capacity.
    pub fn with_capacity(max: usize) -> Self {
        Self(Arc::new(RwLock::new(SymbolGraph::with_capacity(max))))
    }

    /// Acquire a read lock.
    ///
    /// Multiple concurrent reads are allowed.
    /// Panics if the lock is poisoned (another thread panicked while holding the lock).
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, SymbolGraph> {
        self.0.read().expect("SymbolGraph RwLock poisoned")
    }

    /// Acquire a write lock.
    ///
    /// Exclusive access — no other reads or writes allowed.
    /// Panics if the lock is poisoned.
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, SymbolGraph> {
        self.0.write().expect("SymbolGraph RwLock poisoned")
    }

    /// Attempt to acquire a read lock without blocking.
    pub fn try_read(&self) -> Option<std::sync::RwLockReadGuard<'_, SymbolGraph>> {
        self.0.try_read().ok()
    }

    /// Attempt to acquire a write lock without blocking.
    pub fn try_write(&self) -> Option<std::sync::RwLockWriteGuard<'_, SymbolGraph>> {
        self.0.try_write().ok()
    }

    /// Unwrap the inner `Arc<RwLock<SymbolGraph>>`.
    ///
    /// Used for testing or advanced use cases.
    pub fn into_inner(self) -> Arc<RwLock<SymbolGraph>> {
        self.0
    }
}

impl Default for SharedSymbolGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl From<SymbolGraph> for SharedSymbolGraph {
    fn from(graph: SymbolGraph) -> Self {
        Self(Arc::new(RwLock::new(graph)))
    }
}

// ---------------------------------------------------------------------------
// SymbolDefinition Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod symbol_definition_tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_location() -> Location {
        Location::new(PathBuf::from("src/lib.rs"), 42, 4)
    }

    fn sample_definition(name: &str) -> SymbolDefinition {
        SymbolDefinition::new(
            name.to_string(),
            SymbolKind::Function,
            sample_location(),
            format!("fn {}() -> Result<()>", name),
            format!("fn {}() -> Result<()> {{ Ok(()) }}", name),
            SourceLanguage::Rust,
        )
    }

    #[test]
    fn test_symbol_definition_new() {
        let def = sample_definition("my_function");

        assert_eq!(def.name, "my_function");
        assert_eq!(def.kind, SymbolKind::Function);
        assert_eq!(def.location.file, PathBuf::from("src/lib.rs"));
        assert_eq!(def.location.line, 42);
        assert_eq!(def.location.column, 4);
        assert_eq!(def.signature, "fn my_function() -> Result<()>");
        assert!(def.documentation.is_none());
        assert_eq!(def.source_files.len(), 1);
        assert!(def.source_files.contains(&PathBuf::from("src/lib.rs")));
        assert_eq!(def.language, SourceLanguage::Rust);
        assert_eq!(def.visibility, SymbolVisibility::Public);
        assert!(def.tags.is_empty());
    }

    #[test]
    fn test_symbol_definition_id_is_unique() {
        let def1 = sample_definition("func_a");
        let def2 = sample_definition("func_b");
        assert_ne!(def1.id, def2.id);
    }

    #[test]
    fn test_symbol_definition_with_documentation() {
        let mut def = sample_definition("documented_fn");
        def.documentation = Some("This is a documented function.".to_string());

        assert_eq!(
            def.documentation,
            Some("This is a documented function.".to_string())
        );
    }

    #[test]
    fn test_symbol_definition_is_at() {
        let def = sample_definition("my_fn");

        assert!(def.is_at(&PathBuf::from("src/lib.rs"), 42));
        assert!(!def.is_at(&PathBuf::from("src/lib.rs"), 43));
        assert!(!def.is_at(&PathBuf::from("src/other.rs"), 42));
    }

    #[test]
    fn test_symbol_definition_spans_line() {
        let def = SymbolDefinition::new(
            "multi_line".to_string(),
            SymbolKind::Function,
            sample_location(),
            "fn multi_line()".to_string(),
            "fn multi_line() {\n    let x = 1;\n    let y = 2;\n}".to_string(),
            SourceLanguage::Rust,
        );

        // Starts at line 42
        assert!(def.spans_line(&PathBuf::from("src/lib.rs"), 42));
        assert!(def.spans_line(&PathBuf::from("src/lib.rs"), 43));
        assert!(def.spans_line(&PathBuf::from("src/lib.rs"), 45));
        // Before start
        assert!(!def.spans_line(&PathBuf::from("src/lib.rs"), 41));
        // Different file
        assert!(!def.spans_line(&PathBuf::from("src/other.rs"), 42));
    }

    #[test]
    fn test_symbol_definition_add_source_file() {
        let mut def = sample_definition("multi_file");

        assert_eq!(def.source_files.len(), 1);

        def.add_source_file(PathBuf::from("src/impl.rs"));
        assert_eq!(def.source_files.len(), 2);
        assert!(def.source_files.contains(&PathBuf::from("src/impl.rs")));

        // Adding the same file again doesn't duplicate
        def.add_source_file(PathBuf::from("src/impl.rs"));
        assert_eq!(def.source_files.len(), 2);
    }

    #[test]
    fn test_symbol_definition_visibility() {
        let def = sample_definition("pub_fn");
        assert_eq!(def.visibility, SymbolVisibility::Public);

        let mut def = SymbolDefinition::new(
            "priv_fn".to_string(),
            SymbolKind::Function,
            sample_location(),
            "fn priv_fn()".to_string(),
            "fn priv_fn() {}".to_string(),
            SourceLanguage::Rust,
        );
        def.visibility = SymbolVisibility::Private;
        assert_eq!(def.visibility, SymbolVisibility::Private);
    }

    #[test]
    fn test_symbol_definition_language() {
        let rust_def = sample_definition("rust_fn");
        assert_eq!(rust_def.language, SourceLanguage::Rust);

        let py_def = SymbolDefinition::new(
            "py_fn".to_string(),
            SymbolKind::Function,
            Location::new(PathBuf::from("src/main.py"), 1, 0),
            "def py_fn():".to_string(),
            "def py_fn():\n    pass".to_string(),
            SourceLanguage::Python,
        );
        assert_eq!(py_def.language, SourceLanguage::Python);

        let ts_def = SymbolDefinition::new(
            "ts_fn".to_string(),
            SymbolKind::Function,
            Location::new(PathBuf::from("src/main.ts"), 1, 0),
            "function tsFn()".to_string(),
            "function tsFn() {}".to_string(),
            SourceLanguage::TypeScript,
        );
        assert_eq!(ts_def.language, SourceLanguage::TypeScript);
    }

    #[test]
    fn test_symbol_definition_kinds() {
        let cases = vec![
            (SymbolKind::Function, "my_func"),
            (SymbolKind::Struct, "MyStruct"),
            (SymbolKind::Enum, "MyEnum"),
            (SymbolKind::Trait, "MyTrait"),
            (SymbolKind::Constant, "MY_CONST"),
            (SymbolKind::Type, "MyType"),
            (SymbolKind::Module, "my_module"),
            (SymbolKind::Impl, "MyImpl"),
            (SymbolKind::Class, "MyClass"),
            (SymbolKind::Interface, "MyInterface"),
            (SymbolKind::Decorator, "my_decorator"),
            (SymbolKind::Macro, "my_macro!"),
        ];

        for (kind, name) in cases {
            let def = SymbolDefinition::new(
                name.to_string(),
                kind.clone(),
                sample_location(),
                format!("{:?} {}", kind, name),
                format!("{} {{}}", name),
                SourceLanguage::Rust,
            );
            assert_eq!(def.kind, kind, "Failed for kind {:?}", kind);
            drop(def);
        }
    }

    #[test]
    fn test_symbol_definition_tags() {
        let mut def = sample_definition("tagged_fn");
        def.tags = vec!["deprecated".to_string(), "unsafe".to_string()];

        assert_eq!(def.tags.len(), 2);
        assert!(def.tags.contains(&"deprecated".to_string()));
        assert!(def.tags.contains(&"unsafe".to_string()));
    }

    #[test]
    fn test_symbol_definition_default_visibility() {
        assert_eq!(SymbolVisibility::default(), SymbolVisibility::Public);
    }

    #[test]
    fn test_symbol_definition_location_new() {
        let loc = Location::new(PathBuf::from("src/main.rs"), 100, 8);
        assert_eq!(loc.file, PathBuf::from("src/main.rs"));
        assert_eq!(loc.line, 100);
        assert_eq!(loc.column, 8);
    }

    #[test]
    fn test_symbol_definition_equality() {
        let def1 = sample_definition("same");
        let mut def2 = sample_definition("same");
        // Different IDs from same input
        def2.id = def1.id; // Force same ID

        assert_eq!(def1, def2);
    }
}
