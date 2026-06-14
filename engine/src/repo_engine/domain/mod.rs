//! Domain entities and interfaces for the Repo Engine bounded context.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#domain
//! Implements: Contract Freeze — SymbolGraph, SymbolDefinition, SymbolKind, Location,
//!   SymbolWorkspaceIntent, SharedSymbolGraph, RepoEngineError, RepoEngineEvent
//! Issue: #138
//!
//! This module defines the core domain types — `SymbolGraph`, `SymbolDefinition`,
//! `SymbolKind`, `Location`, `SymbolWorkspaceIntent`, and all sub-types. These are
//! pure domain objects with no framework dependencies. They serve as the frozen
//! contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All indexing logic must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - SymbolGraph maintains O(1) lookups by symbol name

pub mod error;
pub mod event;
pub mod symbol_graph;
pub mod symbol_workspace;

pub use error::RepoEngineError;
pub use symbol_graph::*;
pub use symbol_workspace::*;
