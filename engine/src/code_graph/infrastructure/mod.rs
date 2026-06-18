//! Infrastructure layer for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for code graph persistence
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces for persisting CodeGraph
//! records. Default implementations may use filesystem storage,
//! databases, or other backends.

pub mod repository;
pub use repository::*;
