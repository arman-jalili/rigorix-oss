//! Infrastructure layer interfaces for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md#infrastructure
//! Implements: Contract Freeze — repository interfaces
//! Issue: #124
//!
//! This module defines repository interfaces that abstract tool storage
//! and retrieval behind clean interfaces. These are the only infrastructure
//! contracts needed — no database schemas, no framework-specific annotations.
//!
//! # Contract (Frozen)
//! - Repository traits only — no implementations
//! - All methods are async
//! - All methods return domain error types

pub mod repository;
pub mod tree_sitter_anchor;
