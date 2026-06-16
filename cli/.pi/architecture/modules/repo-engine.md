# Repo Engine

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Multi-language code indexing and symbol graph management. Uses tree-sitter for parsing Rust, Python, and TypeScript. Builds a `SymbolGraph` mapping symbol names to definitions, locations, and cross-references. Provides O(1) name lookups.

Used by the Planning Pipeline (intent enrichment) and Template Generation (RepoContext for LLM context).

## Components

**CLI-facing:** None — CLI wraps engine contracts directly. No CLI-specific interface files needed.

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| SymbolGraph (aggregate root) | `engine/src/repo_engine/domain/symbol_graph.rs` | `# Contract (Frozen)` |
| SymbolDefinition | `engine/src/repo_engine/domain/symbol_graph.rs` | Single symbol: name, kind, location, doc |
| SymbolKind | `engine/src/repo_engine/domain/symbol_graph.rs` | Enum: Function, Class, Struct, Module, etc. |
| Location | `engine/src/repo_engine/domain/symbol_graph.rs` | File path + line/column range |
| SymbolWorkspaceIntent | `engine/src/repo_engine/domain/symbol_workspace.rs` | Workspace-level intent |
| SharedSymbolGraph | `engine/src/repo_engine/domain/symbol_graph.rs` | Thread-safe shared graph |
| IndexerService (trait) | `engine/src/repo_engine/application/service.rs` | Indexer service trait |
| SymbolGraphService (trait) | `engine/src/repo_engine/application/service.rs` | Graph query service |
| RepoEngineError | `engine/src/repo_engine/domain/error.rs` | Typed error enum |

## Indexer Implementations

| Language | Engine Source | Parser |
|----------|--------------|--------|
| Rust | `engine/src/repo_engine/infrastructure/rust_indexer.rs` | tree-sitter-rust |
| Python | `engine/src/repo_engine/infrastructure/python_indexer.rs` | tree-sitter-python |
| TypeScript | `engine/src/repo_engine/infrastructure/typescript_indexer.rs` | tree-sitter-typescript |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| SymbolGraph | Multi-language code index mapping symbol names to definitions, locations, and references. O(1) lookup. |
| SymbolDefinition | A single code symbol: name, kind (Function/Class/Struct/etc.), file location, documentation. |
| SymbolKind | Enum: Function, Class, Struct, Module, Interface, Enum, Trait, etc. |
| Location | File path + line/column range for a symbol definition. |

## Dependencies

- Depends on: `engine::repo_engine` (all contracts frozen)
- Depends on: `Configuration` (file extension mappings, ignore patterns)
- Depends on: tree-sitter (multi-language parsing)
- Used by: `Planning Pipeline` (enriched symbol context for classification)
- Used by: `Template Generation` (RepoContext for LLM context)
