//! Repo Engine — Multi-language code indexing and symbol graph management.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — module root for SymbolGraph, SymbolDefinition, SymbolWorkspaceIntent
//! Issue: #138
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded contexts (DDD):
//!
//! - `domain/` — `SymbolGraph`, `SymbolDefinition`, `SymbolKind`, `Location`, `SymbolWorkspaceIntent`,
//!   `SharedSymbolGraph`, `RepoEngineError`, events
//! - `application/` — Service traits (`SymbolGraphService`, `IndexerService`),
//!   DTOs for all operations, factory interfaces
//! - `infrastructure/` — Repository interfaces for indexer storage and parser registration
//! - `interfaces/` — API contracts for symbol graph and indexing operations
//!
//! # Architecture References
//!
//! | Component | File (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | SymbolGraph | `rigorix/src/repo_engine/symbol_graph.rs` | `.pi/architecture/modules/repo-engine.md#graph` |
//! | SymbolDefinition | `rigorix/src/repo_engine/symbol_graph.rs` | `.pi/architecture/modules/repo-engine.md#definition` |
//! | SharedSymbolGraph | `rigorix/src/repo_engine/symbol_graph.rs` | `.pi/architecture/modules/repo-engine.md#shared` |
//! | RustIndexer | `rigorix/src/repo_engine/indexer.rs` | `.pi/architecture/modules/repo-engine.md#rust` |
//! | PythonIndexer | `rigorix/src/repo_engine/python_indexer.rs` | `.pi/architecture/modules/repo-engine.md#python` |
//! | TypeScriptIndexer | `rigorix/src/repo_engine/typescript_indexer.rs` | `.pi/architecture/modules/repo-engine.md#typescript` |
//!
//! # Dependencies
//!
//! - **Depends on:** Configuration (file extension mappings, ignore patterns)
//! - **Used by:** Planning Pipeline (enriched symbol context for classification),
//!   Template Generation (Phase 3 symbol validation),
//!   Orchestrator (indexes repo at execution start)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
