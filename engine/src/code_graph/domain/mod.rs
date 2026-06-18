//! Domain entities for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md#domain
//! Implements: Contract Freeze — CodeGraph, ModuleNode, ModuleEdge domain entities
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types for code dependency graph
//! construction and analysis. These are pure domain objects with no
//! framework dependencies.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation happens in the application layer (service traits)
//! - All persistence happens behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod error;
pub mod event;
pub mod graph;

pub use error::*;
pub use graph::*;
