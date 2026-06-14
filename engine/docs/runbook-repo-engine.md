# Runbook: repo-engine Module

<!--
Canonical Reference: .pi/architecture/modules/repo-engine.md
Last Updated: 2026-06-14
-->

## Overview

The `repo-engine` module manages multi-language code indexing and symbol graph
operations. It indexes Rust, Python, and TypeScript source files using tree-sitter
parsers, maintains an in-memory symbol graph with O(1) definition lookups, and
provides workspace validation for pre-execution planning phases.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `SymbolGraph` | Domain entity | In-memory graph with O(1) lookups by fully qualified name |
| `SymbolDefinition` | Domain value object | Symbol with id, name, kind, location, signature, docs |
| `SymbolKind` | Domain enum | 12 variants (Function, Struct, Enum, Trait, Constant, Type, Module, Impl, Class, Interface, Decorator, Macro) |
| `Location` | Domain value object | Source file location (file, line, column) |
| `SourceLanguage` | Domain enum | Rust, Python, TypeScript |
| `SymbolVisibility` | Domain enum | Public, Private, Protected, Crate |
| `SharedSymbolGraph` | Domain wrapper | Arc<RwLock<SymbolGraph>> for thread-safe concurrent access |
| `SymbolWorkspaceIntent` | Domain enum | ReadOnly, ReadWrite, Modification, Deletion — describes task interaction with graph |
| `SymbolGraphServiceImpl` | Application service | RwLock-backed implementation of SymbolGraphService |
| `WorkspaceValidationServiceImpl` | Application service | Phase 3 pre-execution validation |
| `SymbolRepository` | Infrastructure trait | Persistence for symbol definitions |
| `SourceRepository` | Infrastructure trait | Source file reading and language detection |
| `GrammarRepository` | Infrastructure trait | Tree-sitter grammar loading |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async I/O for indexing operations |
| serde + serde_json | Yes | Serialization for symbol definitions and DTOs |
| async-trait | Yes | Async trait support for service interfaces |
| uuid | Yes | Unique symbol identifiers |
| thiserror | Yes | Structured error types |
| tree-sitter | No (optional) | Grammar parsing for source code indexing |

### Initialization

1. Create a `SymbolGraphServiceImpl` or `SharedSymbolGraph` for the graph
2. Optionally configure capacity limits via `SymbolGraphServiceImpl::with_capacity()`
3. Create a `WorkspaceValidationServiceImpl` backed by the graph for validation
4. On startup, index the repository source files:
   - Detect project type from manifest files (Cargo.toml, pyproject.toml, tsconfig.json)
   - Scan directories for supported file extensions
   - Parse each file using tree-sitter language grammars
   - Add extracted symbols to the graph
5. Symbol graph is now ready for O(1) lookups and workspace validation

```rust
use rigorix::repo_engine::application::*;
use rigorix::repo_engine::domain::*;

// Create the symbol graph service
let graph_service = SymbolGraphServiceImpl::new();
let validation_service = WorkspaceValidationServiceImpl::new();

// Add indexed symbols
let output = graph_service
    .add_symbol(AddSymbolInput {
        name: "my_module::MyStruct".to_string(),
        kind: SymbolKind::Struct,
        location: Location::new(PathBuf::from("src/lib.rs"), 10, 0),
        signature: "pub struct MyStruct<T>".to_string(),
        definition_text: "pub struct MyStruct<T> {\n    field: T,\n}".to_string(),
        language: SourceLanguage::Rust,
        documentation: None,
        visibility: SymbolVisibility::Public,
        tags: vec![],
    })
    .await
    .unwrap();

// Query the graph
let lookup = graph_service
    .lookup_symbol(LookupSymbolInput {
        name: "my_module::MyStruct".to_string(),
        include_adjacency: true,
        reference_depth: 1,
    })
    .await
    .unwrap();

// Validate workspace operations
let validation = validation_service
    .validate_workspace(ValidateWorkspaceInput {
        changed_files: vec![PathBuf::from("src/lib.rs")],
        intent: SymbolWorkspaceIntent::Modification,
        check_references: true,
        check_conflicts: true,
    })
    .await
    .unwrap();
```

## Graceful Shutdown

1. **Drain pending indexing operations**: Wait for any in-progress `index_file` or
   `index_directory` calls to complete.
2. **Persist symbol cache** (optional): If a `SymbolRepository` is configured, save
   the current symbol graph to persistent storage.
3. **Release tree-sitter grammars**: If `GrammarRepository` is active, unload
   grammar resources.
4. **Drop graph reference**: Release the `SharedSymbolGraph` or `SymbolGraphServiceImpl`
   — the RwLock ensures all readers complete before the graph is dropped.

```rust
// Graceful shutdown sequence
// 1. Cancel any pending indexing tasks
// 2. Persist symbol cache
if let Some(repo) = symbol_repository {
    let graph = graph_service.export_graph().await;
    repo.save_symbols_batch(&graph.all_definitions().values().cloned().collect::<Vec<_>>()).await?;
}
// 3. Resources released on drop
```

## Common Failure Modes and Recovery

| Failure Mode | Symptoms | Recovery |
|--------------|----------|----------|
| Duplicate symbol | `RepoEngineError::DuplicateSymbol` when adding | Check for existing symbols before add; use `remove_symbol` first if overwrite intended |
| Symbol not found | `RepoEngineError::SymbolNotFound` on lookup/modify/delete | Verify the fully qualified name; check `suggestions` field for near-matches |
| Capacity exceeded | `RepoEngineError::CapacityExceeded` | Increase `max_capacity` or remove unused symbols via `remove_symbol` |
| RwLock poisoned | Panic on read/write | Another thread panicked while holding the lock; restart the service |
| Indexing cancelled | `RepoEngineError::IndexingCancelled` | Task was cancelled mid-index; retry the indexing operation |
| Unsupported extension | `RepoEngineError::UnsupportedExtension` | File type is not supported; skip the file and continue |
| Parse error | `RepoEngineError::ParseError` | File has syntax errors; report the error and skip the file |
| IO error | `RepoEngineError::Io` | Filesystem error; check file permissions and disk space |

## Configuration Reference

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `language_extensions` | Map | `.rs`, `.py`, `.ts`, `.tsx` | File extensions per language |
| `exclude_patterns` | Vec<String> | `*.min.*`, `*.generated.*` | File patterns to skip |
| `max_file_size` | u64 | 1 MB | Maximum file size to index |
| `max_symbol_capacity` | usize | 0 (unlimited) | Maximum symbols in graph |
| `build_adjacency` | bool | true | Track reference relationships |
| `max_files_per_scan` | usize | 100,000 | Files per directory scan |
| `index_on_startup` | bool | true | Auto-index at startup |
| `exclude_dirs` | Vec<String> | node_modules, target, .git, etc. | Directories to skip |

## Observability

### Key Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `repo_engine.symbols.total` | Gauge | Total symbols in the graph |
| `repo_engine.symbols.indexed_total` | Counter | Cumulative symbols indexed (lifetime) |
| `repo_engine.symbols.by_kind` | GaugeVec | Symbols per kind (Function, Struct, etc.) |
| `repo_engine.symbols.by_language` | GaugeVec | Symbols per language (Rust, Python, TS) |
| `repo_engine.references.total` | Gauge | Total reference edges |
| `repo_engine.lookups.total` | Counter | Cumulative symbol lookups |
| `repo_engine.lookups.hits` | Counter | Successful lookups |
| `repo_engine.lookups.misses` | Counter | Failed lookups |
| `repo_engine.indexing.files_processed` | Counter | Files indexed |
| `repo_engine.indexing.duration_ms` | Histogram | Indexing duration |
| `repo_engine.errors.total` | Counter | Total error count by type |

### Health Check Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health/repo-engine` | GET | Returns graph status (symbol count, capacity) and indexing state |
| `/health/repo-engine/ready` | GET | Returns 200 when graph is initialized and ready for queries |
| `/health/repo-engine/live` | GET | Returns 200 while the service is running |

### Logging

All operations should log with a `repo_engine` component tag and include:
- `execution_id` — Correlation ID for the current operation
- `symbol_name` — Affected symbol (if applicable)
- `file_path` — Affected file (if applicable)
- `duration_ms` — Operation duration

```rust
// Structured logging pattern
tracing::info!(
    component = "repo_engine",
    execution_id = %execution_id,
    symbol_name = %name,
    duration_ms = %elapsed,
    "Symbol indexed successfully"
);
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Symbol lookup by name | O(1) | HashMap-based |
| Symbol lookup by file | O(n) | Full scan of definitions |
| Search by pattern | O(n) | Full scan with substring matching |
| Add symbol | O(1) | Insert into HashMap |
| Remove symbol | O(n) | O(1) lookup + O(n) adjacency cleanup |
| Add reference | O(1) | Insert into adjacency map |
| References to symbol | O(n) | Full scan of adjacency map |
| Graph stats | O(n) | Full scan for per-kind/language counts |
| File indexing | Varies | Depends on file size and tree-sitter parse speed |

## Dependencies Graph

```
repo-engine
  ├── Depends on: Configuration (file extensions, ignore patterns)
  └── Used by: Planning Pipeline (enriched symbol context)
      Used by: Template Generation (Phase 3 symbol validation)
      Used by: Orchestrator (indexes repo at execution start)
```
