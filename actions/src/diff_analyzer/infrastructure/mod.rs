//! Infrastructure layer for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for diff data access
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces that abstract data access
//! for the Diff Analyzer module. Implementations will read from the
//! GitHub API (to fetch PR diffs) and potentially cache results.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return `Result<_, DiffAnalyzerError>`
//! - No dependencies on external frameworks

pub mod repository;

pub use repository::*;
